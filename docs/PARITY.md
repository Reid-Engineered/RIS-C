# Core parity report

Scope: **core-v1.0**, reference **ngspice 47**. Generated 2026-11-13T22:24:49+00:00.

**Evidence-backed checklist coverage: 100.00% (48/48).**

Implementation checklist coverage: **100.00% (48/48)**.

ngspice comparison pass rate: **100.00% (13/13 executed test groups)**; oracle status: `passed`.

These percentages describe the agreed core checklist, not all of ngspice, numerical accuracy, engineering effort, or production readiness. Each criterion has one point. Missing, untested, skipped, failed, or unavailable criteria receive no evidence credit; partial implementations receive no implementation credit. A passing witness covers its named fixtures, not every circuit that might use a feature. Oracle counts are test groups, not individual circuits or sample points.

| Category | Implemented | Evidence-backed | Evidence-backed coverage |
| --- | ---: | ---: | ---: |
| Analyses | 8/8 | 8/8 | 100.00% |
| Numerical core | 8/8 | 8/8 | 100.00% |
| Transient integration | 8/8 | 8/8 | 100.00% |
| Devices | 8/8 | 8/8 | 100.00% |
| Netlist compatibility | 8/8 | 8/8 | 100.00% |
| Library and acceptance | 8/8 | 8/8 | 100.00% |

## Acceptance checklist

