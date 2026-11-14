use crate::{device::J, linear::Pattern, *};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);
impl NodeId {
    pub const GROUND: Self = Self(0);
}
#[derive(Clone, Debug)]
pub struct Device {
    pub name: String,
    pub nodes: Vec<NodeId>,
    pub element: Element,
}
#[derive(Clone, Debug)]
pub struct Circuit {
    pub(crate) nodes: Vec<String>,
    pub(crate) devices: Vec<Device>,
    pub(crate) temperature: f64,
    pub(crate) ic: HashMap<NodeId, f64>,
    pub(crate) nodeset: HashMap<NodeId, f64>,
}
#[derive(Clone, Debug)]
pub struct CircuitBuilder {
    circuit: Circuit,
    lookup: HashMap<String, NodeId>,
}
impl Default for CircuitBuilder {
    fn default() -> Self {
        Self::new()
    }
}
impl CircuitBuilder {
    pub fn new() -> Self {
        Self {
            circuit: Circuit {
                nodes: vec!["0".into()],
                devices: vec![],
                temperature: 27.0,
                ic: HashMap::new(),
                nodeset: HashMap::new(),
            },
            lookup: HashMap::from([("0".into(), NodeId::GROUND)]),
        }
    }
    pub fn node(&mut self, name: &str) -> NodeId {
        let name = name.to_ascii_lowercase();
        *self.lookup.entry(name.clone()).or_insert_with(|| {
            let n = NodeId(self.circuit.nodes.len());
            self.circuit.nodes.push(name);
            n
        })
    }
    pub fn add(&mut self, name: &str, nodes: &[NodeId], element: Element) -> Result<()> {
        let name = name.to_ascii_lowercase();
        if name.is_empty() || self.circuit.devices.iter().any(|d| d.name == name) {
            return Err(Error::Circuit(format!("empty or duplicate device {name}")));
        }
        if nodes.len() != element.terminals()
            || nodes.iter().any(|n| n.0 >= self.circuit.nodes.len())
        {
            return Err(Error::Circuit(format!("invalid terminals for {name}")));
        }
        match &element {
            Element::Resistor(v) => error::valid(*v, &name, true)?,
            Element::Capacitor { capacitance, ic } => {
                error::valid(*capacitance, &name, true)?;
                if let Some(v) = ic {
                    error::valid(*v, &name, false)?;
                }
            }
            Element::Inductor { inductance, ic } => {
                error::valid(*inductance, &name, true)?;
                if let Some(v) = ic {
                    error::valid(*v, &name, false)?;
                }
            }
            Element::Mutual { coupling, .. } => {
                if !coupling.is_finite() || coupling.abs() > 1.0 {
                    return Err(Error::Circuit(
                        "coupling coefficient must be in [-1,1]".into(),
                    ));
                }
            }
            Element::VoltageSwitch { model, .. } | Element::CurrentSwitch { model, .. } => {
                model.validate()?
            }
            Element::Voltage(s) | Element::Current(s) => s.validate()?,
            Element::Vcvs(v)
            | Element::Vccs(v)
            | Element::Cccs { gain: v, .. }
            | Element::Ccvs { gain: v, .. } => error::valid(*v, &name, false)?,
            Element::Semiconductor { model, geometry: g } => {
                error::valid(g.delta_temperature, &name, false)?;
                if !g.temperature_at(self.circuit.temperature).is_finite()
                    || g.temperature_at(self.circuit.temperature) <= -273.15
                {
                    return Err(Error::Circuit(
                        "instance temperature must exceed absolute zero".into(),
                    ));
                }
                for v in g.initial_voltages.iter().flatten() {
                    error::valid(*v, &name, false)?;
                }
                if matches!(model.kind, ModelKind::Npn | ModelKind::Pnp)
                    && model.get("rbm", model.get("rb", 0.0)) > model.get("rb", 0.0)
                {
                    return Err(Error::Circuit("RBM must not exceed RB".into()));
                }
                for v in [
                    g.area,
                    g.multiplier,
                    g.width,
                    g.length - 2.0 * model.get("ld", 0.0),
                ] {
                    error::valid(v, &name, true)?;
                }
                for v in [
                    g.drain_area,
                    g.source_area,
                    g.drain_perimeter,
                    g.source_perimeter,
                    g.drain_squares,
                    g.source_squares,
                ] {
                    error::valid(v, &name, false)?;
                    if v < 0.0 {
                        return Err(Error::Circuit("negative device geometry".into()));
                    }
                }
            }
        }
        self.circuit.devices.push(Device {
            name,
            nodes: nodes.to_vec(),
            element,
        });
        Ok(())
    }
    pub fn temperature(&mut self, celsius: f64) -> Result<()> {
        if !celsius.is_finite() || celsius <= -273.15 {
            return Err(Error::Circuit(
                "temperature must exceed absolute zero".into(),
            ));
        }
        self.circuit.temperature = celsius;
        Ok(())
    }
    pub fn initial_voltage(&mut self, node: NodeId, value: f64) -> Result<()> {
        self.hint(node, value, true)
    }
    pub fn nodeset(&mut self, node: NodeId, value: f64) -> Result<()> {
        self.hint(node, value, false)
    }
    fn hint(&mut self, node: NodeId, value: f64, ic: bool) -> Result<()> {
        if node.0 >= self.circuit.nodes.len() || !value.is_finite() || (node.0 == 0 && value != 0.0)
        {
            return Err(Error::Circuit("invalid voltage hint".into()));
        }
        let map = if ic {
            &mut self.circuit.ic
        } else {
            &mut self.circuit.nodeset
        };
        if map.insert(node, value).is_some_and(|v| v != value) {
            return Err(Error::Circuit("conflicting voltage hints".into()));
        }
        Ok(())
    }
    pub fn build(self) -> Circuit {
        self.circuit
    }
}
#[derive(Debug)]
pub(crate) struct CompiledDevice {
    pub name: String,
    pub nodes: Vec<Option<usize>>,
    pub element: Element,
    pub branch: Option<usize>,
    pub control: Option<usize>,
}
#[derive(Debug)]
pub(crate) struct CompiledData {
    pub pattern: Pattern,
    pub devices: Vec<CompiledDevice>,
    pub names: Vec<String>,
    pub nvolt: usize,
    pub temperature: f64,
    pub initial: Vec<f64>,
    pub ic: HashMap<usize, f64>,
    pub mutual: Vec<(usize, usize, f64)>,
    pub has_switches: bool,
    pub has_nonlinear: bool,
}
#[derive(Clone, Debug)]
pub struct CompiledCircuit(pub(crate) Arc<CompiledData>);
impl Circuit {
    pub fn compile(&self) -> Result<CompiledCircuit> {
        let mut nodes = self.nodes.clone();
        let mut devices = Vec::new();
        for d in &self.devices {
            let mut d = d.clone();
            if let Element::Semiconductor { model, .. } = &mut d.element {
                *model = model.prepared()?;
            }
            if let Element::Semiconductor { model, geometry } = &d.element {
                if !geometry.temperature_at(self.temperature).is_finite()
                    || geometry.temperature_at(self.temperature) <= -273.15
                {
                    return Err(Error::Circuit(
                        "instance temperature must exceed absolute zero".into(),
                    ));
                }
                let external_base = d.nodes[1];
                for (terminal, r) in
                    model.series(*geometry, geometry.temperature_at(self.temperature))
                {
                    if r > 0.0 {
                        let old = d.nodes[terminal];
                        let hidden = NodeId(nodes.len());
                        let hidden_name = format!("{}#{}", d.name, terminal);
                        if nodes.contains(&hidden_name) {
                            return Err(Error::Circuit("internal node name collision".into()));
                        }
                        nodes.push(hidden_name);
                        d.nodes[terminal] = hidden;
                        if !(matches!(model.kind, ModelKind::Npn | ModelKind::Pnp) && terminal == 1)
                        {
                            devices.push(Device {
                                name: format!("{}#r{}", d.name, terminal),
                                nodes: vec![old, hidden],
                                element: Element::Resistor(
                                    r / (if matches!(model.kind, ModelKind::Nmos | ModelKind::Pmos)
                                    {
                                        geometry.multiplier
                                    } else {
                                        geometry.area * geometry.multiplier
                                    }),
                                ),
                            });
                        }
                    }
                }
                if matches!(model.kind, ModelKind::Npn | ModelKind::Pnp) {
                    d.nodes.push(external_base);
                }
            }
            devices.push(d);
        }
        let nvolt = nodes.len() - 1;
        let mut names: Vec<_> = nodes[1..].iter().map(|s| format!("v({s})")).collect();
        let mut branches = HashMap::new();
        for d in &devices {
            if d.element.needs_branch() {
                branches.insert(d.name.clone(), names.len());
                names.push(format!("i({})", d.name));
            }
        }
        if names.is_empty() {
            return Err(Error::Circuit("circuit has no unknowns".into()));
        }
        let mutual = crate::coupling::compile(&devices, &branches)?;
        let mut entries: BTreeSet<_> = (0..names.len()).map(|i| (i, i)).collect();
        for &(a, b, _) in &mutual {
            entries.insert((a, b));
            entries.insert((b, a));
        }
        let mut out = Vec::new();
        for d in devices {
            if matches!(d.element, Element::Mutual { .. }) {
                continue;
            }
            let control = match &d.element {
                Element::Cccs { control, .. }
                | Element::Ccvs { control, .. }
                | Element::CurrentSwitch { control, .. } => {
                    Some(*branches.get(&control.to_ascii_lowercase()).ok_or_else(|| {
                        Error::Circuit(format!(
                            "{}: control {control} has no branch current",
                            d.name
                        ))
                    })?)
                }
                _ => None,
            };
            let nd: Vec<_> = d.nodes.iter().map(|n| n.0.checked_sub(1)).collect();
            let branch = branches.get(&d.name).copied();
            let local: Vec<_> = nd
                .iter()
                .copied()
                .flatten()
                .chain(branch)
                .chain(control)
                .collect();
            for &r in &local {
                for &c in &local {
                    entries.insert((r, c));
                }
            }
            out.push(CompiledDevice {
                name: d.name,
                nodes: nd,
                element: d.element,
                branch,
                control,
            });
        }
        let pattern = Pattern::new(names.len(), &entries)?;
        let mut initial = vec![0.0; names.len()];
        for (&node, &value) in &self.nodeset {
            if node.0 != 0 {
                initial[node.0 - 1] = value;
            }
        }
        let ic = self
            .ic
            .iter()
            .filter(|(n, _)| n.0 != 0)
            .map(|(n, v)| (n.0 - 1, *v))
            .collect();
        let has_switches = out.iter().any(|d| {
            matches!(
                d.element,
                Element::VoltageSwitch { .. } | Element::CurrentSwitch { .. }
            )
        });
        let has_nonlinear = out
            .iter()
            .any(|d| matches!(d.element, Element::Semiconductor { .. }));
        Ok(CompiledCircuit(Arc::new(CompiledData {
            pattern,
            devices: out,
            names,
            nvolt,
            temperature: self.temperature,
            initial,
            ic,
            mutual,
            has_switches,
            has_nonlinear,
        })))
    }
}
impl CompiledCircuit {
    pub fn signals(&self) -> &[String] {
        &self.0.names
    }
    pub fn unknowns(&self) -> usize {
        self.0.names.len()
    }
    pub fn simulator(&self) -> Simulator {
        Simulator::new(self.clone())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MeyerState {
    pub voltages: [f64; 3],
    pub caps: [f64; 3],
    pub charges: [f64; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PhaseState {
    pub filtered: f64,
    pub previous: f64,
    pub step: Option<f64>,
}
#[derive(Clone, Copy)]
pub(crate) struct DeviceHistory<'a> {
    pub meyer: &'a [Option<MeyerState>],
    pub phase: &'a [Option<PhaseState>],
    pub step: f64,
    pub switches: &'a [bool],
}
#[derive(Debug)]
pub(crate) struct Evaluation {
    pub f: Vec<f64>,
    pub q: Vec<f64>,
    pub g: Vec<f64>,
    pub c: Vec<f64>,
    pub scale: Vec<f64>,
    pub meyer: Vec<Option<MeyerState>>,
    pub phase: Vec<Option<PhaseState>>,
    pub switches: Vec<bool>,
}
struct EvaluationMode<'a> {
    switches: Option<&'a [bool]>,
    small_signal: bool,
}
impl CompiledData {
    pub fn initial_switches(&self) -> Vec<bool> {
        self.devices
            .iter()
            .map(|d| match d.element {
                Element::VoltageSwitch { initial_on, .. }
                | Element::CurrentSwitch { initial_on, .. } => initial_on,
                _ => false,
            })
            .collect()
    }
    pub fn switch_control(&self, d: &CompiledDevice, x: &[f64]) -> f64 {
        let val = |n: Option<usize>| n.map_or(0.0, |k| x[k]);
        match d.element {
            Element::VoltageSwitch { .. } => val(d.nodes[2]) - val(d.nodes[3]),
            Element::CurrentSwitch { .. } => val(d.control),
            _ => 0.0,
        }
    }
    pub fn next_switches(&self, x: &[f64], previous: &[bool]) -> Vec<bool> {
        self.devices
            .iter()
            .enumerate()
            .map(|(i, d)| match d.element {
                Element::VoltageSwitch { model, .. } | Element::CurrentSwitch { model, .. } => {
                    model.state(self.switch_control(d, x), previous[i])
                }
                _ => false,
            })
            .collect()
    }
    #[cfg(test)]
    pub fn evaluate(
        &self,
        x: &[f64],
        time: Option<f64>,
        source_scale: f64,
        override_source: Option<(&str, f64)>,
    ) -> Result<Evaluation> {
        self.evaluate_context(x, time, source_scale, override_source, None, None)
    }
    pub fn evaluate_context(
        &self,
        x: &[f64],
        time: Option<f64>,
        source_scale: f64,
        override_source: Option<(&str, f64)>,
        previous: Option<DeviceHistory<'_>>,
        switches: Option<&[bool]>,
    ) -> Result<Evaluation> {
        self.evaluate_mode(
            x,
            time,
            source_scale,
            override_source,
            previous,
            EvaluationMode {
                switches,
                small_signal: false,
            },
        )
    }
    pub fn evaluate_ac(&self, x: &[f64], switches: &[bool]) -> Result<Evaluation> {
        self.evaluate_mode(
            x,
            None,
            1.0,
            None,
            None,
            EvaluationMode {
                switches: Some(switches),
                small_signal: true,
            },
        )
    }
    fn evaluate_mode(
        &self,
        x: &[f64],
        time: Option<f64>,
        source_scale: f64,
        override_source: Option<(&str, f64)>,
        previous: Option<DeviceHistory<'_>>,
        mode: EvaluationMode<'_>,
    ) -> Result<Evaluation> {
        let EvaluationMode {
            switches,
            small_signal,
        } = mode;
        let n = self.names.len();
        let mut e = Evaluation {
            f: vec![0.0; n],
            q: vec![0.0; n],
            g: self.pattern.zeros(),
            c: self.pattern.zeros(),
            scale: vec![0.0; n],
            meyer: vec![None; self.devices.len()],
            phase: vec![None; self.devices.len()],
            switches: vec![false; self.devices.len()],
        };
        for (device_index, d) in self.devices.iter().enumerate() {
            let val = |node: Option<usize>| node.map_or(0.0, |i| x[i]);
            let a = d.nodes[0];
            let b = d.nodes[1];
            let v = val(a) - val(b);
            let mut f = |row: Option<usize>, value: f64| {
                if let Some(r) = row {
                    e.f[r] += value;
                    e.scale[r] += value.abs();
                }
            };
            let mut stamp = |row: Option<usize>, col: Option<usize>, value: f64| {
                if let (Some(r), Some(c)) = (row, col) {
                    e.g[self.pattern.slot(r, c)] += value;
                }
            };
            match &d.element {
                Element::VoltageSwitch { model, initial_on }
                | Element::CurrentSwitch {
                    model, initial_on, ..
                } => {
                    let old = switches
                        .or(previous.map(|h| h.switches))
                        .map_or(*initial_on, |s| s[device_index]);
                    // Transient intervals end at localized crossings. Integrate the
                    // interval with its accepted conductance, then commit its end state.
                    let on = if small_signal || previous.is_some() {
                        old
                    } else {
                        model.state(self.switch_control(d, x), old)
                    };
                    e.switches[device_index] = on;
                    let g = if on {
                        model.ron.recip()
                    } else {
                        model.roff.recip()
                    };
                    f(a, g * v);
                    f(b, -g * v);
                    stamp(a, a, g);
                    stamp(a, b, -g);
                    stamp(b, a, -g);
                    stamp(b, b, g);
                }
                Element::Mutual { .. } => unreachable!("K is compiled as cross-flux stamps"),
                Element::Resistor(r) => {
                    f(a, v / r);
                    f(b, -v / r);
                    stamp(a, a, 1.0 / r);
                    stamp(a, b, -1.0 / r);
                    stamp(b, a, -1.0 / r);
                    stamp(b, b, 1.0 / r);
                }
                Element::Capacitor {
                    capacitance: cap, ..
                } => {
                    if let Some(r) = a {
                        e.q[r] += cap * v;
                    }
                    if let Some(r) = b {
                        e.q[r] -= cap * v;
                    }
                    for (row, col, s) in [(a, a, 1.0), (a, b, -1.0), (b, a, -1.0), (b, b, 1.0)] {
                        if let (Some(r), Some(c)) = (row, col) {
                            e.c[self.pattern.slot(r, c)] += s * cap;
                        }
                    }
                }
                Element::Current(s) => {
                    let cur = source_scale
                        * override_source
                            .filter(|(name, _)| *name == d.name)
                            .map_or_else(|| s.at(time), |(_, v)| v);
                    f(a, cur);
                    f(b, -cur);
                }
                Element::Voltage(s) => {
                    let k = d.branch;
                    let src = source_scale
                        * override_source
                            .filter(|(name, _)| *name == d.name)
                            .map_or_else(|| s.at(time), |(_, v)| v);
                    f(a, val(k));
                    f(b, -val(k));
                    f(k, v - src);
                    stamp(a, k, 1.0);
                    stamp(b, k, -1.0);
                    stamp(k, a, 1.0);
                    stamp(k, b, -1.0);
                }
                Element::Inductor { inductance: l, .. } => {
                    let k = d.branch;
                    f(a, val(k));
                    f(b, -val(k));
                    f(k, v);
                    stamp(a, k, 1.0);
                    stamp(b, k, -1.0);
                    stamp(k, a, 1.0);
                    stamp(k, b, -1.0);
                    let k = k.unwrap();
                    e.q[k] -= l * x[k];
                    e.c[self.pattern.slot(k, k)] -= l;
                }
                Element::Vccs(gain) | Element::Vcvs(gain) => {
                    let p = d.nodes[2];
                    let m = d.nodes[3];
                    let ctrl = val(p) - val(m);
                    if matches!(d.element, Element::Vccs(_)) {
                        f(a, gain * ctrl);
                        f(b, -gain * ctrl);
                        stamp(a, p, *gain);
                        stamp(a, m, -gain);
                        stamp(b, p, -gain);
                        stamp(b, m, *gain);
                    } else {
                        let k = d.branch;
                        f(a, val(k));
                        f(b, -val(k));
                        f(k, v - gain * ctrl);
                        stamp(a, k, 1.0);
                        stamp(b, k, -1.0);
                        stamp(k, a, 1.0);
                        stamp(k, b, -1.0);
                        stamp(k, p, -gain);
                        stamp(k, m, *gain);
                    }
                }
                Element::Cccs { gain, .. } | Element::Ccvs { gain, .. } => {
                    let ctrl = d.control;
                    if matches!(d.element, Element::Cccs { .. }) {
                        f(a, gain * val(ctrl));
                        f(b, -gain * val(ctrl));
                        stamp(a, ctrl, *gain);
                        stamp(b, ctrl, -gain);
                    } else {
                        let k = d.branch;
                        f(a, val(k));
                        f(b, -val(k));
                        f(k, v - gain * val(ctrl));
                        stamp(a, k, 1.0);
                        stamp(b, k, -1.0);
                        stamp(k, a, 1.0);
                        stamp(k, b, -1.0);
                        stamp(k, ctrl, -gain);
                    }
                }
                Element::Semiconductor { model, geometry } => {
                    let temperature = geometry.temperature_at(self.temperature);
                    let count = d.nodes.len();
                    for col in 0..count {
                        let mut vv = [J::constant(0.0); 5];
                        for (k, node) in d.nodes.iter().enumerate() {
                            vv[k] = J::new(val(*node), [if col == k { 1.0 } else { 0.0 }]);
                        }
                        let (mut ii, qq) = if small_signal {
                            model.eval_mode(vv, temperature, *geometry, true)
                        } else {
                            model.eval(vv, temperature, *geometry)
                        };
                        if matches!(model.kind, ModelKind::Npn | ModelKind::Pnp)
                            && model.phase_delay() > 0.0
                        {
                            let (f, _, qb) = model.bjt_transport(vv, temperature, *geometry);
                            let forward = f / qb;
                            let old =
                                previous.and_then(|history| history.phase[device_index].as_ref());
                            let filtered = if let (Some(old), Some(history)) = (old, previous) {
                                let a = history.step / model.phase_delay();
                                let ratio = history.step / old.step.unwrap_or(history.step);
                                let denominator = 1.0 + 3.0 * a + 3.0 * a * a;
                                (old.filtered * (1.0 + ratio + 3.0 * a) - old.previous * ratio)
                                    / denominator
                                    + forward * (3.0 * a * a / denominator)
                            } else {
                                forward
                            };
                            let p = if model.kind == ModelKind::Pnp {
                                -1.0
                            } else {
                                1.0
                            };
                            let correction = p * (filtered - forward);
                            ii[0] = ii[0] + correction;
                            ii[2] = ii[2] - correction;
                            if col == 0 {
                                e.phase[device_index] = Some(PhaseState {
                                    filtered: filtered.value(),
                                    previous: old.map_or(forward.value(), |o| o.filtered),
                                    step: previous.map(|h| h.step),
                                });
                            }
                        }
                        for row in 0..count {
                            if let Some(r) = d.nodes[row] {
                                if col == 0 {
                                    e.f[r] += ii[row].value();
                                    e.q[r] += qq[row].value();
                                    e.scale[r] += ii[row].value().abs();
                                }
                                if let Some(c) = d.nodes[col] {
                                    let slot = self.pattern.slot(r, c);
                                    e.g[slot] += ii[row].dx();
                                    e.c[slot] += qq[row].dx();
                                }
                            }
                        }
                    }
                    let mut vv = [0.0; 5];
                    for (k, node) in d.nodes.iter().enumerate() {
                        vv[k] = val(*node);
                    }
                    if let Some(caps) = model.meyer(vv, *geometry, temperature) {
                        let voltages = [vv[1] - vv[0], vv[1] - vv[2], vv[1] - vv[3]];
                        let old = previous.and_then(|history| history.meyer[device_index].as_ref());
                        let mut charges = [0.0; 3];
                        for (j, terminal) in [0, 2, 3].into_iter().enumerate() {
                            let cap = old.map_or(2.0 * caps[j], |old| old.caps[j] + caps[j]);
                            charges[j] = old.map_or(cap * voltages[j], |old| {
                                old.charges[j] + cap * (voltages[j] - old.voltages[j])
                            });
                            let a = d.nodes[1];
                            let b = d.nodes[terminal];
                            if let Some(r) = a {
                                e.q[r] += charges[j];
                            }
                            if let Some(r) = b {
                                e.q[r] -= charges[j];
                            }
                            for (row, col, sign) in
                                [(a, a, 1.0), (a, b, -1.0), (b, a, -1.0), (b, b, 1.0)]
                            {
                                if let (Some(r), Some(c)) = (row, col) {
                                    e.c[self.pattern.slot(r, c)] += sign * cap;
                                }
                            }
                        }
                        e.meyer[device_index] = Some(MeyerState {
                            voltages,
                            caps,
                            charges,
                        });
                    }
                }
            }
        }
        for &(a, b, m) in &self.mutual {
            e.q[a] -= m * x[b];
            e.q[b] -= m * x[a];
            e.c[self.pattern.slot(a, b)] -= m;
            e.c[self.pattern.slot(b, a)] -= m;
        }
        if e.f
            .iter()
            .chain(&e.q)
            .chain(&e.g)
            .chain(&e.c)
            .any(|v| !v.is_finite())
        {
            return Err(Error::NonFinite("device evaluation".into()));
        }
        Ok(e)
    }
    pub fn ac_matrix(&self, e: &Evaluation, x: &[f64], omega: f64) -> Vec<Complex64> {
        let mut matrix: Vec<_> =
            e.g.iter()
                .zip(&e.c)
                .map(|(g, c)| Complex64::new(*g, omega * c))
                .collect();
        for d in &self.devices {
            if let Element::Semiconductor { model, geometry } = &d.element {
                if !matches!(model.kind, ModelKind::Npn | ModelKind::Pnp)
                    || model.phase_delay() == 0.0
                {
                    continue;
                }
                let mut v = [J::constant(0.0); 5];
                for (k, node) in d.nodes.iter().enumerate() {
                    v[k] = J::new(
                        node.map_or(0.0, |i| x[i]),
                        [if k == 0 || k == 1 { 1.0 } else { 0.0 }],
                    );
                }
                let (f, r, qb) =
                    model.bjt_transport(v, geometry.temperature_at(self.temperature), *geometry);
                let p = if model.kind == ModelKind::Pnp {
                    -1.0
                } else {
                    1.0
                };
                let gmplusgo = (p * (f - r) / qb).dx();
                let correction =
                    (Complex64::from_polar(1.0, -omega * model.phase_delay()) - 1.0) * gmplusgo;
                for (row, col, sign) in [(0, 1, 1.0), (0, 2, -1.0), (2, 1, -1.0), (2, 2, 1.0)] {
                    if let (Some(r), Some(c)) = (d.nodes[row], d.nodes[col]) {
                        matrix[self.pattern.slot(r, c)] += sign * correction;
                    }
                }
            }
        }
        matrix
    }
    pub fn ac_rhs(&self) -> Vec<Complex64> {
        let mut b = vec![Complex64::default(); self.names.len()];
        for d in &self.devices {
            match &d.element {
                Element::Voltage(s) => b[d.branch.unwrap()] += s.ac,
                Element::Current(s) => {
                    if let Some(k) = d.nodes[0] {
                        b[k] -= s.ac;
                    }
                    if let Some(k) = d.nodes[1] {
                        b[k] += s.ac;
                    }
                }
                _ => {}
            }
        }
        b
    }
}
