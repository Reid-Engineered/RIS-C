use crate::{
    circuit::{DeviceHistory, Evaluation, MeyerState, PhaseState},
    *,
};
use std::{collections::HashMap, ops::ControlFlow};

#[derive(Clone, Debug)]
pub struct SolverOptions {
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub max_iterations: usize,
    pub transient_reltol: f64,
}
impl Default for SolverOptions {
    fn default() -> Self {
        Self {
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            max_iterations: 100,
            transient_reltol: 1e-3,
        }
    }
}
impl SolverOptions {
    pub fn precise() -> Self {
        Self {
            reltol: 1e-8,
            vntol: 1e-10,
            abstol: 1e-14,
            max_iterations: 200,
            transient_reltol: 1e-4,
        }
    }
    fn validate(&self) -> Result<()> {
        if [self.reltol, self.vntol, self.abstol, self.transient_reltol]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
            || self.max_iterations == 0
        {
            Err(Error::Analysis(
                "invalid solver tolerances/iteration limit".into(),
            ))
        } else {
            Ok(())
        }
    }
}
/// Strategy that last returned a solution of the original DC equations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DcMethod {
    Direct,
    GminStepping,
    SourceStepping,
}
#[derive(Clone, Debug, Default)]
pub struct RunStats {
    pub newton_iterations: usize,
    pub factorizations: usize,
    pub accepted_steps: usize,
    pub rejected_steps: usize,
    /// Total Newton attempts made with gmin or source continuation parameters.
    pub continuation_attempts: usize,
    pub direct_failures: usize,
    pub gmin_attempts: usize,
    pub source_attempts: usize,
    pub dc_method: Option<DcMethod>,
    /// Transient convergence at the representational precision floor, with updates still converged.
    pub roundoff_limited_convergences: usize,
    pub breakpoints: usize,
    pub minimum_step: Option<f64>,
}
#[derive(Clone, Debug)]
pub struct Sample {
    pub axis: f64,
    pub values: Vec<f64>,
}
impl Sample {
    pub fn get(&self, circuit: &CompiledCircuit, name: &str) -> Option<f64> {
        if name.eq_ignore_ascii_case("v(0)") {
            return Some(0.0);
        }
        circuit
            .signals()
            .iter()
            .position(|s| s.eq_ignore_ascii_case(name))
            .map(|k| self.values[k])
    }
}
#[derive(Clone, Debug)]
pub struct AcSample {
    pub frequency: f64,
    pub values: Vec<Complex64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegrationMethod {
    BackwardEuler,
    Trapezoidal,
    Bdf2,
}
#[derive(Clone, Debug)]
pub struct TransientOptions {
    pub stop: f64,
    pub output_step: f64,
    pub max_step: f64,
    pub min_step: f64,
    pub max_steps: usize,
    pub max_retries: usize,
    pub method: IntegrationMethod,
    pub uic: bool,
}
impl TransientOptions {
    pub fn new(stop: f64, output_step: f64) -> Self {
        Self {
            stop,
            output_step,
            max_step: output_step,
            min_step: 1e-18,
            max_steps: 1_000_000,
            max_retries: 40,
            method: IntegrationMethod::Trapezoidal,
            uic: false,
        }
    }
    fn validate(&self) -> Result<()> {
        if [self.stop, self.output_step, self.max_step, self.min_step]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
            || self.min_step > self.max_step
            || self.max_steps == 0
            || self.max_retries == 0
        {
            Err(Error::Analysis(
                "invalid transient interval or limits".into(),
            ))
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug)]
pub enum FrequencyGrid {
    Linear {
        points: usize,
        start: f64,
        stop: f64,
    },
    Decade {
        points_per_decade: usize,
        start: f64,
        stop: f64,
    },
    Octave {
        points_per_octave: usize,
        start: f64,
        stop: f64,
    },
}
impl FrequencyGrid {
    pub fn values(&self) -> Result<Vec<f64>> {
        let (count, start, stop, base) = match *self {
            Self::Linear {
                points,
                start,
                stop,
            } => (points, start, stop, 0.0_f64),
            Self::Decade {
                points_per_decade,
                start,
                stop,
            } => (points_per_decade, start, stop, 10.0),
            Self::Octave {
                points_per_octave,
                start,
                stop,
            } => (points_per_octave, start, stop, 2.0),
        };
        if count == 0
            || count > 1_000_000
            || !start.is_finite()
            || !stop.is_finite()
            || start < 0.0
            || stop < start
            || (base != 0.0 && start <= 0.0)
        {
            return Err(Error::Analysis("invalid frequency grid".into()));
        }
        if base == 0.0 {
            Ok((0..count)
                .map(|k| {
                    if count == 1 {
                        start
                    } else {
                        start + (stop - start) * k as f64 / (count - 1) as f64
                    }
                })
                .collect())
        } else {
            let span = (stop / start).log(base) * count as f64;
            let n = (span + 32.0 * f64::EPSILON * span.abs()).floor() as usize;
            if n > 1_000_000 {
                return Err(Error::Analysis("frequency grid too large".into()));
            }
            Ok((0..=n)
                .map(|k| start * base.powf(k as f64 / count as f64))
                .collect())
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
struct History {
    time: f64,
    x: Vec<f64>,
    q: Vec<f64>,
    dq: Vec<f64>,
    previous_q: Option<Vec<f64>>,
    step: Option<f64>,
    meyer: Vec<Option<MeyerState>>,
    phase: Vec<Option<PhaseState>>,
}
#[derive(Clone, Copy)]
struct Dynamic<'a> {
    a0: f64,
    reference: &'a [f64],
    history: &'a [f64],
}
struct Problem<'a> {
    time: Option<f64>,
    dynamic: Option<Dynamic<'a>>,
    gmin: f64,
    scale: f64,
    source: Option<(&'a str, f64)>,
    constraints: Option<&'a HashMap<usize, f64>>,
    storage: Option<DeviceHistory<'a>>,
}
impl Default for Problem<'_> {
    fn default() -> Self {
        Self {
            time: None,
            dynamic: None,
            gmin: 0.0,
            scale: 1.0,
            source: None,
            constraints: None,
            storage: None,
        }
    }
}
#[derive(Debug)]
pub struct Simulator {
    pub circuit: CompiledCircuit,
    pub options: SolverOptions,
    pub stats: RunStats,
    state: Option<Vec<f64>>,
}
impl Simulator {
    pub fn new(circuit: CompiledCircuit) -> Self {
        Self {
            circuit,
            options: Default::default(),
            stats: Default::default(),
            state: None,
        }
    }
    pub fn operating_point(&mut self) -> Result<Sample> {
        self.options.validate()?;
        self.stats = Default::default();
        let start = self
            .state
            .clone()
            .unwrap_or_else(|| self.circuit.0.initial.clone());
        let x = self.dc_solve(&start, None, None)?;
        self.state = Some(x.clone());
        Ok(Sample {
            axis: 0.0,
            values: x,
        })
    }
    fn dc_solve(
        &mut self,
        start: &[f64],
        source: Option<(&str, f64)>,
        constraints: Option<&HashMap<usize, f64>>,
    ) -> Result<Vec<f64>> {
        let p = Problem {
            source,
            constraints,
            ..Default::default()
        };
        if let Ok(x) = self.newton(start, &p) {
            self.stats.dc_method = Some(DcMethod::Direct);
            return Ok(x);
        }
        self.stats.direct_failures += 1;
        self.stats.dc_method = None;
        // Each continuation begins from its own known state and ends on the original equations.
        let mut x = start.to_vec();
        let mut g = 1e-2;
        let mut step = 0.5;
        let mut attempts = 0;
        self.stats.gmin_attempts += 1;
        self.stats.continuation_attempts += 1;
        if let Ok(y) = self.newton(&x, &Problem { gmin: g, ..p }) {
            x = y;
            while g > 1e-12 && attempts < 100 {
                let next = (g * step).max(1e-13);
                attempts += 1;
                self.stats.continuation_attempts += 1;
                self.stats.gmin_attempts += 1;
                match self.newton(&x, &Problem { gmin: next, ..p }) {
                    Ok(y) => {
                        x = y;
                        g = next;
                        step = (step * 0.8).max(0.1);
                    }
                    Err(_) => {
                        step = step.sqrt();
                        if step > 0.999 {
                            break;
                        }
                    }
                }
            }
            if let Ok(y) = self.newton(&x, &p) {
                self.stats.dc_method = Some(DcMethod::GminStepping);
                return Ok(y);
            }
        }
        x = vec![0.0; start.len()];
        let mut alpha = 0.0_f64;
        let mut inc = 0.1_f64;
        self.stats.source_attempts += 1;
        self.stats.continuation_attempts += 1;
        x = self.newton(&x, &Problem { scale: 0.0, ..p })?;
        for _ in 0..150 {
            let next = (alpha + inc).min(1.0_f64);
            self.stats.continuation_attempts += 1;
            self.stats.source_attempts += 1;
            match self.newton(&x, &Problem { scale: next, ..p }) {
                Ok(y) => {
                    x = y;
                    alpha = next;
                    inc = (inc * 1.5).min(0.25);
                    if alpha == 1.0 {
                        let solution = self.newton(&x, &p)?;
                        self.stats.dc_method = Some(DcMethod::SourceStepping);
                        return Ok(solution);
                    }
                }
                Err(_) => {
                    inc *= 0.5;
                    if inc < 1e-8 {
                        break;
                    }
                }
            }
        }
        let solution = self.newton(&x, &p)?;
        self.stats.dc_method = Some(DcMethod::SourceStepping);
        Ok(solution)
    }
    fn residual(
        &self,
        e: &Evaluation,
        x: &[f64],
        p: &Problem<'_>,
    ) -> (Vec<f64>, Vec<f64>, f64, f64) {
        let data = &self.circuit.0;
        let mut f = e.f.clone();
        let mut jac = e.g.clone();
        let mut scale = e.scale.clone();
        if let Some(d) = &p.dynamic {
            for k in 0..f.len() {
                let storage = d.a0 * (e.q[k] - d.reference[k]) + d.history[k];
                f[k] += storage;
                scale[k] += storage.abs();
            }
            for (j, c) in jac.iter_mut().zip(&e.c) {
                *j += d.a0 * c;
            }
        }
        if p.gmin > 0.0 {
            for k in 0..data.nvolt {
                f[k] += p.gmin * x[k];
                jac[data.pattern.slot(k, k)] += p.gmin;
            }
        }
        if let Some(constraints) = p.constraints {
            for (&r, &v) in constraints {
                for (k, &(row, _)) in data.pattern.coords.iter().enumerate() {
                    if row == r {
                        jac[k] = 0.0;
                    }
                }
                jac[data.pattern.slot(r, r)] = 1.0;
                f[r] = x[r] - v;
                scale[r] = x[r].abs().max(v.abs());
            }
        }
        // Discretized storage subtracts nearly equal charges. Bound roundoff in
        // both the charge values and their voltage/flux arguments rather than
        // rejecting a converged update against an unrepresentable residual.
        let mut rounding = vec![0.0; f.len()];
        if let Some(dynamic) = &p.dynamic {
            for (k, bound) in rounding.iter_mut().enumerate() {
                *bound = dynamic.a0.abs() * (e.q[k].abs() + dynamic.reference[k].abs())
                    + dynamic.history[k].abs();
            }
            for (slot, &(row, col)) in data.pattern.coords.iter().enumerate() {
                rounding[row] += jac[slot].abs() * x[col].abs();
            }
            for bound in &mut rounding {
                *bound *= 16.0 * f64::EPSILON;
            }
        }
        let mut norm = 0.0_f64;
        let mut requested_norm = 0.0_f64;
        for (k, value) in f.iter().enumerate() {
            let voltage_equation =
                k >= data.nvolt || p.constraints.is_some_and(|m| m.contains_key(&k));
            let tolerance = if voltage_equation {
                self.options.vntol
            } else {
                self.options.abstol
            } + self.options.reltol * scale[k];
            requested_norm = requested_norm.max(value.abs() / tolerance);
            norm = norm.max(value.abs() / (tolerance + rounding[k]));
        }
        (f, jac, norm, requested_norm)
    }

