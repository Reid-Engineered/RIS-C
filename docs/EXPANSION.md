# Coupling and switch expansion report

Scope: **coupling-switches-v1.0**, reference **ngspice 47**. Generated 2026-11-14T00:51:05+00:00.

**Evidence-backed checklist coverage: 100.00% (8/8).**

Implementation checklist coverage: **100.00% (8/8)**.

ngspice comparison pass rate: **100.00% (3/3 executed test groups)**; oracle status: `passed`.

These percentages describe the coupling and switch expansion checklist, not all of ngspice, numerical accuracy, engineering effort, or production readiness. Each criterion has one point. Missing, untested, skipped, failed, or unavailable criteria receive no evidence credit; partial implementations receive no implementation credit. A passing witness covers its named fixtures, not every circuit that might use a feature. Oracle counts are test groups, not individual circuits or sample points.

| Category | Implemented | Evidence-backed | Evidence-backed coverage |
| --- | ---: | ---: | ---: |
| Coupled inductors | 3/3 | 3/3 | 100.00% |
| Controlled switches | 4/4 | 4/4 | 100.00% |
| Converter validation | 1/1 | 1/1 | 100.00% |

## Acceptance checklist

| ID | Criterion | State | Evidence / witness tests |
| --- | --- | --- | --- |
| X01 | K syntax, hierarchical references and physical coupling validation | evidenced | [crates/risc-c-netlist/tests/expansion.rs](../crates/risc-c-netlist/tests/expansion.rs); `expansion/coupling_validation_and_hierarchy` |
| X02 | Loaded transformer AC transfer, ideal coupling and dot polarity | evidenced | [crates/risc-c-netlist/tests/ngspice.rs](../crates/risc-c-netlist/tests/ngspice.rs); `expansion/transformer_ac_transfer_and_polarity`, `oracle/expansion_transformer_matches_reference` |
| X03 | Coupled flux histories and passive magnetic energy balance for BE/trap/BDF2 | evidenced | [crates/risc-c-netlist/tests/expansion.rs](../crates/risc-c-netlist/tests/expansion.rs); `expansion/coupled_transient_energy_balance` |
| X04 | SW/CSW DC hysteresis, controlling-current polarity and frozen-bias AC | evidenced | [crates/risc-c-netlist/tests/expansion.rs](../crates/risc-c-netlist/tests/expansion.rs); `expansion/switch_dc_hysteresis_ac_and_current_polarity`, `expansion/switch_models_and_controls_are_instance_scoped` |
| X05 | ON/OFF initialization, threshold localization and transient restart | evidenced | [crates/risc-c-netlist/tests/expansion.rs](../crates/risc-c-netlist/tests/expansion.rs); `expansion/switch_event_timing_and_initial_hints` |
| X06 | Rejected and failed trials preserve accepted hysteresis and retry results | evidenced | [crates/risc-c-core/src/simulator.rs](../crates/risc-c-core/src/simulator.rs); `core/simulator::switch_history_tests::rejected_switch_trials_preserve_hysteresis_and_retry` |
| X07 | Voltage/current-controlled switched-load DC/transient reference comparisons | evidenced | [crates/risc-c-netlist/tests/ngspice.rs](../crates/risc-c-netlist/tests/ngspice.rs); `oracle/expansion_switches_match_reference` |
| X08 | Buck converter refinement, power balance and ngspice waveform agreement | evidenced | [crates/risc-c-netlist/tests/ngspice.rs](../crates/risc-c-netlist/tests/ngspice.rs); `expansion/buck_refinement_and_power_balance`, `oracle/expansion_buck_matches_reference` |

## Refresh and history

Run `python3 scripts/expansion.py --run` to execute the registered suites and the 10,000-node benchmark, then regenerate Markdown and JSON. Set `NGSPICE_BIN` or use `--ngspice /absolute/path`; the local pinned reference is discovered automatically. Without ngspice, oracle coverage is unmeasured and its criteria lose evidence credit.

Run `python3 scripts/expansion.py --check` to detect stale source inputs, changed scope, or edited report totals. The check validates the recorded snapshot; it does not rerun tests. CI refreshes the results and uploads reports plus logs even when tests fail.

The denominator is versioned in [expansion-scope.json](../reports/expansion-scope.json). Changing criteria requires a scope-version bump and an entry in its change log; compare history only within the same scope version. [History](../reports/expansion-history.jsonl) records source/metric changes without duplicating identical reruns. [Machine-readable snapshot](../reports/expansion.json) contains per-suite outcomes and source hashes.

Source SHA-256: `7e9db3d335ed33dfe106969fc1fb42e01fc2743817c2857f5f1aaa03f8434a1b`.
