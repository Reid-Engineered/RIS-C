# Classic electrical-model completion boundary

The core plan targets the classic SPICE junction diode, Gummel–Poon BJT, and Level-1 MOS electrical/charge models. This inventory makes those legacy completeness gates explicit. It does not claim that every newer parameter added to ngspice 47 under these model names is implemented. Noise, self-heating, advanced model families, quasi-saturation extensions, and alternative temperature-law families are outside this classic baseline; unsupported fields remain errors.

Reference equations/defaults come from the pinned ngspice 47 sources listed in [provenance](../reference/PROVENANCE.md). The classic tables can also be checked against the first-party [SPICE 3-derived MacSpice device guide](https://www.macspice.com/ug/sec3.html), sections 3.3.1, 3.3.2, and 3.4.1. Implementation comparisons use ngspice 47, not MacSpice.

| Legacy electrical target | Canonical parameters | Count |
| --- | --- | ---: |
| Junction diode | IS RS N TT CJO VJ M EG XTI FC BV IBV TNOM | 13 |
| Gummel–Poon NPN/PNP | IS BF NF VAF IKF ISE NE BR NR VAR IKR ISC NC RB IRB RBM RE RC CJE VJE MJE TF XTF VTF ITF PTF CJC VJC MJC XCJC TR CJS VJS MJS XTB EG XTI FC TNOM | 39 |
| Level-1 NMOS/PMOS | VTO KP GAMMA PHI LAMBDA RD RS CBD CBS IS PB CGSO CGDO CGBO RSH CJ MJ CJSW MJSW JS TOX LD UO FC NSUB TPG NSS TNOM | 28 |

Noise-only KF/AF are not electrical-target omissions: noise analysis was excluded by the original plan. Level-specific flags and standard aliases are accepted separately. Additional implemented BJT ISS/NS/SUBS support is not used to inflate the 80-parameter classic inventory.

## Executable matrix

`crates/spice-netlist/tests/common/legacy.rs` defines the complete parameter inventory and generates a baseline plus a one-at-a-time perturbation for each canonical parameter. The same inventory is used in the ordinary parser/compilation tests and in the external-oracle tests.

- 152 cards: 14 diode, 40 NPN, 40 PNP, 29 NMOS, and 29 PMOS.
- Every card is compared with ngspice in DC and complex AC: 304 reference analyses. Defaults are combined with nonzero capacitance, resistance, transit-time, and phase values so interactions are exercised too.
- A full composite card for each of the five device types is tested in transient at 0, 27, and 80 C: 15 waveform comparisons. All output points and selected terminal currents are compared. PNP transient substrate bias is chosen reverse-biased, as in normal isolated operation; forward substrate capacitance is additionally exercised in the DC/AC matrix.
- Process-derived MOS values get a separate precedence test across both polarities, three gate types, and three temperatures. Explicit PHI/GAMMA/VTO/KP override derived process values; RD/RS override RSH.
- BJT excess-phase AC and transient behavior has a separate NPN/PNP case and a history-isolation regression.
- TEMP/DTEMP instance precedence and coupled initial voltage constraints have separate tests.

DC/AC current tolerances are `1e-11 A + 1e-6 * magnitude`; the dedicated process/phase tests also check voltage outputs. Transient current comparisons use `1e-9 A + 2e-3 * magnitude` with an explicitly bounded internal step. These are finite fixture acceptance bounds, not a uniform error guarantee for every possible circuit.

A parameter perturbation is compared against the reference even when an explicit override makes that parameter inactive. Separate process and sheet-resistance cases verify those parameters without their overrides. Coverage is not represented as exhaustive combinations of every parameter value or circuit topology.

## Corrections exposed by the matrix

- Default PNP substrate junction connects to the base (lateral); default NPN connects to the collector (vertical). SUBS can explicitly select either configuration.
- MOS gate-to-bulk overlap capacitance uses effective channel length, including LD.
- NSUB/TPG/NSS derive PHI/GAMMA/VTO at nominal temperature only when those values are not explicitly supplied.
- RSH and NRD/NRS generate internal series resistances, scaled by multiplicity.
- PTF uses ngspice's AC excess-phase rotation and transient Weil recurrence with accepted-step history.
- At tiny steps, charge subtraction can reach representational precision before the requested current residual. A bound from charge, Jacobian, and state magnitudes prevents a false timestep collapse; voltage/current update tests remain enforced, and `roundoff_limited_convergences` records use of this bound. A sub-ULP RC regression also verifies that it cannot hide an unresolved ideal-source constraint.

The wider numerical stress corpus separately includes both stable states of a cross-coupled MOS latch, zero-diagonal/strongly-scaled MNA constraints, the coupled fast-diode/slow-RC case, the coupled fast-collector/slow-emitter BJT case, and switching/rejected-step histories. Passing this corpus is core acceptance, not a production-readiness or all-ngspice compatibility claim.