    fn newton(&mut self, start: &[f64], p: &Problem<'_>) -> Result<Vec<f64>> {
        let mut x = start.to_vec();
        let mut last = f64::INFINITY;
        let nonlinear = self
            .circuit
            .0
            .devices
            .iter()
            .any(|d| matches!(d.element, Element::Semiconductor { .. }));
        for iter in 0..self.options.max_iterations {
            self.stats.newton_iterations += 1;
            let e = self
                .circuit
                .0
                .evaluate_storage(&x, p.time, p.scale, p.source, p.storage)?;
            let (f, jac, norm, requested_norm) = self.residual(&e, &x, p);
            last = norm;
            let rhs: Vec<_> = f.iter().map(|v| -v).collect();
            self.stats.factorizations += 1;
            let dx = self.circuit.0.pattern.solve(&jac, &rhs)?;
            let update = dx
                .iter()
                .enumerate()
                .map(|(k, v)| {
                    v.abs()
                        / (if k < self.circuit.0.nvolt {
                            self.options.vntol
                        } else {
                            self.options.abstol
                        } + self.options.reltol * x[k].abs())
                })
                .fold(0.0_f64, f64::max);
            if norm <= 1.0 && update <= 1.0 {
                if requested_norm > 1.0 {
                    self.stats.roundoff_limited_convergences += 1;
                }
                return Ok(x);
            }
            // Dampen voltage updates, then backtrack on the actual nonlinear residual.
            let maxdv = dx[..self.circuit.0.nvolt]
                .iter()
                .map(|v| v.abs())
                .fold(0.0_f64, f64::max);
            let mut alpha = if nonlinear {
                (0.5 / maxdv).min(1.0)
            } else {
                1.0
            };
            let weights: Vec<_> = e
                .scale
                .iter()
                .enumerate()
                .map(|(k, s)| {
                    let abs = if k < self.circuit.0.nvolt {
                        self.options.abstol
                    } else {
                        self.options.vntol
                    };
                    abs + self.options.reltol * (s + f[k].abs())
                })
                .collect();
            let merit = |f: &[f64]| {
                f.iter()
                    .zip(&weights)
                    .map(|(f, w)| f.abs() / w)
                    .fold(0.0_f64, f64::max)
            };
            let baseline = merit(&f);
            let mut candidate = x.clone();
            for attempt in 0..16 {
                for k in 0..x.len() {
                    candidate[k] = x[k] + alpha * dx[k];
                }
                if !nonlinear {
                    break;
                }
                if let Ok(next) = self
                    .circuit
                    .0
                    .evaluate_storage(&candidate, p.time, p.scale, p.source, p.storage)
                {
                    let (nf, _, n, _) = self.residual(&next, &candidate, p);
                    if merit(&nf) < baseline || n <= 1.0 || attempt == 15 {
                        break;
                    }
                }
                alpha *= 0.5;
            }
            if candidate.iter().any(|v| !v.is_finite()) {
                return Err(Error::NonFinite("Newton update".into()));
            }
            x = candidate;
            if iter + 1 == self.options.max_iterations {
                break;
            }
        }
        Err(Error::Convergence {
            iterations: self.options.max_iterations,
            residual: last,
        })
    }
    pub fn dc_sweep(
        &mut self,
        source: &str,
        start: f64,
        stop: f64,
        step: f64,
    ) -> Result<Vec<Sample>> {
        self.options.validate()?;
        let source = source.to_ascii_lowercase();
        if ![start, stop, step].iter().all(|v| v.is_finite())
            || step == 0.0
            || (stop - start) * step < 0.0
        {
            return Err(Error::Analysis("invalid DC sweep".into()));
        }
        if !self.circuit.0.devices.iter().any(|d| {
            d.name == source && matches!(d.element, Element::Voltage(_) | Element::Current(_))
        }) {
            return Err(Error::Analysis(format!(
                "{source} is not an independent source"
            )));
        }
        let span = (stop - start) / step;
        let count = (span + 32.0 * f64::EPSILON * span.abs()).floor() as usize;
        if count > 1_000_000 {
            return Err(Error::Analysis("DC sweep too large".into()));
        }
        self.stats = Default::default();
        let mut x = self.circuit.0.initial.clone();
        let mut samples = Vec::new();
        for k in 0..=count {
            let value = start + k as f64 * step;
            x = self.dc_solve(&x, Some((&source, value)), None)?;
            samples.push(Sample {
                axis: value,
                values: x.clone(),
            });
        }
        // A swept operating point is not the nominal bias for a subsequent AC analysis.
        self.state = None;
        Ok(samples)
    }
    pub fn ac(&mut self, grid: &FrequencyGrid) -> Result<Vec<AcSample>> {
        let frequencies = grid.values()?;
        let op = self.operating_point()?;
        let e = self.circuit.0.evaluate_ac(&op.values)?;
        let rhs = self.circuit.0.ac_rhs();
        let mut samples = Vec::new();
        for frequency in frequencies {
            let omega = std::f64::consts::TAU * frequency;
            let a = self.circuit.0.ac_matrix(&e, &op.values, omega);
            self.stats.factorizations += 1;
            let values = self.circuit.0.pattern.solve_complex(&a, &rhs)?;
            samples.push(AcSample { frequency, values });
        }
        Ok(samples)
    }
    fn initialize(&mut self, uic: bool) -> Result<History> {
        let data = self.circuit.0.clone();
        let (mut x, constraints) = crate::initial::constraints(&data, uic)?;
        if !uic {
            x = self.dc_solve(&x, None, Some(&constraints))?;
        }
        let e = data.evaluate(&x, Some(0.0), 1.0, None)?;
        Ok(History {
            time: 0.0,
            x,
            q: e.q,
            dq: e.f.iter().map(|v| -v).collect(),
            previous_q: None,
            step: None,
            meyer: e.meyer,
            phase: e.phase,
        })
    }
    fn take_step(&mut self, hist: &History, h: f64, method: IntegrationMethod) -> Result<History> {
        let (a0, history) = match method {
            IntegrationMethod::Trapezoidal if hist.step.is_some() => (
                2.0 / h,
                hist.q
                    .iter()
                    .zip(&hist.dq)
                    .map(|(_, d)| -d)
                    .collect::<Vec<_>>(),
            ),
            IntegrationMethod::Bdf2 if hist.previous_q.is_some() => {
                let r = h / hist.step.unwrap();
                let a0 = (1.0 + 2.0 * r) / (1.0 + r) / h;
                let prev = hist.previous_q.as_ref().unwrap();
                (
                    a0,
                    hist.q
                        .iter()
                        .zip(prev)
                        .map(|(q, p)| r * r * (p - q) / ((1.0 + r) * h))
                        .collect(),
                )
            }
            _ => (1.0 / h, hist.q.iter().map(|_| 0.0).collect()),
        };
        let p = Problem {
            time: Some(hist.time + h),
            dynamic: Some(Dynamic {
                a0,
                reference: &hist.q,
                history: &history,
            }),
            storage: Some(DeviceHistory {
                meyer: &hist.meyer,
                phase: &hist.phase,
                step: h,
            }),
            ..Default::default()
        };
        let x = self.newton(&hist.x, &p)?;
        let e = self.circuit.0.evaluate_storage(
            &x,
            Some(hist.time + h),
            1.0,
            None,
            Some(DeviceHistory {
                meyer: &hist.meyer,
                phase: &hist.phase,
                step: h,
            }),
        )?;
        let dq =
            e.q.iter()
                .zip(&history)
                .enumerate()
                .map(|(k, (q, b))| a0 * (q - hist.q[k]) + b)
                .collect();
        Ok(History {
            time: hist.time + h,
            x,
            q: e.q,
            dq,
            previous_q: Some(hist.q.clone()),
            step: Some(h),
            meyer: e.meyer,
            phase: e.phase,
        })
    }
    /// Trial solves borrow accepted history; only the caller may commit a returned step.
    fn estimate_step(
        &mut self,
        hist: &History,
        h: f64,
        method: IntegrationMethod,
    ) -> Result<(History, f64, i32)> {
        let full = self.take_step(hist, h, method)?;
        let half = self.take_step(hist, h * 0.5, method)?;
        let fine = self.take_step(&half, h * 0.5, method)?;
        let order = if method == IntegrationMethod::BackwardEuler || hist.step.is_none() {
            1
        } else {
            2
        };
        let divisor = (2.0_f64).powi(order) - 1.0;
        let ev = self.circuit.0.evaluate_storage(
            &full.x,
            Some(full.time),
            1.0,
            None,
            Some(DeviceHistory {
                meyer: &hist.meyer,
                phase: &hist.phase,
                step: h,
            }),
        )?;
        let mut dynamic = vec![false; full.x.len()];
        for (slot, &(_, col)) in self.circuit.0.pattern.coords.iter().enumerate() {
            if ev.c[slot] != 0.0 {
                dynamic[col] = true;
            }
        }
        let err = full
            .x
            .iter()
            .zip(&fine.x)
            .enumerate()
            .filter(|(k, _)| dynamic[*k])
            .map(|(k, (a, b))| {
                let abs = if k < self.circuit.0.nvolt {
                    self.options.vntol
                } else {
                    self.options.abstol
                };
                (a - b).abs()
                    / (divisor * (abs + self.options.transient_reltol * a.abs().max(b.abs())))
            })
            .fold(0.0_f64, f64::max);
        Ok((full, err, order))
    }
    pub fn transient(&mut self, options: &TransientOptions) -> Result<Vec<Sample>> {
        let mut samples = Vec::new();
        self.transient_with(options, |s| {
            samples.push(s.clone());
            ControlFlow::Continue(())
        })?;
        Ok(samples)
    }
    pub fn transient_with<F>(&mut self, options: &TransientOptions, mut sink: F) -> Result<()>
    where
        F: FnMut(&Sample) -> ControlFlow<()>,
    {
        options.validate()?;
        self.options.validate()?;
        self.stats = Default::default();
        self.state = None;
        let mut hist = self.initialize(options.uic)?;
        if sink(&Sample {
            axis: 0.0,
            values: hist.x.clone(),
        })
        .is_break()
        {
            return Err(Error::Cancelled);
        }
        let mut h = (options.output_step * 0.01)
            .min(options.max_step)
            .max(options.min_step);
        let mut output_index = 1_u64;
        let mut accepted = 0;
        let mut restart = true;
        while hist.time < options.stop {
            if accepted >= options.max_steps {
                return Err(Error::TimeStep {
                    time: hist.time,
                    reason: "accepted-step limit".into(),
                });
            }
            let output_time = (output_index as f64 * options.output_step).min(options.stop);
            let output_time = if options.stop - output_time <= 16.0 * f64::EPSILON * options.stop {
                options.stop
            } else {
                output_time
            };
            let breakpoint = self
                .circuit
                .0
                .devices
                .iter()
                .filter_map(|d| match &d.element {
                    Element::Voltage(s) | Element::Current(s) => s
                        .waveform
                        .as_ref()
                        .and_then(|w| w.next_breakpoint(hist.time)),
                    _ => None,
                })
                .min_by(f64::total_cmp);
            let target = breakpoint
                .map_or(output_time, |b| b.min(output_time))
                .min(options.stop);
            if target - hist.time <= 16.0 * f64::EPSILON * target.abs().max(1e-30) {
                hist.time = target;
                if breakpoint.is_some_and(|b| b == target) {
                    hist.step = None;
                    hist.previous_q = None;
                    restart = true;
                    self.stats.breakpoints += 1;
                }
                if target >= output_time {
                    if sink(&Sample {
                        axis: output_time,
                        values: hist.x.clone(),
                    })
                    .is_break()
                    {
                        return Err(Error::Cancelled);
                    }
                    output_index += 1;
                }
                continue;
            }
            h = h.min(options.max_step).min(target - hist.time);
            let method = if restart {
                IntegrationMethod::BackwardEuler
            } else {
                options.method
            };
            let mut retries = 0;
            let mut last_failure = String::from("minimum step reached");
            loop {
                if h < options.min_step || hist.time + h == hist.time {
                    return Err(Error::TimeStep {
                        time: hist.time,
                        reason: last_failure.clone(),
                    });
                }
                let trial = self.estimate_step(&hist, h, method);
                match trial {
                    Ok((next, err, order)) if err <= 1.0 => {
                        self.stats.accepted_steps += 1;
                        accepted += 1;
                        self.stats.minimum_step =
                            Some(self.stats.minimum_step.map_or(h, |v| v.min(h)));
                        hist = next;
                        restart = false;
                        if (hist.time - target).abs()
                            <= 16.0 * f64::EPSILON * target.abs().max(1e-30)
                        {
                            hist.time = target;
                        }
                        if breakpoint.is_some_and(|b| b == target) && hist.time == target {
                            self.stats.breakpoints += 1;
                            hist.step = None;
                            hist.previous_q = None;
                            restart = true;
                        }
                        if hist.time >= output_time {
                            if sink(&Sample {
                                axis: output_time,
                                values: hist.x.clone(),
                            })
                            .is_break()
                            {
                                return Err(Error::Cancelled);
                            }
                            output_index += 1;
                        }
                        h *= if err == 0.0 {
                            2.0
                        } else {
                            (0.9 * err.powf(-1.0 / (order as f64 + 1.0))).clamp(0.5, 2.0)
                        };
                        break;
                    }
                    Ok((_, err, order)) => {
                        last_failure = format!("local error estimate {err:.3e}");
                        h *= (0.8 * err.powf(-1.0 / (order as f64 + 1.0))).clamp(0.1, 0.5);
                    }
                    Err(
                        e @ (Error::Convergence { .. } | Error::Singular(_) | Error::NonFinite(_)),
                    ) => {
                        last_failure = e.to_string();
                        h *= 0.25;
                    }
                    Err(e) => return Err(e),
                }
                retries += 1;
                self.stats.rejected_steps += 1;
                if retries >= options.max_retries {
                    return Err(Error::TimeStep {
                        time: hist.time,
                        reason: format!("retry limit: {last_failure}"),
                    });
                }
            }
        }
        self.state = Some(hist.x);
        Ok(())
    }
}
#[cfg(test)]
mod reliability_tests {
    use super::*;

