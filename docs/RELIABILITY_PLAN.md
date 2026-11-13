# Solver reliability increment

1. Add analytical octave-grid checks and a nonlinear sweep-to-AC regression that proves AC uses nominal bias.
2. Exercise real gmin and source-stepping recoveries after direct Newton failures. Expose per-strategy attempt counts and the successful DC method, and verify the recovered result against the original circuit equations.
3. Extract the existing transient trial/error-estimation operation into an internal helper. Verify that discarded error-estimate trials and failed Newton trials preserve all accepted histories, including Meyer capacitances/charges, and that retrying matches a clean run.
4. Add a coupled diode/RC transient with widely separated time constants; check timestep refinement and compare its waveform with pinned ngspice 47.
5. Register the five new witnesses against the existing 48-item parity denominator, run the full validation suite, and refresh the report/history.

Acceptance: targeted tests pass without masking solver failures; formatting, Clippy, Rust tests, parity-accounting tests, and ngspice comparisons pass. Keep the broader bistability/difficult-pivot/stiff-corpus criterion open: one stiff fixture does not complete that corpus.
