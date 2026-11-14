# Local validation

Local environment: Linux x86_64, Rust 1.100.0, release profile for the benchmark. Native CI has additionally executed and passed all 46 local tests on Linux x86-64, macOS ARM64, and Windows AMD64 for the numerical source recorded in [platform-evidence.json](../reports/platform-evidence.json). The record includes job URLs, runtime platform/architecture, commit/run IDs, source fingerprints, test counts, and log digests; it is validated against the current numerical source before the platform criterion receives credit.

## Checks

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`: 46 tests passed; sixteen external-oracle tests intentionally ignored in the ordinary suite.
- `NGSPICE_BIN=... cargo test -p risc-c-netlist --test ngspice --locked -- --ignored`: sixteen oracle groups cover the original core and coupling/switch expansion against locally built, unmodified ngspice 47.

The test suites include independent Peroxide dense solves, finite-difference checks of Peroxide-generated device derivatives, terminal charge/current conservation, RLC analytical responses, all controlled-source types, parameter scoping/cycles/includes, 2,000 deterministic malformed netlists, initial conditions, limits, singular circuits, callback cancellation, concurrent simulators, and CLI exit/output behavior.

Oracle coverage includes diode/NPN/PNP/MOS operating points; RC complex frequency response and transient charging; diode breakdown, BJT nonlinear base resistance and MOS mobility/threshold behavior at 0/27/80 C; semiconductor AC capacitances; and CMOS switching with rejected steps and breakpoint restarts. Passing this corpus is not exhaustive ngspice compatibility.

## Reliability regression evidence

- Octave sweeps: frequency ratios, endpoint behavior, invalid inputs, and the analytical complex RC transfer function.
- Sweep-to-AC reuse: a diode biased at 0.1 V, swept to 5 V, then analyzed at the original 0.1 V bias; a fresh simulator provides an independent run.
- Gmin/source fallback: tight iteration budgets cause real direct failures, per-strategy statistics prove which recovery ran, and original KCL/voltage constraints plus ngspice comparisons verify the recovered solution.
- Rejected/failed trials: deliberately oversized inverter steps exceed the error limit, and a restricted Newton budget causes failure. All accepted state (voltages, charges, derivatives, BDF history, and Meyer state) stays unchanged; retry results match a clean run for BE/trapezoidal/BDF2.
- Stiff nonlinear transient: `examples/stiff-diode.cir` couples nanosecond diode storage to millisecond RC storage. BDF2 results are compared at all 151 output samples against a refined Rust run and ngspice with a 1 us maximum step. Voltage comparisons use `1e-6 + 1e-3 * max(abs(a), abs(b))` tolerances. The refinement test runs in the normal suite without ngspice too.

The parity denominator remains the same 48 criteria. The subsequent classic-core increment adds the complete canonical parameter inventory, process-derived MOS parameters, BJT excess phase and substrate configuration, instance temperatures, consistent IC networks, bistable/scaled-pivot cases, and a second stiff topology. See the [audit](LEGACY_PARAMETER_AUDIT.md) for the regression dimensions and [parity report](PARITY.md) for current results and native CI evidence.

## Coupling and switch witnesses

- Loaded transformer transfer has an independent complex-network calculation and ngspice AC/transient comparisons for K=-1, -0.98, 0, 0.98, and 1. K syntax/scoping and multiwinding energy validation include singular ideal groups and invalid pair combinations.
- Coupled-winding passive energy decay is checked against integrated resistor dissipation for backward Euler, trapezoidal, and BDF2, with a 0.4% energy-balance bound.
- SW/CSW tests cover both sweep directions, negative control current, nominal frozen-bias AC, scoped models/controls, ON/OFF hints, threshold localization, and accepted-state isolation after discarded or failed trials. Direct event timing is checked within 1 ns. Reference switched-load samples avoid exact threshold instants where the reference's iteration history can choose a different boundary state.
- `examples/buck.cir` runs at 50 kHz with BDF2, a 200 ns maximum step/output interval, finite switch resistance, and a diode freewheel path. A 100 ns refinement is compared with 15 mV/5 mA absolute bounds. A separate ngspice run uses a 25 ns maximum step; output comparisons use 10 mV + 0.3% and inductor-current comparisons use 5 mA + 0.5%. Integrated supply energy must cover load plus final storage within 10 uJ, with positive aggregate losses below 15% of supply energy.

Run `python3 scripts/expansion.py --run` and `--check` to maintain the separate eight-criterion report and append-only history. Both reporting scripts keep strict source-freshness checks by default. CI uses `--check --allow-stale` only to validate the integrity of checked-in snapshots, which are imported separately; its oracle job executes current sources and uploads fresh evidence. Allowing a stale snapshot does not give it current-source evidence credit. The completed 48-criterion classic scope remains separate.

## Initial sparse benchmark

Command: `cargo run -p risc-c-core --example ladder --release -- 10000`.

One run of a linear RC ladder, 10,000 voltage nodes plus one source branch current:

| Measurement | Observed |
| --- | ---: |
| Unknowns | 10,001 |
| Builder | 940.5 ms |
| Compile and symbolic analysis | 39.0 ms |
| DC solve, two Newton iterations/factorizations | 8.3 ms |
| Three-frequency AC, including bias check | 12.5 ms |
| Whole executable wall time | 1.006 s |
| Peak resident memory | 23,144 KiB |

Measured the already-built executable in a fresh Python subprocess using `perf_counter` and Linux child `getrusage`; compiler memory/time is excluded. These are single-run measurements, not speed-parity claims or representative nonlinear workloads. Builder duplicate-name checks currently dominate this ladder's construction cost; runtime stamping still uses indexed hash lookup rather than fully prebound slot arrays.

Adaptive transient control uses extra full/half trial solves for an inspectable error estimate. More device families, stiff networks, and long-duration nonlinear waveforms need their own performance/accuracy characterization before production use.