    fn diode_bias(supply: f64, iterations: usize) -> Simulator {
        let mut b = CircuitBuilder::new();
        let a = b.node("in");
        let o = b.node("out");
        let g = NodeId::GROUND;
        b.add("v", &[a, g], Element::Voltage(supply.into()))
            .unwrap();
        b.add("r", &[a, o], Element::Resistor(10000.0)).unwrap();
        b.add(
            "d",
            &[o, g],
            Element::Semiconductor {
                model: Model::new(ModelKind::Diode),
                geometry: Default::default(),
            },
        )
        .unwrap();
        let mut s = b.build().compile().unwrap().simulator();
        s.options = SolverOptions::precise();
        s.options.max_iterations = iterations;
        s
    }

    fn assert_original_diode_equations(simulator: &Simulator, solution: &[f64], supply: f64) {
        let c = &simulator.circuit;
        let signal = |name: &str| solution[c.signals().iter().position(|s| s == name).unwrap()];
        let input = signal("v(in)");
        let vd = signal("v(out)");
        assert!((input - supply).abs() < 1e-12);
        let current = (input - vd) / 10000.0;
        let thermal = (1.38064852e-23 / 1.6021766208e-19) * 300.15;
        let diode = 1e-14 * ((vd / thermal).exp() - 1.0) + 1e-12 * vd;
        assert!(
            (current - diode).abs()
                <= simulator.options.abstol
                    + simulator.options.reltol * (current.abs() + diode.abs()),
            "original KCL must hold at the configured tolerances without continuation shunts"
        );
        assert!((signal("i(v)") + current).abs() < 1e-12);
        let e = c.0.evaluate(solution, None, 1.0, None).unwrap();
        let (_, _, residual, _) = simulator.residual(&e, solution, &Problem::default());
        assert!(residual <= 1.0, "original scaled residual is {residual}");
    }

