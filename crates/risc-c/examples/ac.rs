//! Run with: cargo run -p risc-c --example ac
use risc_c::{AnalysisData, run_str};

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let output = run_str(
        "RC filter\nV1 in 0 DC 0 AC 1\nR1 in out 1k\nC1 out 0 1u\n.ac lin 1 159.15494309189535 159.15494309189535\n.end",
    )?;
    let AnalysisData::Ac(samples) = &output.analyses[0].data else {
        return Err("expected AC samples".into());
    };
    let gain = samples[0]
        .get(&output.circuit, "v(out)")
        .ok_or("missing v(out)")?;
    assert!((gain.norm() - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9);
    println!(
        "f = {:.3} Hz, gain = {:.6}, phase = {:.3} degrees",
        samples[0].frequency,
        gain.norm(),
        gain.arg().to_degrees()
    );
    Ok(())
}
