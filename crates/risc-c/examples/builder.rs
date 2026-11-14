//! Run with: cargo run -p risc-c --example builder
use risc_c::{CircuitBuilder, Element, NodeId, Result};

fn main() -> Result<()> {
    let mut builder = CircuitBuilder::new();
    let input = builder.node("in");
    let out = builder.node("out");
    let ground = NodeId::GROUND;
    builder.add("v1", &[input, ground], Element::Voltage(10.0.into()))?;
    builder.add("r1", &[input, out], Element::Resistor(1000.0))?;
    builder.add("r2", &[out, ground], Element::Resistor(1000.0))?;
    let circuit = builder.build().compile()?;
    let point = circuit.simulator().operating_point()?;
    assert!((point.get(&circuit, "v(out)").unwrap() - 5.0).abs() < 1e-9);
    println!("v(out) = {:.3} V", point.get(&circuit, "v(out)").unwrap());
    Ok(())
}
