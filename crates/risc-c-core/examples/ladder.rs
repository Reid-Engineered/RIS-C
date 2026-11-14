//! Reproducible sparse-core benchmark; no native numerical libraries.
use risc_c_core::*;
use std::time::Instant;
fn main() -> Result<()> {
    let count = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "10000".into())
        .parse::<usize>()
        .map_err(|_| Error::Analysis("expected node count".into()))?;
    if !(2..=100_000).contains(&count) {
        return Err(Error::Analysis("node count must be 2..100000".into()));
    }
    let begin = Instant::now();
    let mut b = CircuitBuilder::new();
    let mut previous = b.node("in");
    let z = NodeId::GROUND;
    b.add(
        "v",
        &[previous, z],
        Element::Voltage(Source {
            dc: 1.0,
            ac: Complex64::new(1.0, 0.0),
            waveform: None,
        }),
    )?;
    for k in 1..count {
        let node = b.node(&format!("n{k}"));
        b.add(
            &format!("r{k}"),
            &[previous, node],
            Element::Resistor(1000.0),
        )?;
        b.add(
            &format!("shunt{k}"),
            &[node, z],
            Element::Resistor(10_000.0),
        )?;
        b.add(
            &format!("c{k}"),
            &[node, z],
            Element::Capacitor {
                capacitance: 1e-9,
                ic: None,
            },
        )?;
        previous = node;
    }
    let built = begin.elapsed();
    let begin = Instant::now();
    let c = b.build().compile()?;
    let compilation = begin.elapsed();
    let mut s = c.simulator();
    let begin = Instant::now();
    s.operating_point()?;
    let dc = begin.elapsed();
    let dc_stats = s.stats.clone();
    let begin = Instant::now();
    s.ac(&FrequencyGrid::Linear {
        points: 3,
        start: 10.0,
        stop: 1000.0,
    })?;
    let ac = begin.elapsed();
    println!(
        "unknowns={} build={built:?} compile={compilation:?} dc={dc:?} ac_3_points={ac:?} dc_newton={} dc_factorizations={}",
        c.unknowns(),
        dc_stats.newton_iterations,
        dc_stats.factorizations
    );
    Ok(())
}
