# Coupled inductors and controlled switches

1. Add symmetric mutual-flux stamps for K elements. Resolve winding names after elaboration; validate coefficients, duplicate pairs, and positive-semidefinite multiwinding energy.
2. Add SW/CSW models with finite RON/ROFF, thresholds, nonnegative hysteresis, and instance ON/OFF hints. Preserve state through DC sweeps and freeze it for small-signal AC.
3. Keep switch states in accepted/trial transient histories. Locate threshold crossings, restart integration, and test rollback under rejected or failed trials.
4. Validate transformer polarity/transfer, switched loads, hysteresis, energy balance, and a basic converter analytically and against pinned ngspice 47.
5. Maintain a separate expansion checklist and percentage report; retain the original core-v1.0 definition and 48-item denominator.

Negative-hysteresis ngspice switch extensions, nonlinear magnetic cores, and transmission lines remain outside this increment.
