# RIS-C

Rust SPICE core

A library-first, pure-Rust analog simulator, using ngspice 47 as its numerical reference. Includes a circuit API, a bounded SPICE netlist parser, and a batch CLI.

This is a pure-Rust implementation of the planned classic analog core, **not a drop-in ngspice replacement**. DC, AC, and adaptive transient analyses cover the documented classic device parameters. See the [parameter audit](docs/LEGACY_PARAMETER_AUDIT.md), [implementation status](docs/STATUS.md), and live parity evidence for the exact acceptance boundary. Newer ngspice extensions and broad vendor compatibility are separate work.

## Parity tracking

[Current percentage report](docs/PARITY.md) tracks the versioned core acceptance checklist, separately from the ngspice comparison pass rate. Missing and untested criteria stay in the denominator. [JSON](reports/parity.json) records test outcomes; [history](reports/parity-history.jsonl) keeps progress snapshots.

```sh
python3 scripts/parity.py --run    # refresh tests, oracle comparisons, benchmark and report
python3 scripts/parity.py --check  # detect stale source or report edits without rerunning
```

The local ngspice 47 reference is discovered automatically, or set `NGSPICE_BIN`. Without a reference, oracle coverage is reported as unmeasured. The report is a checklist metric, not a percentage of all ngspice features or engineering effort.

Coupled inductors (`K`) and hysteretic voltage/current-controlled switches (`S`/`W`, `SW`/`CSW`) have a separate [expansion report](docs/EXPANSION.md), with transformer, switched-load, energy-balance, and buck-converter regressions. The original core checklist keeps its 48-item definition.

```sh
python3 scripts/expansion.py --run
python3 scripts/expansion.py --check
cargo run -p spice-cli -- run examples/transformer.cir --output transformer.csv
cargo run -p spice-cli -- run examples/buck.cir --output buck.csv
```

## Run

Requires Rust 1.91+; tested here on Rust 1.100. No native numerical library or ngspice installation is required to run the simulator.

```sh
cargo test --workspace --locked
cargo run -p spice-cli -- run examples/rc.cir --output results.csv
cargo run -p spice-cli -- run examples/amplifier.cir --analysis ac --output gain.csv
cargo run -p spice-cli -- run examples/inverter.cir --analysis tran --output switching.csv
cargo run -p spice-cli -- run examples/stiff-diode.cir --output stiff.csv
```

When using this session's local toolchain, substitute `scripts/cargo-local.sh` for `cargo`. Multiple declared analyses produce `results.op.csv`, `results.ac.csv`, and `results.tran.csv`. `--output -` supports a single analysis on stdout. AC columns contain real and imaginary values, frequencies are Hz, and time is seconds. Diagnostics and solver counts go to stderr. An analysis failure returns nonzero and does not publish new simulation results.

## Embed

```rust
use spice_core::*;

fn main() -> Result<()> {
    let mut b = CircuitBuilder::new();
    let input = b.node("in");
    let output = b.node("out");
    let ground = NodeId::GROUND;
    b.add("v1", &[input, ground], Element::Voltage(10.0.into()))?;
    b.add("r1", &[input, output], Element::Resistor(1_000.0))?;
    b.add("r2", &[output, ground], Element::Resistor(1_000.0))?;
    let circuit = b.build().compile()?;
    let point = circuit.simulator().operating_point()?;
    assert!((point.get(&circuit, "v(out)").unwrap() - 5.0).abs() < 1e-9);
    Ok(())
}
```

`CompiledCircuit` is immutable and shareable. Each `Simulator` owns its state, history, tolerances, and statistics. `transient_with` streams samples through a callback that can cancel with `ControlFlow::Break(())`. `RunStats` reports the last successful DC method and separate gmin/source continuation attempts, along with timestep and Newton statistics. Results contain node voltages and allocated branch currents; currents use the device's first-to-second terminal orientation. The public API is version 0.1 and may evolve.

## Numerical implementation

- MNA with fixed sparse structure, internal resistance nodes, branch-current constraints, and reusable symbolic LU.
- **Peroxide 0.43.1:** local forward-mode automatic differentiation for device Jacobians and charges; independent dense reference solves in tests.
- **faer 0.24.4:** sparse, nonsymmetric real and complex LU. Peroxide's sparse LU converts to dense, so it is not used for production sparse solves.
- Newton iteration with voltage damping, residual backtracking, update/residual convergence checks, gmin continuation, and source stepping. BJT AC freezes the bias-dependent base resistance as ngspice does; Newton differentiates it to improve convergence of the same bias equations.
- AC linearizes the DC bias and solves `G + jωC`.
- Backward Euler, trapezoidal, and variable-step BDF2 transient integration. Step-doubling error estimates control storage variables; accepted histories, including Meyer charges, are isolated from trial steps. Breakpoints restart the integrator. Output times constrain internal steps.
- Row/column equilibration and linear backward-error checks. Invalid, nonfinite, singular, nonconvergent, and exhausted-timestep cases return errors.

[Compatibility](docs/COMPATIBILITY.md) specifies the supported dialect and model parameters. [Provenance](reference/PROVENANCE.md) maps the implementation to upstream algorithms and records the pinned source checksum. [LICENSE](LICENSE) and [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES) retain attribution.

## Validate against ngspice

The oracle is optional and is never linked into the Rust runtime:

```sh
scripts/build-reference.sh
NGSPICE_BIN="$PWD/reference/build/src/ngspice" \
  cargo test -p spice-netlist --test ngspice --locked -- --ignored
```

The reference build needs a C compiler, make, curl, tar, and Python 3.11+. The archive checksum is verified. Analytical, derivative, charge-conservation, parser, CLI, cancellation, and concurrency tests run without ngspice. Oracle tests explicitly fail if requested without `NGSPICE_BIN`.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p spice-core --example ladder --release -- 10000
```

Native numerical acceptance has been observed on Linux x86-64, macOS ARM64, and Windows AMD64 for the recorded numerical fingerprint; see [platform evidence](reports/platform-evidence.json). A separate Linux job runs the pinned ngspice oracle suite. New source changes invalidate stale platform evidence.
