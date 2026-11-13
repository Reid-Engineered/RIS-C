pub mod legacy;
use spice_core::*;
use spice_netlist::{Analysis, parse_str};

pub const STIFF_NETLIST: &str = include_str!("../../../../examples/stiff-diode.cir");

/// Same refinement fixture is run with and without the external oracle.
pub fn stiff_diode_refinement() -> (CompiledCircuit, Vec<Sample>) {
    let text = STIFF_NETLIST;
    let parsed = parse_str(text).unwrap();
    let circuit = parsed.circuit.compile().unwrap();
    let mut options = parsed
        .analyses
        .iter()
        .find_map(|a| match a {
            Analysis::Transient { options, .. } => Some(options.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(options.method, IntegrationMethod::Bdf2);
    let mut s = circuit.simulator();
    s.options = SolverOptions::precise();
    let coarse = s.transient(&options).unwrap();
    assert!(s.stats.breakpoints >= 1);
    assert!(s.stats.minimum_step.unwrap() < 1e-9);
    options.max_step = 5e-6;
    s.options.transient_reltol = 1e-5;
    let fine = s.transient(&options).unwrap();
    assert_eq!(coarse.len(), fine.len());
    for (a, b) in coarse.iter().zip(&fine) {
        assert_eq!(a.axis, b.axis);
        for signal in ["v(fast)", "v(slow)"] {
            let av = a.get(&circuit, signal).unwrap();
            let bv = b.get(&circuit, signal).unwrap();
            assert!(av.is_finite() && bv.is_finite());
            assert!(
                (av - bv).abs() <= 1e-6 + 1e-3 * av.abs().max(bv.abs()),
                "step refinement changed {signal}: {av} vs {bv}"
            );
        }
    }
    let last = coarse.last().unwrap();
    assert!(last.get(&circuit, "v(fast)").unwrap() > 0.6);
    assert!(last.get(&circuit, "v(slow)").unwrap() > 0.5);
    (circuit, coarse)
}