    #[test]
    fn gmin_recovers_after_direct_newton_failure() {
        let mut s = diode_bias(1.0, 8);
        let start = s.circuit.0.initial.clone();
        assert!(matches!(
            s.newton(&start, &Problem::default()),
            Err(Error::Convergence { .. })
        ));
        let solution = s.operating_point().unwrap();
        assert_eq!(s.stats.direct_failures, 1);
        assert_eq!(s.stats.dc_method, Some(DcMethod::GminStepping));
        assert!(s.stats.gmin_attempts > 1);
        assert_eq!(s.stats.source_attempts, 0);
        assert_eq!(s.stats.continuation_attempts, s.stats.gmin_attempts);
        assert_original_diode_equations(&s, &solution.values, 1.0);
        let mut reference = diode_bias(1.0, 200);
        let reference = reference.operating_point().unwrap();
        for (a, b) in solution.values.iter().zip(reference.values) {
            assert!((a - b).abs() < 1e-9);
        }
    }

    #[test]
    fn source_stepping_recovers_after_direct_and_gmin_failure() {
        let mut s = diode_bias(20.0, 12);
        let start = s.circuit.0.initial.clone();
        assert!(matches!(
            s.newton(&start, &Problem::default()),
            Err(Error::Convergence { .. })
        ));
        assert!(matches!(
            s.newton(
                &start,
                &Problem {
                    gmin: 1e-2,
                    ..Default::default()
                }
            ),
            Err(Error::Convergence { .. })
        ));
        let solution = s.operating_point().unwrap();
        assert_eq!(s.stats.direct_failures, 1);
        assert_eq!(s.stats.dc_method, Some(DcMethod::SourceStepping));
        assert_eq!(s.stats.gmin_attempts, 1);
        assert!(s.stats.source_attempts > 1);
        assert_eq!(
            s.stats.continuation_attempts,
            s.stats.gmin_attempts + s.stats.source_attempts
        );
        assert_original_diode_equations(&s, &solution.values, 20.0);
        let mut reference = diode_bias(20.0, 200);
        let reference = reference.operating_point().unwrap();
        for (a, b) in solution.values.iter().zip(reference.values) {
            assert!((a - b).abs() < 1e-9);
        }
    }

