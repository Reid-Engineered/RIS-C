# Implementation status

This workspace implements the planned classic analog core and its explicit electrical-parameter inventory. Native Linux/macOS/Windows acceptance is recorded for the validated numerical source in [platform evidence](../reports/platform-evidence.json); it is based on actual execution, not cross-compilation or a configured workflow alone. The [maintained percentage report](PARITY.md) quantifies the versioned core checklist and identifies criteria without executable evidence.

| Planned stage | Current status |
| --- | --- |
| 0 Reference contract | Pinned ngspice 47 archive/checksum, source mapping, build script, attribution, explicit parameter coverage |
| 1 Linear DC | Implemented and analytically tested; dense Peroxide and sparse faer cross-checks |
| 2 Linear AC/transient | Implemented; RC/RL/RLC analytical checks and external RC comparisons |
| 3 Nonlinear diode | Implemented supported subset, continuation, structured errors, bias/temperature/breakdown comparisons |
| 4 Adaptive integration | BE/trapezoidal/variable-step BDF2; immutable trial histories, step-doubling, breakpoints, rejection/restart tests |
| 5 Legacy transistors | Classic diode/Gummel–Poon/MOS1 electrical sets audited (80 canonical parameters); 152 DC/AC cards and 15 type/temperature transient cases, plus process/phase/instance-temperature tests |
| 6 Netlist/CLI | Declared subset, hierarchical parameter/model scoping, source-map diagnostics, batch CSV and failure-exit tests |
| 7 Hardening | Linux validation, native CI, 10,000-node benchmark, bistability/scaled-pivot/two stiff topologies, and the classic parameter regression matrix; see platform evidence for observed OS results |

## Deliberate architectural departures

- Peroxide supplies local automatic differentiation and the independent dense oracle. faer supplies real/complex sparse LU.
- Step doubling controls storage variables rather than porting ngspice's truncation-error estimator. The two-half-step solution is accepted; the full step supplies the error estimate. Trial histories remain isolated until acceptance.
- Newton differentiates nonlinear base resistance; AC freezes it at bias to reproduce ngspice's small-signal approximation.
- No C runtime/device-model fallback. No frontend/numparam or Nutmeg port.

## Reliability increment

The [small implementation plan](RELIABILITY_PLAN.md) adds direct witnesses for octave sweeps, restoring nominal nonlinear bias after a DC sweep, successful gmin/source continuation after direct-solve failure, and isolation of accepted histories after rejected/failed trials. Recovery results are checked against the unmodified circuit and ngspice 47. A coupled nanosecond/millisecond diode/RC fixture adds timestep-refinement and oracle waveform checks. The subsequent core increment completes the declared stress corpus with bistability, difficult pivots, and a second stiff topology.

`RunStats` now exposes `direct_failures`, `gmin_attempts`, `source_attempts`, and the last successful `dc_method`. `continuation_attempts` includes initial gmin/source solves and equals the sum of both strategy counters; final rechecks of the original equations are excluded. No test-only convergence forcing or alternative circuit equations are introduced.

## Native acceptance

[GitHub Actions run 43536126132](https://github.com/Reid-Engineered/RIS-C/actions/runs/43536126132) passed 46 tests on each of Linux x86-64, macOS ARM64, and Windows AMD64. Results were extracted from successful job logs, checked against GitHub job conclusions, and matched to the numerical source SHA-256. The parity runner refuses credit from stale or incomplete platform records. The runtime commit SHA in a PR job can be its synthetic merge commit; the workflow head and numerical fingerprint are retained separately.

## Completion boundary and follow-up work

[The legacy audit](LEGACY_PARAMETER_AUDIT.md) makes the original classic-device target explicit: 13 diode, 39 BJT, and 28 MOS1 electrical parameters. These are exercised in the DC/AC perturbation matrix and the five-device/three-temperature transient matrix. The wider stress corpus now includes both latch states, scaled zero-diagonal MNA cases, diode/RC and BJT storage stiffness, and switching/history failures. This is finite core acceptance, not exhaustive production qualification.

The initial-condition solver handles floating difference networks, consistent loops, conflicting constraints, and redundant constraints on ideal sources. Element IC applies with UIC; instance TEMP/DTEMP and OFF syntax are supported. Excess-phase and Meyer histories stay local to accepted/trial steps. Transient convergence explicitly accounts for representational charge-subtraction roundoff while still requiring the configured state-update tolerance.

Remaining work after this classic core includes newer ngspice model extensions, broader vendor syntax, more stress/benchmark families, API stabilization, and production qualification. No all-ngspice compatibility or production-readiness claim is made by the version number. The six original completion gates remain visible in the same 48-item parity denominator.

## Coupling and switch expansion

The next increment adds K-coupled linear windings and S/W controlled switches with SW/CSW models, finite RON/ROFF, nonnegative hysteresis, and scoped model/control references. Symmetric cross-flux stamps work in AC and all three transient methods; multiwinding validation rejects negative magnetic-energy matrices while admitting ideal coupling.

Switches retain accepted hysteresis through a DC sweep, freeze conductance for AC, and localize transient threshold crossings before committing state and restarting integration. Trial states remain isolated. A switching-specific shared KCL scale improves Newton line-search progress through current redistribution; final per-equation convergence tolerances stay unchanged. Transformer transfer/polarity, magnetic-energy decay, switched loads, history rollback, and buck-converter waveform/refinement/power balance have executable witnesses in the separate [eight-item report](EXPANSION.md). The original core definition remains unchanged.
