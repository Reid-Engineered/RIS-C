//! Explicit classic electrical parameter inventory. Later ngspice extensions are separate.
use std::collections::BTreeMap;
const D: &str = "is=1e-14 rs=2 n=1.1 tt=5e-9 cjo=1e-11 vj=0.8 m=0.4 eg=1.11 xti=3 fc=0.4 bv=5.1 ibv=1e-3 tnom=27";
const Q: &str = "is=1e-14 bf=150 nf=1.05 vaf=100 ikf=0.05 ise=1e-15 ne=1.5 br=2 nr=1.1 var=50 ikr=0.05 isc=1e-15 nc=2 rb=100 irb=1e-4 rbm=10 re=1 rc=1 cje=1e-11 vje=0.75 mje=0.33 tf=2e-9 xtf=1 vtf=10 itf=0.01 ptf=30 cjc=3e-12 vjc=0.75 mjc=0.33 xcjc=0.5 tr=5e-9 cjs=2e-12 vjs=0.75 mjs=0.3 xtb=0.5 eg=1.11 xti=3 fc=0.5 tnom=27";
const M: &str = "vto=1 kp=1e-4 gamma=0.3 phi=0.6 lambda=0.02 rd=2 rs=2 cbd=1e-12 cbs=1e-12 is=1e-14 pb=0.8 cgso=1e-10 cgdo=1e-10 cgbo=1e-10 rsh=20 cj=1e-4 mj=0.5 cjsw=1e-10 mjsw=0.5 js=1e-8 tox=1e-7 ld=5e-8 uo=600 fc=0.5 nsub=1e16 tpg=1 nss=1e10 tnom=27";
/// A baseline followed by one-at-a-time perturbations for every canonical parameter.
pub fn variants() -> Vec<(String, String, String)> {
    let mut cases = Vec::new();
    for (kind, source) in [("D", D), ("NPN", Q), ("PNP", Q), ("NMOS", M), ("PMOS", M)] {
        let mut params: BTreeMap<String, f64> = source
            .split_whitespace()
            .map(|s| {
                let (k, v) = s.split_once('=').unwrap();
                (k.into(), v.parse().unwrap())
            })
            .collect();
        if kind == "PMOS" {
            params.insert("vto".into(), -1.0);
        }
        let changes: Vec<_> = std::iter::once(("baseline".into(), 0.0))
            .chain(params.iter().map(|(k, v)| {
                (
                    k.clone(),
                    if k == "tnom" {
                        v + 10.0
                    } else if k == "tpg" {
                        0.0
                    } else {
                        v * 1.15
                    },
                )
            }))
            .collect();
        for (parameter, value) in changes {
            let mut card = params.clone();
            if parameter != "baseline" {
                card.insert(parameter.clone(), value);
            }
            let fields = card
                .iter()
                .map(|(k, v)| format!("{k}={v:.16e}"))
                .collect::<Vec<_>>()
                .join(" ");
            cases.push((
                kind.into(),
                parameter,
                format!(".model device {kind}({fields})"),
            ));
        }
    }
    cases
}
