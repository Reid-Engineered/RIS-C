# Integrate RIS-C

Start with one Rust dependency or the `spice` executable. The library and CLI
run entirely in Rust. ngspice is an optional development oracle; applications
do not need it installed or linked.

## Install and pin a version

Use Rust 1.91+ with the platform's normal Rust linker. Windows MSVC toolchains
need MSVC Build Tools. Development CI runs on Linux, macOS, and Windows.

Inside an existing Rust project:

```sh
cargo add risc-c --git https://github.com/Reid-Engineered/RIS-C
```

Commit `Cargo.lock` for applications. To pin a reviewed Git commit explicitly:

```sh
cargo add risc-c --git https://github.com/Reid-Engineered/RIS-C --rev <full-commit-sha>
```

Replace `<full-commit-sha>` with an actual commit containing `crates/risc-c`.
The API is version 0.1 and may evolve; update the pin deliberately. Cargo's Git
dependency builds the workspace's internal crates automatically. A crates.io
release is not required for this installation path.

For a vendored checkout, use a relative path in your application's `Cargo.toml`:

```toml
[dependencies]
risc-c = { path = "vendor/RIS-C/crates/risc-c" }
```

For the CLI, use the same reviewed revision when reproducibility matters:

```sh
cargo install --git https://github.com/Reid-Engineered/RIS-C --rev <full-commit-sha> --locked risc-c-cli
spice --version
```

Ensure Cargo's binary directory is on `PATH`, or pass the full executable path
to your subprocess. The package is `risc-c-cli`; its binary is `spice`.

## First simulation

```rust
use risc_c::run_str;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = run_str(
        "Divider\nV1 in 0 10\nR1 in out 1k\nR2 out 0 1k\n.op\n.end",
    )?;
    let point = output.operating_point().ok_or("expected .op result")?;
    let voltage = point.get(&output.circuit, "v(out)").ok_or("missing v(out)")?;
    assert!((voltage - 5.0).abs() < 1e-9);
    println!("v(out) = {voltage:.3} V");
    Ok(())
}
```

Run `cargo run`; expect `v(out) = 5.000 V`. The
[complete source](../crates/risc-c/examples/netlist.rs) is compiled and run by the
independent-consumer CI check.

Use `risc_c::run_file("circuit.cir")` for a deck with `.include` directives.
Includes resolve relative to each including file. `run_str` has no filesystem
include resolver. The first physical line is always the title, ground is node
`0`, and a deck ends with `.end`. Declare at least one analysis.

## Work with results

`run_str`, `run_file`, and `run(parsed)` return `RunOutput`:

- `circuit` owns the shared signal definitions; `circuit.signals()` gives the
  order of every sample's `values` vector.
- `analyses` contains declarations in source order. Each `AnalysisResult` has
  `analysis`, typed `data`, and its own `stats` snapshot.
- `AnalysisData::OperatingPoint` holds one `Sample`.
- `AnalysisData::Dc` holds `Vec<Sample>`; `axis` is the swept source value.
- `AnalysisData::Ac` holds `Vec<AcSample>`; `frequency` is in Hz and each value
  is a complex response. `gain.norm()` gives magnitude; `gain.arg()` is radians.
- `AnalysisData::Transient` holds `Vec<Sample>`; `axis` is seconds. The declared
  `.tran` output start filters results after integrating from t=0.

Both `Sample::get(&circuit, "v(out)")` and
`AcSample::get(&circuit, "v(out)")` are case-insensitive. They return `None` for
unknown signals. `v(0)` returns zero. Branch signals such as `i(v1)` exist only
for devices with allocated branch-current unknowns. Currents use the device's
first-to-second terminal orientation. For a large sample loop, look up the
signal's index once in `circuit.signals()` and index the values directly.

The runner returns only after every declared analysis succeeds; it returns no
partial result if a later analysis fails. Results are kept in memory. Each call
uses a fresh simulator, so separate calls do not share mutable numerical state.

## Customize or build circuits directly

