# RIS-C: embed SPICE simulation

One dependency for the circuit API, SPICE parser, and a typed analysis runner.
Runs entirely in Rust; ngspice is only an optional development oracle.

```rust
use risc_c::run_str;

let output = run_str("Divider\nV1 in 0 10\nR1 in out 1k\nR2 out 0 1k\n.op\n.end")?;
let point = output.operating_point().expect("the deck declares .op");
assert!((point.get(&output.circuit, "v(out)").unwrap() - 5.0).abs() < 1e-9);
# Ok::<(), risc_c::RunError>(())
```

Use `run_file(path)` for `.include` support. Both helpers run all declared
`.op`, `.dc`, `.ac`, and `.tran` analyses in order. `RunOutput::analyses` contains
typed `AnalysisData` and per-analysis `RunStats`. Missing analyses, parse errors,
invalid circuits, and solver failures are returned as `RunError`; no partial
result is returned on failure.

For custom options, call `netlist::parse_str` / `parse_file`, adjust the parsed
deck, and pass it to `run`. For direct circuit construction or streaming, use
the re-exported `CircuitBuilder`, `Simulator`, and `TransientOptions` types.

Use SI units in Rust: ohms, farads, henries, volts, amperes, seconds, and hertz.
Netlists also accept SPICE suffixes (`1k`, `1u`, etc.; `M` means milli).
The first netlist line is a title, ground is node `0`, and a deck ends in `.end`.
This implements a documented SPICE subset; check the repository's
[compatibility contract](https://github.com/Reid-Engineered/RIS-C/blob/main/docs/COMPATIBILITY.md)
before importing vendor models.

Install from the Git repository with `cargo add risc-c --git https://github.com/Reid-Engineered/RIS-C`.
Commit your application's `Cargo.lock`; use `--rev <reviewed-commit>` when pinning
the dependency in a reusable library. The import is `risc_c` (underscores).
See the repository's integration guide for complete Rust and subprocess examples.