| ID | Criterion | State | Evidence / witness tests |
| --- | --- | --- | --- |
| A01 | DC operating point | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/divider_and_source_current`, `oracle/nonlinear_operating_points` |
| A02 | Independent-source DC sweep | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/divider_and_source_current` |
| A03 | Linear RC small-signal AC | evidenced | [crates/spice-netlist/tests/ngspice.rs](../crates/spice-netlist/tests/ngspice.rs); `oracle/rc_ac_and_transient` |
| A04 | Semiconductor bias-linearized AC | evidenced | [crates/spice-netlist/tests/ngspice.rs](../crates/spice-netlist/tests/ngspice.rs); `oracle/semiconductor_ac_capacitances` |
| A05 | Linear and decade frequency grids | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/ac_rc_cutoff`, `oracle/rc_ac_and_transient` |
| A06 | Octave frequency grids | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/octave_grid_matches_rc_transfer_function` |
| A07 | Nominal bias restored after DC sweep | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/ac_restores_nominal_nonlinear_bias_after_sweep` |
| A08 | UIC transient initialization | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `parser/initial_conditions`, `analytical/floating_capacitor_ic_respects_existing_hint` |
| N01 | MNA branch constraints and current sign conventions | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/divider_and_source_current` |
| N02 | Sparse real and complex LU against dense Peroxide | evidenced | [crates/spice-core/src/linear.rs](../crates/spice-core/src/linear.rs); `core/linear::tests::peroxide_dense_oracle_and_complex` |
| N03 | Row/column equilibration on badly scaled fixture | evidenced | [crates/spice-core/src/linear.rs](../crates/spice-core/src/linear.rs); `core/linear::tests::peroxide_dense_oracle_and_complex` |
| N04 | Newton device derivatives and bias convergence | evidenced | [crates/spice-core/src/device.rs](../crates/spice-core/src/device.rs); `core/device::tests::peroxide_jacobians_and_charge_conservation`, `oracle/nonlinear_operating_points` |
| N05 | Gmin continuation through a direct-solve failure | evidenced | [crates/spice-core/src/simulator.rs](../crates/spice-core/src/simulator.rs); `core/simulator::reliability_tests::gmin_recovers_after_direct_newton_failure`, `oracle/continuation_recovery_matches_original_ngspice_bias` |
| N06 | Source stepping through a direct-solve failure | evidenced | [crates/spice-core/src/simulator.rs](../crates/spice-core/src/simulator.rs); `core/simulator::reliability_tests::source_stepping_recovers_after_direct_and_gmin_failure`, `oracle/continuation_recovery_matches_original_ngspice_bias` |
| N07 | Singular circuits return errors | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/singular_is_error` |
| N08 | Independent concurrent simulators | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/concurrent_instances_and_cancellation` |
| T01 | Backward Euler RC response | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/rc_transient_all_methods` |
| T02 | Trapezoidal RC response | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/rc_transient_all_methods` |
| T03 | BDF2 RC response | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/rc_transient_all_methods` |
| T04 | Adaptive RL/RLC response | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/rl_and_rlc_transient` |
| T05 | Rejected steps on a switching circuit | evidenced | [crates/spice-netlist/tests/ngspice.rs](../crates/spice-netlist/tests/ngspice.rs); `oracle/switching_rejections_and_restart` |
| T06 | Source breakpoints and integrator restarts | evidenced | [crates/spice-netlist/tests/ngspice.rs](../crates/spice-netlist/tests/ngspice.rs); `oracle/switching_rejections_and_restart` |
| T07 | Isolated accepted and trial histories | evidenced | [crates/spice-core/src/simulator.rs](../crates/spice-core/src/simulator.rs); `core/simulator::reliability_tests::rejected_trials_preserve_accepted_meyer_and_bdf_histories`, `oracle/switching_rejections_and_restart` |
| T08 | Conflicting ICs and resource limits return errors | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/conflicting_initial_conditions`, `analytical/tolerances_and_limits_are_checked` |
| D01 | Linear resistor/capacitor/inductor behavior | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/divider_and_source_current`, `analytical/rl_and_rlc_transient` |
| D02 | Independent DC/AC and pulse source fixtures | evidenced | [crates/spice-netlist/tests/ngspice.rs](../crates/spice-netlist/tests/ngspice.rs); `oracle/rc_ac_and_transient`, `oracle/switching_rejections_and_restart` |
| D03 | Linear E/G/F/H controlled sources | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/dependent_sources`, `analytical/current_controlled_and_transconductance_sources` |
| D04 | Complete planned legacy diode electrical parameter set | evidenced | [docs/LEGACY_PARAMETER_AUDIT.md](../docs/LEGACY_PARAMETER_AUDIT.md); `oracle/classic_parameter_matrix_matches_ngspice`, `oracle/classic_transient_temperature_matrix_matches_ngspice` |
| D05 | Complete planned Gummel-Poon electrical parameter set | evidenced | [docs/LEGACY_PARAMETER_AUDIT.md](../docs/LEGACY_PARAMETER_AUDIT.md); `oracle/classic_parameter_matrix_matches_ngspice`, `oracle/classic_transient_temperature_matrix_matches_ngspice`, `oracle/bjt_excess_phase_matches_reference` |
| D06 | Complete planned MOS1 electrical parameter set | evidenced | [docs/LEGACY_PARAMETER_AUDIT.md](../docs/LEGACY_PARAMETER_AUDIT.md); `oracle/classic_parameter_matrix_matches_ngspice`, `oracle/classic_transient_temperature_matrix_matches_ngspice`, `oracle/mos1_process_and_sheet_resistance_match_reference` |
| D07 | Supported diode/BJT/MOS DC and capacitance subset | evidenced | [crates/spice-netlist/tests/ngspice.rs](../crates/spice-netlist/tests/ngspice.rs); `oracle/nonlinear_operating_points`, `oracle/semiconductor_ac_capacitances` |
| D08 | Supported-device temperature and breakdown fixtures | evidenced | [crates/spice-netlist/tests/ngspice.rs](../crates/spice-netlist/tests/ngspice.rs); `oracle/temperature_breakdown_and_base_resistance` |
| P01 | Numeric suffixes and bounded parameter expressions | evidenced | [crates/spice-netlist/src/expression.rs](../crates/spice-netlist/src/expression.rs); `expressions/expression::tests::expressions`, `parser/deep_expressions_are_bounded` |
| P02 | Parameter scoping and dependency cycles | evidenced | [crates/spice-netlist/src/lib.rs](../crates/spice-netlist/src/lib.rs); `parser/hierarchy_and_scoping`, `expressions/expression::tests::cycles` |
| P03 | Model cards and line continuations | evidenced | [crates/spice-netlist/src/lib.rs](../crates/spice-netlist/src/lib.rs); `parser/model_cards_and_continuations` |
| P04 | Subcircuit expansion and recursion rejection | evidenced | [crates/spice-netlist/src/lib.rs](../crates/spice-netlist/src/lib.rs); `parser/hierarchy_and_scoping`, `parser/rejects_unsupported_fields` |
| P05 | Relative includes, case preservation and cycle rejection | evidenced | [crates/spice-netlist/src/lib.rs](../crates/spice-netlist/src/lib.rs); `parser/include_cycle_and_case_sensitive_path` |
| P06 | Analysis directives and initial-condition syntax | evidenced | [crates/spice-netlist/src/lib.rs](../crates/spice-netlist/src/lib.rs); `parser/example_netlists_compile`, `parser/initial_conditions` |
| P07 | Unsupported features fail explicitly | evidenced | [crates/spice-netlist/src/lib.rs](../crates/spice-netlist/src/lib.rs); `parser/rejects_unsupported_fields` |
| P08 | Malformed-input and nesting limits | evidenced | [crates/spice-netlist/src/lib.rs](../crates/spice-netlist/src/lib.rs); `parser/malformed_inputs_return_errors`, `parser/deep_expressions_are_bounded` |
| L01 | Typed embeddable circuit API | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/divider_and_source_current` |
| L02 | Batch CLI CSV and nonzero error exits | evidenced | [crates/spice-cli/tests/batch.rs](../crates/spice-cli/tests/batch.rs); `cli/stdout_csv_and_failure_exit` |
| L03 | Failed analyses do not publish successful result files | evidenced | [crates/spice-cli/tests/batch.rs](../crates/spice-cli/tests/batch.rs); `cli/simulation_failure_does_not_publish_results` |
| L04 | Streaming cancellation and reuse | evidenced | [crates/spice-core/tests/analytical.rs](../crates/spice-core/tests/analytical.rs); `analytical/concurrent_instances_and_cancellation` |
| L05 | 10,000-node sparse benchmark executes | evidenced | [crates/spice-core/examples/ladder.rs](../crates/spice-core/examples/ladder.rs); `scale/ladder_10000` |
| L06 | Observed Linux/macOS/Windows numerical acceptance | evidenced | [docs/STATUS.md](../docs/STATUS.md); `platform/Linux`, `platform/Darwin`, `platform/Windows` |
| L07 | Bistability, difficult-pivot and stiff nonlinear stress corpus | evidenced | [crates/spice-netlist/tests/ngspice.rs](../crates/spice-netlist/tests/ngspice.rs); `parser/bistable_latch_and_scaled_pivots`, `oracle/numerical_stress_corpus_matches_ngspice`, `oracle/stiff_nonlinear_transient_matches_reference_and_refinement`, `core/simulator::precision_floor_tests::sub_ulp_charge_updates_do_not_collapse_the_timestep_or_hide_source_errors` |
| L08 | Full planned legacy-model regression matrix | evidenced | [docs/LEGACY_PARAMETER_AUDIT.md](../docs/LEGACY_PARAMETER_AUDIT.md); `parser/every_classic_electrical_parameter_is_accepted`, `oracle/classic_parameter_matrix_matches_ngspice`, `oracle/classic_transient_temperature_matrix_matches_ngspice`, `oracle/instance_temperature_overrides_match_ngspice` |

## Refresh and history

Run `python3 scripts/parity.py --run` to execute the registered suites and the 10,000-node benchmark, then regenerate Markdown and JSON. Set `NGSPICE_BIN` or use `--ngspice /absolute/path`; the local pinned reference is discovered automatically. Without ngspice, oracle coverage is unmeasured and its criteria lose evidence credit.

Run `python3 scripts/parity.py --check` to detect stale source inputs, changed scope, or edited report totals. The check validates the recorded snapshot; it does not rerun tests. CI refreshes the results and uploads reports plus logs even when tests fail.

The denominator is versioned in [parity-scope.json](../reports/parity-scope.json). Changing criteria requires a scope-version bump and an entry in its change log; compare history only within the same scope version. [History](../reports/parity-history.jsonl) records source/metric changes without duplicating identical reruns. [Machine-readable snapshot](../reports/parity.json) contains per-suite outcomes and source hashes.

Source SHA-256: `cac3e4eeb0f5fba4108b4142d3efcbc0bc4f06e8bb25fdfe3bd89c6a3f4148ad`.