    fn meyer_inverter() -> Simulator {
        let mut b = CircuitBuilder::new();
        let rail = b.node("rail");
        let input = b.node("input");
        let output = b.node("output");
        let g = NodeId::GROUND;
        b.add("vdd", &[rail, g], Element::Voltage(5.0.into()))
            .unwrap();
        b.add(
            "vin",
            &[input, g],
            Element::Voltage(Source {
                dc: 0.0,
                ac: Default::default(),
                waveform: Some(Waveform::Pwl(vec![(0.0, 0.0), (100e-9, 5.0)])),
            }),
        )
        .unwrap();
        for (name, nodes, kind, threshold, kp, width) in [
            (
                "mn",
                [output, input, g, g],
                ModelKind::Nmos,
                1.0,
                100e-6,
                10e-6,
            ),
            (
                "mp",
                [output, input, rail, rail],
                ModelKind::Pmos,
                -1.0,
                50e-6,
                20e-6,
            ),
        ] {
            let model = Model::new(kind)
                .parameter("vto", threshold)
                .unwrap()
                .parameter("kp", kp)
                .unwrap()
                .parameter("tox", 100e-9)
                .unwrap();
            b.add(
                name,
                &nodes,
                Element::Semiconductor {
                    model,
                    geometry: Geometry {
                        length: 1e-6,
                        width,
                        ..Default::default()
                    },
                },
            )
            .unwrap();
        }
        b.add(
            "load",
            &[output, g],
            Element::Capacitor {
                capacitance: 10e-12,
                ic: None,
            },
        )
        .unwrap();
        let mut s = b.build().compile().unwrap().simulator();
        s.options = SolverOptions::precise();
        s
    }

