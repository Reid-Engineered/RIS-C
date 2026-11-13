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
                    && *rise > 0.0
                    && *fall > 0.0
                    && *width >= 0.0
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
            Err(Error::Circuit("invalid waveform (pulse edges must have positive duration; PWL times strictly increase)".into()))
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
                let u = (t - delay).rem_euclid(*period);
                if u < *rise {
                    low + (high - low) * u / rise
                } else if u < rise + width {
                    *high
                } else if u < rise + width + fall {
                    high + (low - high) * (u - rise - width) / fall
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
