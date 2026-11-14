//! Run with: cargo run -p risc-c --example stream
use risc_c::{Error, TransientOptions, netlist};
use std::ops::ControlFlow;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let parsed =
        netlist::parse_str("RC\nV1 in 0 1\nR1 in out 1k\nC1 out 0 1u\n.tran 100u 5m\n.end")?;
    let circuit = parsed.circuit.compile()?;
    let mut simulator = circuit.simulator();
    simulator.options = parsed.solver_options;
    let mut received = 0;
    let result = simulator.transient_with(&TransientOptions::new(0.005, 0.0001), |sample| {
        assert!(sample.get(&circuit, "v(out)").is_some());
        received += 1;
        // An application can persist samples here, and stop on a user request.
        if received == 3 {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    });
    match result {
        Err(Error::Cancelled) => println!("Cancelled after {received} samples; data is partial."),
        Err(error) => return Err(error.into()),
        Ok(()) => return Err("expected the callback to cancel".into()),
    }
    assert_eq!(received, 3);
    Ok(())
}
