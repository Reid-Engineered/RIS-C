# Compatibility contract, version 0.1

Compatibility means documented equations and numerical agreement on the fixture corpus, not matching internal iteration counts, step choices, or bit patterns. Unsupported features return errors. A model level number alone does not imply support for every ngspice parameter at that level.

## Netlists

- First physical line is the title; `.end` terminates the deck. Content after it is an error.
- ASCII case-insensitive identifiers; node `0` is ground. Comments begin with `*` on their own line, or `$` / `;` inline. `+` continues the preceding statement.
- Numeric suffixes: T, G, MEG, K, M (milli), U, N, P, F, MIL. Expressions should be braced or quoted. Supports `+ - * / ^`, parentheses, `pi`, `abs`, `min`, `max`, `sqrt`, `exp`, `log`/`ln`, `sin`, `cos`. Expressions must be finite. No runtime behavioral expressions.
- `.param` supports forward references and detects cycles. Subcircuit defaults and overrides have instance scope. Subcircuit-local models are scoped. Top-level subcircuit definitions only; nested definitions and recursion are errors. Hierarchical names use `xinstance:device`.
- `.include` / `.inc` paths resolve relative to the including file and preserve case. Include cycles and expansion limits produce errors. `parse_str` has no filesystem include resolver; use `parse_file` for includes. No `.lib` sections or search paths.
- `.model`, `.subckt`, `.ends`, `.op`, one-source `.dc`, `.ac lin|dec|oct`, `.tran step stop [start [maxstep]] [uic]`, `.ic V(node)=value`, `.nodeset V(node)=value`, and one static `.temp` value.
- `.options` supports `reltol`, `vntol`, `abstol`, `itl1`, `method=trap|gear|be`, and `maxord=2`. Gear means BDF2, not arbitrary-order Gear.
- No `.control`, `.print`, `.plot`, `.measure`, `.save`, polynomial sources, vendor dialects, model selection/binning, or parameter sweeps other than an independent voltage/current source. CLI exports all allocated unknowns, including internal nodes.

## Elements

| Syntax | Support |
| --- | --- |
| R name n+ n− value | Positive linear resistance |
| C name n+ n− value [IC=value] | Positive linear capacitance |
| L name n+ n− value [IC=value] | Positive linear inductance |
| V / I | DC, complex AC magnitude/phase, PULSE, SIN, PWL |
| E / G | Four nodes and one linear gain |
| F / H | Two nodes, controlling branch device, gain |
| D | Two nodes, model, optional area, M, TEMP/DTEMP, OFF, and IC |
| Q | C B E [substrate] model, area/M, TEMP/DTEMP, OFF, IC=VBE,VCE; omitted substrate is ground |
| M | D G S B model, L/W/AD/AS/PD/PS/M/NRD/NRS, TEMP/DTEMP, OFF, IC=VDS,VGS,VBS |

RLC temperature coefficients, nonlinear passive elements, mutual inductance, switches, transmission lines, and other families are unsupported. F/H controls must name a device with an allocated branch current (voltage source, inductor, E or H).

Sources require explicit finite pulse rise/fall times greater than zero and a period at least rise+width+fall. All seven PULSE arguments are required. SIN accepts offset, amplitude, frequency, optional delay/damping/phase. PWL times are nonnegative and strictly increasing; duplicate-time discontinuities are rejected. Without a DC field, waveform value at zero supplies the bias. Pulse/SIN/PWL knots are breakpoints; this does not replace an appropriate maximum step for oscillatory inputs.

## Semiconductor model parameters

The [classic electrical audit](LEGACY_PARAMETER_AUDIT.md) defines the legacy completion target. Names below are the canonical accepted sets, plus `LEVEL=1` and standard aliases (including TREF, diode JS/CJ0/PB/MJ, and BJT VA/VB/IK/PE/ME/PC/MC/CCS/PS/MS). Parameters outside these lists are errors, including noise-only parameters even though noise analysis is excluded.

| Model | Accepted parameters |
| --- | --- |
| D | IS N RS CJO VJ M FC TT BV IBV EG XTI TNOM |
| NPN / PNP | IS BF BR NF NR VAF VAR IKF IKR ISE ISC NE NC RB RBM IRB RC RE ISS NS CJE VJE MJE CJC VJC MJC XCJC CJS VJS MJS TF TR XTF VTF ITF PTF FC EG XTI XTB SUBS TNOM |
| NMOS / PMOS | VTO (VT0) KP GAMMA PHI LAMBDA RD RS CBD CBS CJ CJSW MJ MJSW PB FC IS JS CGSO CGDO CGBO TOX LD UO (U0) NSUB TPG NSS RSH TNOM |

Temperature scaling covers the default silicon temperature laws and the listed parameters. Internal series resistance nodes are allocated. BJTs include high injection, Early effect, leakage, nonlinear base resistance, charge storage, substrate current/capacitance, and external base-collector charge splitting. MOS1 includes bidirectional channel operation, body diodes, depletion/overlap capacitances, and history-dependent intrinsic Meyer capacitances when TOX is provided. KP is derived from TOX/UO when KP is absent. Process-derived PHI/GAMMA/VTO through NSUB/TPG/NSS and RSH/NRD/NRS geometry are implemented with explicit-parameter precedence. BJT excess phase PTF is implemented in AC and transient; newer quasi-saturation/alternative temperature-law extensions are not. Default NPN uses a vertical substrate connection; default PNP uses a lateral base connection. SUBS overrides it. Extended diode recombination, sidewall, high-injection, and self-heating parameters are not implemented.

The nonlinear exponential is exact through argument 700 and has a linear overflow guard above it; extremely high forward-bias operating points are outside the validated range.

## Initial conditions and limits

Default transient initialization solves a constrained bias with `.ic` node values, then releases those constraints. Element IC attributes apply only with UIC, as in SPICE. UIC starts from the supplied capacitor/semiconductor voltage and inductor-current history and zero elsewhere; its initial sample need not satisfy every algebraic equation. Voltage-difference constraints are solved together, including floating networks and consistent loops; contradictory loops and conflicts with imposed ideal sources return errors. A free common-mode reference uses the initial node hint or zero. Redundant ICs on an ideal-source component do not replace extra KCL equations. TEMP overrides circuit temperature and DTEMP; without TEMP, DTEMP adds to circuit temperature. OFF is an initialization hint, not a switch that disables device conduction.

Default Newton tolerances: RELTOL=1e-3, VNTOL=1e-6 V, ABSTOL=1e-12 A, 100 iterations. Transient error control has a separate relative tolerance (default 1e-3). `SolverOptions::precise` tightens these for validation. Minimum step, retries, total steps, expansion size, expression size, and grid size are bounded. A failed transient leaves no cached successful bias; callers must treat streamed samples as partial until the operation returns success.

The step-doubling estimate is practical error control, not an absolute bound on accumulated global error. Tighten tolerance and maximum step to verify waveform convergence, especially for long oscillatory runs. Switching fixtures use waveform/timing bounds rather than smooth-signal relative tolerances at discontinuities.
