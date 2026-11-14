use crate::{Error, Result};

#[derive(Clone, Debug)]
pub enum Waveform {
    Constant(f64),
    Pulse {
        low: f64,
        high: f64,
        delay: f64,
        rise: f64,
        fall: f64,
        width: f64,
        period: f64,
    },
    Sin {
        offset: f64,
        amplitude: f64,
        frequency: f64,
        delay: f64,
        damping: f64,
        phase: f64,
    },
    Pwl(Vec<(f64, f64)>),
}
impl Waveform {
    pub fn validate(&self) -> Result<()> {
        let ok = match self {
            Self::Constant(v) => v.is_finite(),
            Self::Pulse {
                low,
                high,
                delay,
                rise,
                fall,
                width,
                period,
            } => {
                [*low, *high, *delay, *rise, *fall, *width, *period]
                    .iter()
                    .all(|v| v.is_finite())
                    && *delay >= 0.0
                    && *rise >= 0.0
                    && *fall >= 0.0
                    && *width >= 0.0
                    && *period > 0.0
                    && *period >= rise + width + fall
            }
            Self::Sin {
                offset,
                amplitude,
                frequency,
                delay,
                damping,
                phase,
            } => {
                [*offset, *amplitude, *frequency, *delay, *damping, *phase]
                    .iter()
                    .all(|v| v.is_finite())
                    && *frequency >= 0.0
                    && *delay >= 0.0
                    && *damping >= 0.0
            }
            Self::Pwl(p) => {
                !p.is_empty()
                    && p.iter()
                        .all(|(t, v)| t.is_finite() && *t >= 0.0 && v.is_finite())
                    && p.windows(2).all(|w| w[0].0 < w[1].0)
            }
        };
        if ok {
            Ok(())
        } else {
            Err(Error::Circuit(
                "invalid waveform (pulse durations must be nonnegative with positive period >= rise+width+fall; PWL times strictly increase)"
                    .into(),
            ))
        }
    }
    pub fn value(&self, t: f64) -> f64 {
        match self {
            Self::Constant(v) => *v,
            Self::Pulse {
                low,
                high,
                delay,
                rise,
                fall,
                width,
                period,
            } => {
                if t < *delay {
                    return *low;
                }
                let eps = 16.0 * f64::EPSILON * t.abs().max(1e-30);
                let mut u = (t - delay).rem_euclid(*period);
                if u >= *period - eps {
                    u = 0.0;
                }
                if u + eps < *rise {
                    low + (high - low) * u / rise
                } else if u + eps < rise + width {
                    *high
                } else if u + eps < rise + width + fall {
                    high + (low - high) * (u - rise - width).max(0.0) / fall
                } else {
                    *low
                }
            }
            Self::Sin {
                offset,
                amplitude,
                frequency,
                delay,
                damping,
                phase,
            } => {
                let u = (t - delay).max(0.0);
                offset
                    + amplitude
                        * (-damping * u).exp()
                        * (std::f64::consts::TAU * frequency * u + phase.to_radians()).sin()
            }
            Self::Pwl(p) => {
                let k = p.partition_point(|(u, _)| *u <= t);
                if k == 0 {
                    p[0].1
                } else if k == p.len() {
                    p[k - 1].1
                } else {
                    let (a, x) = p[k - 1];
                    let (b, y) = p[k];
                    x + (y - x) * (t - a) / (b - a)
                }
            }
        }
    }
    /// Next waveform knot, including the end of each finite pulse edge.
    pub fn next_breakpoint(&self, t: f64) -> Option<f64> {
        let eps = 16.0 * f64::EPSILON * t.abs().max(1e-30);
        match self {
            Self::Pulse {
                delay,
                rise,
                fall,
                width,
                period,
                ..
            } => {
                let cycle = ((t - delay) / period).floor().max(0.0);
                [0.0, *rise, rise + width, rise + width + fall, *period]
                    .into_iter()
                    .flat_map(|u| {
                        [
                            delay + cycle * period + u,
                            delay + (cycle + 1.0) * period + u,
                        ]
                    })
                    .filter(|u| *u > t + eps)
                    .min_by(f64::total_cmp)
            }
            Self::Sin { delay, .. } => (*delay > t + eps).then_some(*delay),
            Self::Pwl(p) => p.iter().map(|x| x.0).find(|u| *u > t + eps),
            _ => None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Source {
    pub dc: f64,
    pub ac: num_complex::Complex64,
    pub waveform: Option<Waveform>,
}
impl Source {
    pub fn dc(value: f64) -> Self {
        Self {
            dc: value,
            ac: Default::default(),
            waveform: None,
        }
    }
    pub fn at(&self, t: Option<f64>) -> f64 {
        match (t, &self.waveform) {
            (Some(t), Some(w)) => w.value(t),
            _ => self.dc,
        }
    }
    pub fn validate(&self) -> Result<()> {
        if !self.dc.is_finite() || !self.ac.re.is_finite() || !self.ac.im.is_finite() {
            return Err(Error::Circuit("nonfinite source".into()));
        }
        if let Some(w) = &self.waveform {
            w.validate()?;
        }
        Ok(())
    }
}
impl From<f64> for Source {
    fn from(v: f64) -> Self {
        Self::dc(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_edge_pulse_validation_and_evaluation() {
        let square = Waveform::Pulse {
            low: 0.0,
            high: 5.0,
            delay: 1e-6,
            rise: 0.0,
            fall: 0.0,
            width: 2e-6,
            period: 5e-6,
        };
        assert!(square.validate().is_ok());
        assert_eq!(square.value(0.0), 0.0);
        assert_eq!(square.value(0.999e-6), 0.0);
        assert_eq!(square.value(1e-6), 5.0);
        assert_eq!(square.value(2.5e-6), 5.0);
        assert_eq!(square.value(3e-6), 0.0);
        assert_eq!(square.value(5.999e-6), 0.0);
        assert_eq!(square.value(6e-6), 5.0);
        assert_eq!(square.value(8e-6), 0.0);

        assert_eq!(square.next_breakpoint(0.0), Some(1e-6));
        assert_eq!(square.next_breakpoint(1e-6), Some(3e-6));
        assert_eq!(square.next_breakpoint(3e-6), Some(6e-6));
        assert_eq!(square.next_breakpoint(6e-6), Some(8e-6));
    }

    #[test]
    fn asymmetric_zero_edge_and_zero_width_pulses() {
        let zero_rise = Waveform::Pulse {
            low: -1.0,
            high: 1.0,
            delay: 0.0,
            rise: 0.0,
            fall: 2.0,
            width: 0.0,
            period: 4.0,
        };
        assert!(zero_rise.validate().is_ok());
        assert_eq!(zero_rise.value(0.0), 1.0);
        assert!((zero_rise.value(1.0) - 0.0).abs() < 1e-12);
        assert_eq!(zero_rise.value(2.0), -1.0);
        assert_eq!(zero_rise.value(3.5), -1.0);
        assert_eq!(zero_rise.next_breakpoint(0.0), Some(2.0));
        assert_eq!(zero_rise.next_breakpoint(2.0), Some(4.0));

        let zero_fall = Waveform::Pulse {
            low: 0.0,
            high: 3.0,
            delay: 0.0,
            rise: 2.0,
            fall: 0.0,
            width: 1.0,
            period: 4.0,
        };
        assert!(zero_fall.validate().is_ok());
        assert_eq!(zero_fall.value(0.0), 0.0);
        assert!((zero_fall.value(1.0) - 1.5).abs() < 1e-12);
        assert_eq!(zero_fall.value(2.0), 3.0);
        assert_eq!(zero_fall.value(3.0), 0.0);
        assert_eq!(zero_fall.next_breakpoint(0.0), Some(2.0));
        assert_eq!(zero_fall.next_breakpoint(2.0), Some(3.0));
        assert_eq!(zero_fall.next_breakpoint(3.0), Some(4.0));

        let flat_zero = Waveform::Pulse {
            low: 1.5,
            high: 5.0,
            delay: 0.0,
            rise: 0.0,
            fall: 0.0,
            width: 0.0,
            period: 2.0,
        };
        assert!(flat_zero.validate().is_ok());
        assert_eq!(flat_zero.value(0.0), 1.5);
        assert_eq!(flat_zero.value(1.0), 1.5);
        assert_eq!(flat_zero.next_breakpoint(0.0), Some(2.0));
    }

    #[test]
    fn pulse_validation_rejects_invalid_durations() {
        let zero_period = Waveform::Pulse {
            low: 0.0,
            high: 1.0,
            delay: 0.0,
            rise: 0.0,
            fall: 0.0,
            width: 0.0,
            period: 0.0,
        };
        assert!(zero_period.validate().is_err());

        let negative_rise = Waveform::Pulse {
            low: 0.0,
            high: 1.0,
            delay: 0.0,
            rise: -1e-9,
            fall: 0.0,
            width: 1e-6,
            period: 2e-6,
        };
        assert!(negative_rise.validate().is_err());

        let negative_fall = Waveform::Pulse {
            low: 0.0,
            high: 1.0,
            delay: 0.0,
            rise: 0.0,
            fall: -1e-9,
            width: 1e-6,
            period: 2e-6,
        };
        assert!(negative_fall.validate().is_err());

        let short_period = Waveform::Pulse {
            low: 0.0,
            high: 1.0,
            delay: 0.0,
            rise: 1e-6,
            fall: 1e-6,
            width: 1e-6,
            period: 2.5e-6,
        };
        assert!(short_period.validate().is_err());
    }
}
