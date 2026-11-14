//! Run with: cargo run -p risc-c --example transient
use risc_c::{AnalysisData, SolverOptions, netlist, run};

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut parsed = netlist::parse_str(
        "RC charge\nV1 in 0 PULSE(0 1 1u 10n 10n 10m 20m)\nR1 in out 1k\nC1 out 0 1u\n.tran 100u 5m\n.end",
    )?;
    parsed.solver_options = SolverOptions::precise();
    let output = run(parsed)?;
    let AnalysisData::Transient(samples) = &output.analyses[0].data else {
        return Err("expected transient samples".into());
    };
    let last = samples.last().ok_or("no samples")?;
    let voltage = last
        .get(&output.circuit, "v(out)")
        .ok_or("missing v(out)")?;
    assert!(voltage > 0.99 && voltage < 1.0);
    println!("t = {:.6} s, v(out) = {voltage:.6} V", last.axis);
    Ok(())
}
