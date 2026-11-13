//! Device parameters and local terminal equations.
//! Equations adapted from ngspice 47; see THIRD_PARTY_NOTICES and reference/PROVENANCE.md.
use crate::{Error, Result, Source};
use peroxide::fuga::{ExpLogOps, Jet, PowOps, TrigOps};
use std::collections::BTreeMap;
pub(crate) type J = Jet<1>;
fn c(v: f64) -> J {
    J::constant(v)
}
fn positive(x: J, floor: f64) -> J {
    if x.value() > floor { x } else { c(floor) }
}
fn exp(x: J) -> J {
    // Linear exponential extension prevents overflow without a flat derivative.
    if x.value() > 700.0 {
        (x - 700.0 + 1.0) * 700.0_f64.exp()
    } else {
        x.exp()
    }
}
fn junction(v: J, is: f64, nvt: f64) -> J {
    if v.value() < -3.0 * nvt {
        -is * (1.0 + (3.0 * nvt / (v * std::f64::consts::E)).powi(3))
    } else {
        is * (exp(v / nvt) - 1.0)
    }
}
fn depletion(v: J, cap: f64, potential: f64, grading: f64, fc: f64) -> J {
    if cap == 0.0 {
        return c(0.0);
    }
    let vf = potential * fc;
    if v.value() < vf {
        if (1.0 - grading).abs() < 1e-12 {
            -potential * cap * (1.0 - v / potential).ln()
        } else {
            potential * cap * (1.0 - (1.0 - v / potential).powf(1.0 - grading)) / (1.0 - grading)
        }
    } else {
        let a = 1.0 - fc;
        let qf = if (1.0 - grading).abs() < 1e-12 {
            -potential * cap * a.ln()
        } else {
            potential * cap * (1.0 - a.powf(1.0 - grading)) / (1.0 - grading)
        };
        let cf = cap * a.powf(-grading);
        let d = v - vf;
        qf + cf * d + 0.5 * cf * grading / (potential * a) * d * d
    }
}
const KQ: f64 = 1.38064852e-23 / 1.6021766208e-19;
fn bandgap(t: f64) -> f64 {
    1.16 - 7.02e-4 * t * t / (t + 1108.0)
}
fn pb_factor(t: f64) -> f64 {
    -2.0 * KQ
        * t
        * (1.5 * (t / 300.15).ln() - bandgap(t) / (2.0 * KQ * t) + 1.1150877 / (2.0 * KQ * 300.15))
}
fn thermal_junction(cap: f64, potential: f64, m: f64, t: f64, tn: f64) -> (f64, f64) {
    let pbo = (potential - pb_factor(tn)) / (tn / 300.15);
    let new_potential = t / 300.15 * pbo + pb_factor(t);
    let old = (potential - pbo) / pbo;
    let new = (new_potential - pbo) / pbo;
    (
        cap * (1.0 + m * (4e-4 * (t - 300.15) - new)) / (1.0 + m * (4e-4 * (tn - 300.15) - old)),
        new_potential,
    )
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    Diode,
    Npn,
    Pnp,
    Nmos,
    Pmos,
}
#[derive(Clone, Debug)]
pub struct Model {
    pub kind: ModelKind,
    params: BTreeMap<String, f64>,
}
impl Model {
    pub fn new(kind: ModelKind) -> Self {
        Self {
            kind,
            params: BTreeMap::new(),
        }
    }
    pub fn parameter(mut self, name: &str, value: f64) -> Result<Self> {
        self.set(name, value)?;
        Ok(self)
    }
    pub fn set(&mut self, name: &str, value: f64) -> Result<()> {
        let lower = name.to_ascii_lowercase();
        let alias = match (self.kind, lower.as_str()) {
            (_, "tref") => "tnom",
            (ModelKind::Diode, "js") => "is",
            (ModelKind::Diode, "cj" | "cj0") => "cjo",
            (ModelKind::Diode, "pb") => "vj",
            (ModelKind::Diode, "mj") => "m",
            (ModelKind::Npn | ModelKind::Pnp, "va") => "vaf",
            (ModelKind::Npn | ModelKind::Pnp, "vb") => "var",
            (ModelKind::Npn | ModelKind::Pnp, "ik") => "ikf",
            (ModelKind::Npn | ModelKind::Pnp, "c2") => "ise",
            (ModelKind::Npn | ModelKind::Pnp, "c4") => "isc",
            (ModelKind::Npn | ModelKind::Pnp, "pe") => "vje",
            (ModelKind::Npn | ModelKind::Pnp, "me") => "mje",
            (ModelKind::Npn | ModelKind::Pnp, "pc") => "vjc",
            (ModelKind::Npn | ModelKind::Pnp, "mc") => "mjc",
            (ModelKind::Npn | ModelKind::Pnp, "ccs" | "csub") => "cjs",
            (ModelKind::Npn | ModelKind::Pnp, "ps") => "vjs",
            (ModelKind::Npn | ModelKind::Pnp, "ms") => "mjs",
            (_, name) => name,
        };
        let name: String = match alias {
            "u0" => "uo".into(),
            "vt0" => "vto".into(),
            s => s.into(),
        };
        let allowed = match self.kind {
            ModelKind::Diode => "is n rs cjo vj m fc tt bv ibv eg xti tnom",
            ModelKind::Npn | ModelKind::Pnp => {
                "is bf br nf nr vaf var ikf ikr ise isc ne nc rb rbm irb rc re iss ns cje vje mje cjc vjc mjc xcjc cjs vjs mjs tf tr xtf vtf itf ptf fc eg xti xtb subs tnom"
            }
            ModelKind::Nmos | ModelKind::Pmos => {
                "vto kp gamma phi lambda rd rs cbd cbs cj cjsw mj mjsw pb fc is js cgso cgdo cgbo tox ld uo nsub tpg nss rsh tnom"
            }
        };
        if !value.is_finite() || !allowed.split_whitespace().any(|s| s == name) {
            return Err(Error::Circuit(format!(
                "unsupported or invalid {:?} model parameter {name}",
                self.kind
            )));
        }
        if ((name == "fc" && !(0.0..1.0).contains(&value))
            || (name == "xcjc" && !(0.0..=1.0).contains(&value)))
            || (name == "tnom" && value <= -273.15)
        {
            return Err(Error::Circuit(format!("invalid model parameter {name}")));
        }
        let signed = ["vto", "tnom", "tpg", "subs"];
        if !signed.contains(&name.as_str()) && value < 0.0 {
            return Err(Error::Circuit(format!("negative model parameter {name}")));
        }
        if [
            "n", "nf", "nr", "ne", "nc", "bf", "br", "vj", "vje", "vjc", "vjs", "pb", "phi", "tox",
            "eg", "ns",
        ]
        .contains(&name.as_str())
            && value == 0.0
        {
            return Err(Error::Circuit(format!("zero model parameter {name}")));
        }
        if name == "tpg" && ![-1.0, 0.0, 1.0].contains(&value) {
            return Err(Error::Circuit("TPG must be -1, 0 or 1".into()));
        }
        if name == "subs" && ![-1.0, 1.0].contains(&value) {
            return Err(Error::Circuit("SUBS must be -1 or 1".into()));
        }
        self.params.insert(name, value);
        Ok(())
    }
    pub fn get(&self, name: &str, default: f64) -> f64 {
        self.params.get(name).copied().unwrap_or(default)
    }
    pub fn parameters(&self) -> &BTreeMap<String, f64> {
        &self.params
    }
    fn polarity(&self) -> f64 {
        if matches!(self.kind, ModelKind::Pnp | ModelKind::Pmos) {
            -1.0
        } else {
            1.0
        }
    }
    /// Resolve MOS1 process parameters using the ngspice nominal-temperature rules.
    pub(crate) fn prepared(&self) -> Result<Self> {
        let mut model = self.clone();
        if matches!(self.kind, ModelKind::Nmos | ModelKind::Pmos) && self.params.contains_key("tox")
        {
            let cox = 3.45314379969e-11 / self.get("tox", 1e-7);
            if !self.params.contains_key("kp") {
                model
                    .params
                    .insert("kp".into(), self.get("uo", 600.0) * 1e-4 * cox);
            }
            if let Some(&doping) = self.params.get("nsub") {
                let doping = doping * 1e6;
                if doping <= 1.45e16 {
                    return Err(Error::Circuit(
                        "NSUB must exceed silicon intrinsic carrier density".into(),
                    ));
                }
                let tn = self.get("tnom", 27.0) + 273.15;
                let p = self.polarity();
                let phi = self.get("phi", (2.0 * KQ * tn * (doping / 1.45e16).ln()).max(0.1));
                let gamma = self.get(
                    "gamma",
                    (2.0 * 11.70 * 8.854214871e-12 * 1.6021766208e-19 * doping).sqrt() / cox,
                );
                let tpg = self.get("tpg", 1.0);
                let work_function = if tpg == 0.0 {
                    3.2
                } else {
                    3.25 + 0.5 * bandgap(tn) - p * tpg * 0.5 * bandgap(tn)
                };
                let flatband = work_function
                    - (3.25 + 0.5 * bandgap(tn) + p * 0.5 * phi)
                    - self.get("nss", 0.0) * 1e4 * 1.6021766208e-19 / cox;
                model.params.insert("phi".into(), phi);
                model.params.insert("gamma".into(), gamma);
                if !self.params.contains_key("vto") {
                    model
                        .params
                        .insert("vto".into(), flatband + p * (gamma * phi.sqrt() + phi));
                }
            }
        }
        Ok(model)
    }
    fn mos_temperature(&self, temp: f64) -> (f64, f64) {
        let t = temp + 273.15;
        let tn = self.get("tnom", 27.0) + 273.15;
        let p = self.polarity();
        let phi0 = self.get("phi", 0.6);
        let phi = t / 300.15 * ((phi0 - pb_factor(tn)) / (tn / 300.15)) + pb_factor(t);
        let vto = self.get("vto", 0.0) - p * self.get("gamma", 0.0) * phi0.sqrt()
            + 0.5 * (bandgap(tn) - bandgap(t))
            + p * 0.5 * (phi - phi0)
            + p * self.get("gamma", 0.0) * phi.sqrt();
        (phi, vto)
    }
    /// Half intrinsic Meyer capacitances, ordered G-D, G-S, G-B.
    pub(crate) fn meyer(&self, v: [f64; 5], geom: Geometry, temp: f64) -> Option<[f64; 3]> {
        if !matches!(self.kind, ModelKind::Nmos | ModelKind::Pmos)
            || !self.params.contains_key("tox")
        {
            return None;
        }
        let p = self.polarity();
        let reverse = p * (v[0] - v[2]) < 0.0;
        let (drain, source) = if reverse { (2, 0) } else { (0, 2) };
        let gs = p * (v[1] - v[source]);
        let gd = p * (v[1] - v[drain]);
        let bs = p * (v[3] - v[source]);
        let (phi, vto) = self.mos_temperature(temp);
        let root = if bs <= 0.0 {
            (phi - bs).sqrt()
        } else {
            (phi.sqrt() - bs / (2.0 * phi.sqrt())).max(0.0)
        };
        let von = p * vto + self.get("gamma", 0.0) * (root - phi.sqrt());
        let over = gs - von;
        let sat = over.max(0.025);
        let ds = gs - gd;
        let cox = 3.45314379969e-11 / self.get("tox", 1e-7)
            * geom.width
            * (geom.length - 2.0 * self.get("ld", 0.0))
            * geom.multiplier;
        let (mut cs, mut cd, cb);
        if over <= -phi {
            cs = 0.0;
            cd = 0.0;
            cb = cox * 0.5;
        } else if over <= -phi * 0.5 {
            cs = 0.0;
            cd = 0.0;
            cb = -over * cox / (2.0 * phi);
        } else {
            cb = if over <= 0.0 {
                -over * cox / (2.0 * phi)
            } else {
                0.0
            };
            cs = if over <= 0.0 {
                over * cox / (1.5 * phi) + cox / 3.0
            } else {
                cox / 3.0
            };
            cd = 0.0;
            if ds < sat {
                let denom = (2.0 * sat - ds).powi(2);
                cd = cs * (1.0 - sat * sat / denom);
                cs *= 1.0 - (sat - ds).powi(2) / denom;
            }
        }
        Some(if reverse { [cs, cd, cb] } else { [cd, cs, cb] })
    }
    pub(crate) fn phase_delay(&self) -> f64 {
        self.get("tf", 0.0) * self.get("ptf", 0.0).to_radians()
    }
    pub(crate) fn bjt_transport(&self, v: [J; 5], temp: f64, geom: Geometry) -> (J, J, J) {
        let p = self.polarity();
        let be = p * (v[1] - v[2]);
        let bc = p * (v[1] - v[0]);
        let vt = KQ * (temp + 273.15);
        let ratio = (temp + 273.15) / (self.get("tnom", 27.0) + 273.15);
        let area = geom.area * geom.multiplier;
        let sat = self.get("is", 1e-16)
            * area
            * ratio.powf(self.get("xti", 3.0))
            * ((ratio - 1.0) * self.get("eg", 1.11) / vt).exp();
        let f = junction(be, sat, self.get("nf", 1.0) * vt);
        let r = junction(bc, sat, self.get("nr", 1.0) * vt);
        let inv = |key| {
            let a = self.get(key, 0.0);
            if a == 0.0 { 0.0 } else { 1.0 / a }
        };
        let q1 = 1.0 / positive(1.0 - bc * inv("vaf") - be * inv("var"), 1e-6);
        let q2 = f * inv("ikf") / area + r * inv("ikr") / area;
        let qb = q1 * (1.0 + positive(1.0 + 4.0 * q2, 1e-30).sqrt()) * 0.5;
        (f, r, qb)
    }
    pub(crate) fn series(&self, geometry: Geometry, _temp: f64) -> Vec<(usize, f64)> {
        match self.kind {
            ModelKind::Diode => vec![(0, self.get("rs", 0.0))],
            ModelKind::Npn | ModelKind::Pnp => vec![
                (0, self.get("rc", 0.0)),
                (1, self.get("rb", 0.0)),
                (2, self.get("re", 0.0)),
            ],
            _ => vec![
                (
                    0,
                    self.get("rd", self.get("rsh", 0.0) * geometry.drain_squares),
                ),
                (
                    2,
                    self.get("rs", self.get("rsh", 0.0) * geometry.source_squares),
                ),
            ],
        }
    }
    /// Electrical currents leaving terminals and terminal storage charges.
    /// Terminal order: diode A/K, BJT C/B/E/S, MOS D/G/S/B.
    pub(crate) fn eval(&self, v: [J; 5], temp: f64, geom: Geometry) -> ([J; 5], [J; 5]) {
        self.eval_mode(v, temp, geom, false)
    }
    pub(crate) fn eval_mode(
        &self,
        v: [J; 5],
        temp: f64,
        geom: Geometry,
        small_signal: bool,
    ) -> ([J; 5], [J; 5]) {
        let mut i = [c(0.0); 5];
        let mut q = i;
        let p = self.polarity();
        let vt = KQ * (temp + 273.15);
        let tn = self.get("tnom", 27.0) + 273.15;
        let ratio = (temp + 273.15) / tn;
        let eg = self.get("eg", 1.11);
        let xti = self.get("xti", 3.0);
        let area = geom.area * geom.multiplier;
        let is0 = self.get(
            "is",
            if matches!(self.kind, ModelKind::Npn | ModelKind::Pnp) {
                1e-16
            } else {
                1e-14
            },
        );
        let scale = ratio.powf(xti) * ((ratio - 1.0) * eg / vt).exp();
        match self.kind {
            ModelKind::Diode => {
                let n = self.get("n", 1.0);
                let is = is0 * area * scale.powf(1.0 / n);
                let vd = v[0] - v[1];
                let mut cur = junction(vd, is, n * vt);
                let bv = self.get("bv", f64::INFINITY);
                if bv.is_finite() && is > 0.0 {
                    let cbv = self.get("ibv", 1e-3) * geom.multiplier;
                    let mut xbv = bv;
                    if cbv >= is * bv / vt {
                        xbv = bv - n * vt * (1.0 + cbv / is).ln();
                        for _ in 0..25 {
                            xbv = bv - n * vt * (cbv / is + 1.0 - xbv / vt).ln();
                        }
                    }
                    if vd.value() < -xbv {
                        cur = -is * exp((-vd - xbv) / (n * vt));
                    }
                }
                let diffusion = cur;
                cur = cur + 1e-12 * vd;
                let (cap, pot) = thermal_junction(
                    self.get("cjo", 0.0) * area,
                    self.get("vj", 1.0),
                    self.get("m", 0.5),
                    temp + 273.15,
                    tn,
                );
                let charge = depletion(vd, cap, pot, self.get("m", 0.5), self.get("fc", 0.5))
                    + self.get("tt", 0.0) * diffusion;
                i[0] = cur;
                i[1] = -cur;
                q[0] = charge;
                q[1] = -charge;
            }
            ModelKind::Npn | ModelKind::Pnp => {
                let be = p * (v[1] - v[2]);
                let bc = p * (v[1] - v[0]);
                let (f, r, qb) = self.bjt_transport(v, temp, geom);
                let beta_scale = ratio.powf(self.get("xtb", 0.0));
                let bfe = f / (self.get("bf", 100.0) * beta_scale)
                    + junction(
                        be,
                        self.get("ise", 0.0) * area * scale.powf(1.0 / self.get("ne", 1.5))
                            / beta_scale,
                        self.get("ne", 1.5) * vt,
                    )
                    + 1e-12 * be;
                let brc = r / (self.get("br", 1.0) * beta_scale)
                    + junction(
                        bc,
                        self.get("isc", 0.0) * area * scale.powf(1.0 / self.get("nc", 2.0))
                            / beta_scale,
                        self.get("nc", 2.0) * vt,
                    )
                    + 1e-12 * bc;
                let transport = (f - r) / qb;
                let rb = self.get("rb", 0.0);
                let rbm = self.get("rbm", rb);
                if rb > 0.0 {
                    let mut resistance = rbm + (rb - rbm) / qb;
                    let irb = self.get("irb", 0.0) * area;
                    if irb > 0.0 {
                        let current = positive((bfe + brc) / irb, 1e-9);
                        let arg = 14.59025 * current.sqrt()
                            / (2.4317 * (1.0 + (1.0 + 14.59025 * current).sqrt()));
                        let tangent = arg.tan();
                        let difference = if arg.value().abs() < 1e-3 {
                            arg.powi(3) / 3.0
                                + 2.0 * arg.powi(5) / 15.0
                                + 17.0 * arg.powi(7) / 315.0
                        } else {
                            tangent - arg
                        };
                        resistance =
                            rbm + 3.0 * (rb - rbm) * difference / (arg * tangent * tangent);
                    }
                    // ngspice's AC model freezes the bias-dependent base resistance.
                    // Newton uses its derivative to converge the same DC equations.
                    let resistance = if small_signal {
                        c(resistance.value())
                    } else {
                        resistance
                    };
                    let base_current = (v[4] - v[1]) * area / resistance;
                    i[4] = base_current;
                    i[1] = -base_current;
                }
                i[0] = p * (transport - brc);
                i[1] = i[1] + p * (bfe + brc);
                i[2] = -p * (transport + bfe);
                let mut factor = c(1.0);
                if be.value() > 0.0 && self.get("xtf", 0.0) != 0.0 {
                    let mut a = c(self.get("xtf", 0.0));
                    let vtf = self.get("vtf", 0.0);
                    if vtf > 0.0 {
                        a = a * exp(bc / (1.44 * vtf));
                    }
                    let itf = self.get("itf", 0.0) * area;
                    if itf > 0.0 {
                        a = a * (f / (f + itf)).powi(2);
                    }
                    factor = factor + a;
                }
                let (ce, pe) = thermal_junction(
                    self.get("cje", 0.0) * area,
                    self.get("vje", 0.75),
                    self.get("mje", 0.33),
                    temp + 273.15,
                    tn,
                );
                let (cc, pc) = thermal_junction(
                    self.get("cjc", 0.0) * area,
                    self.get("vjc", 0.75),
                    self.get("mjc", 0.33),
                    temp + 273.15,
                    tn,
                );
                let (cs, ps) = thermal_junction(
                    self.get("cjs", 0.0) * area,
                    self.get("vjs", 0.75),
                    self.get("mjs", 0.0),
                    temp + 273.15,
                    tn,
                );
                let qe = depletion(be, ce, pe, self.get("mje", 0.33), self.get("fc", 0.5))
                    + self.get("tf", 0.0) * if be.value() > 0.0 { f * factor / qb } else { f };
                let qc = depletion(
                    bc,
                    cc * self.get("xcjc", 1.0),
                    pc,
                    self.get("mjc", 0.33),
                    self.get("fc", 0.5),
                ) + self.get("tr", 0.0) * r;
                let subs = self.get(
                    "subs",
                    if self.kind == ModelKind::Npn {
                        1.0
                    } else {
                        -1.0
                    },
                );
                let connection = if subs == 1.0 { 0 } else { 1 };
                let substrate_type = p * subs;
                let sub = substrate_type * (v[3] - v[connection]);
                let subcurrent = substrate_type
                    * (junction(
                        sub,
                        self.get("iss", 0.0) * area * scale,
                        self.get("ns", 1.0) * vt,
                    ) + 1e-12 * sub);
                i[3] = i[3] + subcurrent;
                i[connection] = i[connection] - subcurrent;
                let qs = depletion(sub, cs, ps, self.get("mjs", 0.0), 0.0);
                let qbx = depletion(
                    p * (v[4] - v[0]),
                    cc * (1.0 - self.get("xcjc", 1.0)),
                    pc,
                    self.get("mjc", 0.33),
                    self.get("fc", 0.5),
                );
                q[1] = p * (qe + qc);
                q[2] = -p * qe;
                q[3] = substrate_type * qs;
                q[4] = p * qbx;
                q[0] = -p * (qc + qbx);
                q[connection] = q[connection] - substrate_type * qs;
            }
            ModelKind::Nmos | ModelKind::Pmos => {
                let reverse = p * (v[0] - v[2]).value() < 0.0;
                let (drain, source) = if reverse { (2, 0) } else { (0, 2) };
                let ds = p * (v[drain] - v[source]);
                let gs = p * (v[1] - v[source]);
                let bs = p * (v[3] - v[source]);
                let (phi, vto) = self.mos_temperature(temp);
                let root = if bs.value() <= 0.0 {
                    (phi - bs).sqrt()
                } else {
                    positive(phi.sqrt() - bs / (2.0 * phi.sqrt()), 0.0)
                };
                let threshold = p * vto + self.get("gamma", 0.0) * (root - phi.sqrt());
                let over = gs - threshold;
                let kp_default = if self.params.contains_key("tox") {
                    self.get("uo", 600.0) * 1e-4 * 3.45314379969e-11 / self.get("tox", 1e-7)
                } else {
                    2e-5
                };
                let beta = self.get("kp", kp_default) * geom.width
                    / (geom.length - 2.0 * self.get("ld", 0.0))
                    * geom.multiplier
                    * ratio.powf(-1.5);
                let channel = if over.value() <= 0.0 {
                    c(0.0)
                } else if ds.value() < over.value() {
                    beta * (over * ds - 0.5 * ds * ds) * (1.0 + self.get("lambda", 0.0) * ds)
                } else {
                    0.5 * beta * over * over * (1.0 + self.get("lambda", 0.0) * ds)
                };
                i[drain] = p * channel;
                i[source] = -p * channel;
                for (terminal, ar, per, cj) in [
                    (0, geom.drain_area, geom.drain_perimeter, "cbd"),
                    (2, geom.source_area, geom.source_perimeter, "cbs"),
                ] {
                    let bv = p * (v[3] - v[terminal]);
                    let sat = if self.get("js", 0.0) > 0.0
                        && geom.drain_area > 0.0
                        && geom.source_area > 0.0
                    {
                        self.get("js", 0.0) * ar
                    } else {
                        is0
                    };
                    let cur = p
                        * (junction(
                            bv,
                            sat * geom.multiplier
                                * (-bandgap(temp + 273.15) / vt + bandgap(tn) / (KQ * tn)).exp(),
                            vt,
                        ) + 1e-12 * bv);
                    i[3] = i[3] + cur;
                    i[terminal] = i[terminal] - cur;
                    let (cap, pot) = thermal_junction(
                        self.get(cj, self.get("cj", 0.0) * ar) * geom.multiplier,
                        self.get("pb", 0.8),
                        self.get("mj", 0.5),
                        temp + 273.15,
                        tn,
                    );
                    let (side, sidepot) = thermal_junction(
                        self.get("cjsw", 0.0) * per * geom.multiplier,
                        self.get("pb", 0.8),
                        self.get("mjsw", 0.5),
                        temp + 273.15,
                        tn,
                    );
                    let charge = p
                        * (depletion(bv, cap, pot, self.get("mj", 0.5), self.get("fc", 0.5))
                            + depletion(
                                bv,
                                side,
                                sidepot,
                                self.get("mjsw", 0.5),
                                self.get("fc", 0.5),
                            ));
                    q[3] = q[3] + charge;
                    q[terminal] = q[terminal] - charge;
                }
                for (terminal, cap) in [
                    (0, self.get("cgdo", 0.0) * geom.width),
                    (2, self.get("cgso", 0.0) * geom.width),
                    (
                        3,
                        self.get("cgbo", 0.0) * (geom.length - 2.0 * self.get("ld", 0.0)),
                    ),
                ] {
                    let charge = cap * geom.multiplier * (v[1] - v[terminal]);
                    q[1] = q[1] + charge;
                    q[terminal] = q[terminal] - charge;
                }
            }
        }
        (i, q)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub area: f64,
    pub multiplier: f64,
    pub width: f64,
    pub length: f64,
    pub drain_area: f64,
    pub source_area: f64,
    pub drain_perimeter: f64,
    pub source_perimeter: f64,
    pub drain_squares: f64,
    pub source_squares: f64,
    pub temperature: Option<f64>,
    pub delta_temperature: f64,
    pub off: bool,
    pub initial_voltages: [Option<f64>; 3],
}
impl Geometry {
    pub fn temperature_at(&self, circuit_temperature: f64) -> f64 {
        self.temperature
            .unwrap_or(circuit_temperature + self.delta_temperature)
    }
}
impl Default for Geometry {
    fn default() -> Self {
        Self {
            area: 1.0,
            multiplier: 1.0,
            width: 100e-6,
            length: 100e-6,
            drain_area: 0.0,
            source_area: 0.0,
            drain_perimeter: 0.0,
            source_perimeter: 0.0,
            drain_squares: 1.0,
            source_squares: 1.0,
            temperature: None,
            delta_temperature: 0.0,
            off: false,
            initial_voltages: [None; 3],
        }
    }
}
#[derive(Clone, Debug)]
pub enum Element {
    Resistor(f64),
    Capacitor { capacitance: f64, ic: Option<f64> },
    Inductor { inductance: f64, ic: Option<f64> },
    Voltage(Source),
    Current(Source),
    Vcvs(f64),
    Vccs(f64),
    Cccs { control: String, gain: f64 },
    Ccvs { control: String, gain: f64 },
    Semiconductor { model: Model, geometry: Geometry },
}
impl Element {
    pub(crate) fn needs_branch(&self) -> bool {
        matches!(
            self,
            Self::Voltage(_) | Self::Inductor { .. } | Self::Vcvs(_) | Self::Ccvs { .. }
        )
    }
    pub(crate) fn terminals(&self) -> usize {
        match self {
            Self::Vcvs(_) | Self::Vccs(_) => 4,
            Self::Semiconductor { model, .. } => {
                if model.kind == ModelKind::Diode {
                    2
                } else {
                    4
                }
            }
            _ => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn peroxide_jacobians_and_charge_conservation() {
        for (kind, values, params) in [
            (
                ModelKind::Diode,
                [0.4, 0.0, 0.0, 0.0, 0.0],
                vec![("cjo", 1e-12), ("tt", 1e-9)],
            ),
            (
                ModelKind::Npn,
                [2.0, 0.65, 0.0, 0.0, 0.65],
                vec![("cje", 2e-12), ("cjc", 1e-12), ("tf", 1e-9), ("vaf", 100.0)],
            ),
            (
                ModelKind::Nmos,
                [2.0, 2.0, 0.0, -0.2, 0.0],
                vec![
                    ("kp", 100e-6),
                    ("vto", 1.0),
                    ("gamma", 0.2),
                    ("cgso", 1e-10),
                    ("cbd", 1e-12),
                ],
            ),
        ] {
            let mut model = Model::new(kind);
            for (k, v) in params {
                model.set(k, v).unwrap();
            }
            for col in 0..5 {
                let variables =
                    std::array::from_fn(|k| J::new(values[k], [if col == k { 1.0 } else { 0.0 }]));
                let (i, q) = model.eval(variables, 27.0, Geometry::default());
                assert!(i.iter().map(|v| v.value()).sum::<f64>().abs() < 1e-16);
                assert!(q.iter().map(|v| v.value()).sum::<f64>().abs() < 1e-25);
                let eps = 1e-6;
                let mut plus = values;
                let mut minus = values;
                plus[col] += eps;
                minus[col] -= eps;
                let (ip, qp) = model.eval(plus.map(J::constant), 27.0, Geometry::default());
                let (im, qm) = model.eval(minus.map(J::constant), 27.0, Geometry::default());
                for row in 0..5 {
                    let di = (ip[row].value() - im[row].value()) / (2.0 * eps);
                    let dq = (qp[row].value() - qm[row].value()) / (2.0 * eps);
                    assert!((i[row].dx() - di).abs() < 1e-12 + 1e-5 * di.abs());
                    assert!((q[row].dx() - dq).abs() < 1e-22 + 1e-5 * dq.abs());
                }
            }
        }
    }
}
