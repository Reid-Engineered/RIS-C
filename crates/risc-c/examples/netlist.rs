//! Run with: cargo run -p risc-c --example netlist
use risc_c::run_str;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let output = run_str("Divider\nV1 in 0 10\nR1 in out 1k\nR2 out 0 1k\n.op\n.end")?;
    let point = output.operating_point().ok_or("expected .op result")?;
    let voltage = point
        .get(&output.circuit, "v(out)")
        .ok_or("missing v(out)")?;
    assert!((voltage - 5.0).abs() < 1e-9);
    println!("v(out) = {voltage:.3} V");
    Ok(())
}
