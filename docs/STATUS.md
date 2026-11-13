# Implementation status

This workspace implements the core architecture and a runnable simulation subset. It does not complete the entire proposed release specification. The [maintained percentage report](PARITY.md) quantifies the versioned core checklist and identifies criteria without executable evidence.

| Planned stage | Current status |
| --- | --- |
| 0 Reference contract | Pinned ngspice 47 archive/checksum, source mapping, build script, attribution, explicit parameter coverage |
| 1 Linear DC | Implemented and analytically tested; dense Peroxide and sparse faer cross-checks |
| 2 Linear AC/transient | Implemented; RC/RL/RLC analytical checks and external RC comparisons |
| 3 Nonlinear diode | Implemented supported subset, continuation, structured errors, bias/temperature/breakdown comparisons |
| 4 Adaptive integration | BE/trapezoidal/variable-step BDF2; immutable trial histories, step-doubling, breakpoints, rejection/restart tests |
| 5 Legacy transistors | Gummel–Poon/MOS1 foundation with documented parameter subset; DC, temperature, AC capacitance, and switching fixtures pass; exhaustive electrical-parameter parity remains open |
| 6 Netlist/CLI | Declared subset, hierarchical parameter/model scoping, source-map diagnostics, batch CSV and failure-exit tests |
| 7 Hardening | Local Linux checks, CI definitions, 10,000-node ladder benchmark; broad topology/model regression and observed macOS/Windows CI remain open |

## Deliberate architectural departures

- Peroxide supplies local automatic differentiation and the independent dense oracle. faer supplies real/complex sparse LU.
- Step doubling controls storage variables rather than porting ngspice's truncation-error estimator. The full-step solution is accepted; half steps are isolated estimates. This costs extra evaluations but makes acceptance and rollback straightforward to inspect.
- Newton differentiates nonlinear base resistance; AC freezes it at bias to reproduce ngspice's small-signal approximation.
- No C runtime/device-model fallback. No frontend/numparam or Nutmeg port.

## Reliability increment

The [small implementation plan](RELIABILITY_PLAN.md) adds direct witnesses for octave sweeps, restoring nominal nonlinear bias after a DC sweep, successful gmin/source continuation after direct-solve failure, and isolation of accepted histories after rejected/failed trials. Recovery results are checked against the unmodified circuit and ngspice 47. A coupled nanosecond/millisecond diode/RC fixture adds timestep-refinement and oracle waveform checks. The broader stress-corpus criterion remains open.

`RunStats` now exposes `direct_failures`, `gmin_attempts`, `source_attempts`, and the last successful `dc_method`. `continuation_attempts` includes initial gmin/source solves and equals the sum of both strategy counters; final rechecks of the original equations are excluded. No test-only convergence forcing or alternative circuit equations are introduced.

## Remaining acceptance work

1. Expand legacy electrical parameters: MOS1 process-derived parameters and sheet resistance; BJT excess phase and further default-model extensions; extended diode branches. Every addition needs DC, AC, transient, and temperature cases where relevant.
2. Extend device initialization: instance OFF/TEMP/DTEMP and full SPICE instance-IC semantics; improve floating capacitor initial-condition constraint handling.
3. Broaden numerical stress coverage beyond the tested continuation recoveries and first stiff diode/RC case: bistable operating points, difficult pivots, more stiff nonlinear topologies, and long-duration conservation/phase tests.
4. Add more circuit families to the 10,000-unknown benchmark corpus and collect repeatable allocation/assembly/factorization/peak-memory profiles beyond the initial ladder.
5. Observe the configured platform CI and review public API stability before calling this a production release.

No full-ngspice compatibility or production-readiness claim is made by the version number.