To override netlist options, parse the deck first, set
`parsed.solver_options = risc_c::SolverOptions::precise()`, adjust the relevant
`parsed.analyses` options, then call `risc_c::run(parsed)`.

The facade also re-exports the circuit API. Use `CircuitBuilder`, `NodeId::GROUND`,
and `Element` for generated circuits; call `builder.build().compile()?` once,
then create a simulator with `circuit.simulator()`. `CompiledCircuit` is immutable
and cheaply cloneable; create a separate `Simulator` for each concurrent job.
Keep blocking numerical work off an async executor's task threads.

Runnable recipes, from a checkout:

```sh
cargo run -p risc-c --example netlist
cargo run -p risc-c --example builder
cargo run -p risc-c --example transient
cargo run -p risc-c --example ac
cargo run -p risc-c --example stream
cargo doc -p risc-c --no-deps --open
```

For large runs or cancellation, use `Simulator::transient_with`. Its callback
receives output samples as they arrive and can return `ControlFlow::Break(())`.
Cancellation returns `Error::Cancelled`. Streamed data is partial until the
method returns success; decide explicitly whether your application retains it.
The [streaming example](../crates/risc-c/examples/stream.rs) demonstrates this contract.

## Units, limits, and errors

Rust values use SI: V, A, ohms, F, H, s, and Hz; temperature APIs use degrees
Celsius. Netlist suffixes follow SPICE: `M` is milli and `MEG` is mega. Use
`1e6` when a suffix could be ambiguous.

`RunError` preserves parser file/line diagnostics, circuit compilation errors,
or the zero-based index and kind of a failed analysis. Its analysis `source` is
the underlying core `Error`, including `Singular`, `Convergence`, `TimeStep`,
and `NonFinite`. Display it for users and match variants for application logic.
An empty analysis list yields `RunError::NoAnalyses`.

Choose `TransientOptions::max_step` to resolve your fastest relevant dynamics,
and verify a waveform with a smaller step. `output_step` controls sample times;
it is not an accuracy guarantee. `SolverOptions::precise()` is a convenient
starting point for validation, not a guarantee of convergence. When a run fails,
surface the error and options rather than silently accepting missing data or
relaxing tolerances. Check the [compatibility contract](COMPATIBILITY.md) before
loading vendor netlists or models.

## CLI from any language

```sh
spice run circuit.cir --analysis op --output -
spice run circuit.cir --analysis ac --output gain.csv
spice run circuit.cir --analysis tran --output waveform.csv
```

Pass an argument array to a subprocess API. Stdout with `--output -` is CSV;
diagnostics and solver counts go to stderr. Exit zero means success; nonzero
means an argument, parse, simulation, or output error. Check status before using
output. A failed run may leave a previous output file untouched.

The first CSV column is `operating_point`, the swept source name, `frequency_hz`,
or `time_s`. Other columns are quoted signal names, including internal unknowns.
AC uses paired columns such as `v(out).re` / `v(out).im`. Parse CSV properly and
select signals by header, rather than relying on a fixed column position.

Stdout requires exactly one selected analysis. Multiple file outputs have analysis
suffixes, with indices added for duplicate analysis kinds. Prefer a single analysis
per subprocess when consuming stdout. `--analysis op` can request a bias even
when the deck has no `.op`; other selected kinds must be declared in the deck.

The [Python example](../examples/integrate_cli.py) checks exit status and parses
CSV with the standard library:

```sh
python examples/integrate_cli.py --spice /absolute/path/to/spice
```

Expect `v(out) = 5`. JavaScript, C#, and other subprocess clients can use the same
arguments and CSV contract.

## Verify the integration

```sh
python scripts/check_integration.py
```

This creates an independent temporary Cargo application, consumes only `risc-c`,
runs the examples with analytical assertions, then invokes the CLI from outside
the repository. CI runs it on Linux, macOS, and Windows. After committing a change,
`python scripts/check_integration.py --git` additionally tests Cargo's Git-package
discovery and CLI installation at the current local commit. The CLI is installed
into a temporary directory using the debug profile; the user's PATH is unchanged.
It needs no registry publication.