    #[test]
    fn rejected_trials_preserve_accepted_meyer_and_bdf_histories() {
        for method in [
            IntegrationMethod::BackwardEuler,
            IntegrationMethod::Trapezoidal,
            IntegrationMethod::Bdf2,
        ] {
            let mut s = meyer_inverter();
            let initial = s.initialize(false).unwrap();
            let first = s
                .take_step(&initial, 1e-9, IntegrationMethod::BackwardEuler)
                .unwrap();
            let accepted = s.take_step(&first, 1e-9, method).unwrap();
            let snapshot = accepted.clone();
            assert!(accepted.previous_q.is_some());
            assert!(
                accepted
                    .meyer
                    .iter()
                    .flatten()
                    .any(|m| m.caps.iter().any(|c| *c > 0.0))
            );
            let (_, error, _) = s.estimate_step(&accepted, 80e-9, method).unwrap();
            assert!(
                error > 1.0,
                "oversized trial must be rejected, error={error}, method={method:?}"
            );
            assert_eq!(
                accepted, snapshot,
                "LTE rejection must leave every accepted field unchanged"
            );
            let iterations = s.options.max_iterations;
            s.options.max_iterations = 1;
            assert!(matches!(
                s.estimate_step(&accepted, 20e-9, method),
                Err(Error::Convergence { .. })
            ));
            assert_eq!(
                accepted, snapshot,
                "Newton failure must not leak partial trial state"
            );
            s.options.max_iterations = iterations;
            let retry = s.take_step(&accepted, 0.5e-9, method).unwrap();
            let mut control = meyer_inverter();
            let initial = control.initialize(false).unwrap();
            let first = control
                .take_step(&initial, 1e-9, IntegrationMethod::BackwardEuler)
                .unwrap();
            let clean = control.take_step(&first, 1e-9, method).unwrap();
            let expected = control.take_step(&clean, 0.5e-9, method).unwrap();
            assert_eq!(
                retry, expected,
                "retry after rejected trials must match a clean run"
            );
        }
    }
}

#[cfg(test)]
mod excess_phase_history_tests {
    use super::*;
    fn amplifier() -> Simulator {
        let mut b = CircuitBuilder::new();
        let rail = b.node("rail");
        let base = b.node("base");
        let collector = b.node("collector");
        let g = NodeId::GROUND;
        b.add("vcc", &[rail, g], Element::Voltage(5.0.into()))
            .unwrap();
        b.add(
            "vin",
            &[base, g],
            Element::Voltage(Source {
                dc: 0.65,
                ac: Default::default(),
                waveform: Some(Waveform::Sin {
                    offset: 0.65,
                    amplitude: 0.001,
                    frequency: 20e6,
                    delay: 0.0,
                    damping: 0.0,
                    phase: 0.0,
                }),
            }),
        )
        .unwrap();
        b.add("r", &[rail, collector], Element::Resistor(1000.0))
            .unwrap();
        b.add(
            "c",
            &[collector, g],
            Element::Capacitor {
                capacitance: 2e-12,
                ic: None,
            },
        )
        .unwrap();
        b.add(
            "q",
            &[collector, base, g, g],
            Element::Semiconductor {
                model: Model::new(ModelKind::Npn)
                    .parameter("is", 1e-14)
                    .unwrap()
                    .parameter("tf", 20e-9)
                    .unwrap()
                    .parameter("ptf", 45.0)
                    .unwrap(),
                geometry: Default::default(),
            },
        )
        .unwrap();
        let mut s = b.build().compile().unwrap().simulator();
        s.options = SolverOptions::precise();
        s
    }
    #[test]
    fn excess_phase_history_survives_discarded_and_failed_trials() {
        let mut s = amplifier();
        let initial = s.initialize(false).unwrap();
        let accepted = s
            .take_step(&initial, 1e-9, IntegrationMethod::BackwardEuler)
            .unwrap();
        let snapshot = accepted.clone();
        assert!(
            accepted
                .phase
                .iter()
                .flatten()
                .any(|p| p.filtered != p.previous)
        );
        let _ = s.estimate_step(&accepted, 20e-9, IntegrationMethod::Trapezoidal);
        assert_eq!(accepted, snapshot);
        s.options.max_iterations = 1;
        assert!(
            s.estimate_step(&accepted, 20e-9, IntegrationMethod::Trapezoidal)
                .is_err()
        );
        assert_eq!(accepted, snapshot);
        s.options = SolverOptions::precise();
        let retry = s
            .take_step(&accepted, 0.25e-9, IntegrationMethod::Trapezoidal)
            .unwrap();
        let mut control = amplifier();
        let initial = control.initialize(false).unwrap();
        let accepted = control
            .take_step(&initial, 1e-9, IntegrationMethod::BackwardEuler)
            .unwrap();
        let expected = control
            .take_step(&accepted, 0.25e-9, IntegrationMethod::Trapezoidal)
            .unwrap();
        assert_eq!(retry, expected);
    }
}

#[cfg(test)]
mod precision_floor_tests {
    use super::*;
    #[test]
    fn sub_ulp_charge_updates_do_not_collapse_the_timestep_or_hide_source_errors() {
        let mut b = CircuitBuilder::new();
        let input = b.node("in");
        let output = b.node("out");
        let ground = NodeId::GROUND;
        b.add("v", &[input, ground], Element::Voltage(2.0.into()))
            .unwrap();
        b.add("r", &[input, output], Element::Resistor(1000.0))
            .unwrap();
        b.add(
            "c",
            &[output, ground],
            Element::Capacitor {
                capacitance: 1.0,
                ic: None,
            },
        )
        .unwrap();
        b.initial_voltage(output, 1.0).unwrap();
        let circuit = b.build().compile().unwrap();
        let mut s = circuit.simulator();
        s.options = SolverOptions::precise();
        let history = s.initialize(false).unwrap();
        let next = s
            .take_step(&history, 1e-15, IntegrationMethod::BackwardEuler)
            .unwrap();
        let index = circuit
            .signals()
            .iter()
            .position(|n| n == "v(out)")
            .unwrap();
        // Exact change is about 1e-18 V, below one ULP at the 1 V bias.
        assert_eq!(next.x[index], 1.0);
        assert!(s.stats.roundoff_limited_convergences > 0);
        let mut bad = history.clone();
        let input = circuit.signals().iter().position(|n| n == "v(in)").unwrap();
        bad.x[input] = 1.0;
        s.options.max_iterations = 1;
        assert!(
            matches!(
                s.take_step(&bad, 1e-15, IntegrationMethod::BackwardEuler),
                Err(Error::Convergence { .. })
            ),
            "an unresolved ideal-source constraint must still fail"
        );
    }
}
