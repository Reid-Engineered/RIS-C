use risc_c_core::*;
use risc_c_netlist::*;

fn circuit(text: &str) -> CompiledCircuit {
    parse_str(text).unwrap().circuit.compile().unwrap()
}
fn near(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a:e} vs {b:e}, tol={tol:e}");
}

#[test]
fn coupling_validation_and_hierarchy() {
    for k in [-1.0, 0.0, 1.0] {
        circuit(&format!(
            "K\nK1 L1 L2 {k}\nL1 a 0 1m\nL2 b 0 2m\nR1 a 0 1\nR2 b 0 1\n.end"
        ));
    }
    for body in [
        "K1 L1 L2 1.01",
        "K1 L1 L1 0.5",
        "K1 L1 R1 0.5",
        "K1 L1 missing 0.5",
        "K1 L1 L2 0.5\nK2 L2 L1 0.3",
        "K1 L1 L2 0.9\nK2 L2 L3 0.9\nK3 L1 L3 -0.9",
        "K1 L1 L2 1\nK2 L2 L3 1\nK3 L1 L3 -1",
    ] {
        let text = format!("Bad K\nL1 a 0 1m\nL2 b 0 2m\nL3 c 0 3m\nR1 a 0 1\n{body}\n.end");
        assert!(
            parse_str(&text)
                .map(|p| p.circuit.compile().is_err())
                .unwrap_or(true),
            "{body}"
        );
    }
    let c = circuit(
        "Hierarchy\nX1 a b transformer params: coupling=0.8\nR1 a 0 10\nR2 b 0 10\n.subckt transformer p s params: coupling=1\nK1 Lp Ls {coupling}\nLp p 0 1m\nLs s 0 4m\n.ends\n.end",
    );
    assert!(c.signals().contains(&"i(x1:lp)".into()));
    assert!(c.simulator().operating_point().is_ok());
    circuit(
        "Ideal three-winding\nL1 a 0 1m\nL2 b 0 2m\nL3 c 0 3m\nK1 L1 L2 1\nK2 L1 L3 -1\nK3 L2 L3 -1\n.end",
    );
}

#[test]
fn transformer_ac_transfer_and_polarity() {
    for coupling in [-1.0, -0.8, 0.0, 0.8, 1.0] {
        let c = circuit(&format!(
            "Transfer\nV1 drive 0 AC 1\nRp drive p 1\nL1 p 0 1m\nL2 s 0 4m\nK1 L1 L2 {coupling}\nR1 s 0 100\n.end"
        ));
        let result = c
            .simulator()
            .ac(&FrequencyGrid::Linear {
                points: 3,
                start: 100.0,
                stop: 10000.0,
            })
            .unwrap();
        let idx = c.signals().iter().position(|s| s == "v(s)").unwrap();
        for sample in result {
            let w = std::f64::consts::TAU * sample.frequency;
            let m = 0.002 * coupling;
            let secondary = Complex64::new(100.0, w * 0.004);
            let primary = Complex64::new(1.0, w * 0.001) + w * w * m * m / secondary;
            let expected = Complex64::new(0.0, w * m * 100.0) / secondary / primary;
            near((sample.values[idx] - expected).norm(), 0.0, 1e-10);
        }
    }
}

#[test]
fn coupled_transient_energy_balance() {
    let c = circuit(
        "Energy\nL1 a 0 1m IC=0.1\nL2 b 0 4m IC=-0.02\nK1 L1 L2 0.7\nR1 a 0 10\nR2 b 0 20\n.end",
    );
    for method in [
        IntegrationMethod::BackwardEuler,
        IntegrationMethod::Trapezoidal,
        IntegrationMethod::Bdf2,
    ] {
        let mut sim = c.simulator();
        sim.options = SolverOptions::precise();
        let mut o = TransientOptions::new(0.001, 1e-6);
        o.max_step = 0.5e-6;
        o.uic = true;
        o.method = method;
        let samples = sim.transient(&o).unwrap();
        let energy = |s: &Sample| {
            let a = s.get(&c, "i(l1)").unwrap();
            let b = s.get(&c, "i(l2)").unwrap();
            0.5 * (0.001 * a * a + 0.004 * b * b) + 0.0014 * a * b
        };
        let power = |s: &Sample| {
            let a = s.get(&c, "i(l1)").unwrap();
            let b = s.get(&c, "i(l2)").unwrap();
            10.0 * a * a + 20.0 * b * b
        };
        // UIC's t=0 algebraic voltages are unspecified; branch currents determine power.
        let loss: f64 = samples
            .windows(2)
            .map(|p| 0.5 * (power(&p[0]) + power(&p[1])) * (p[1].axis - p[0].axis))
            .sum();
        let initial = energy(&samples[0]);
        let final_energy = energy(samples.last().unwrap());
        near(initial - final_energy, loss, initial * 0.004);
        for pair in samples.windows(2) {
            assert!(energy(&pair[1]) <= energy(&pair[0]) + initial * 1e-10);
        }
    }
}

#[test]
fn switch_dc_hysteresis_ac_and_current_polarity() {
    for current in [false, true] {
        let control = if current {
            "I1 0 ctrl DC 0 AC 1\nVsense ctrl 0 0\nW1 rail out Vsense gate\n.model gate CSW(RON=1 ROFF=1meg IT=0.5 IH=0.1)"
        } else {
            "Vctrl ctrl 0 DC 0 AC 1\nS1 rail out ctrl 0 gate\n.model gate SW(RON=1 ROFF=1meg VT=0.5 VH=0.1)"
        };
        let c = circuit(&format!(
            "Hysteresis\nVrail rail 0 DC 1 AC 1\nRload out 0 100\n{control}\n.end"
        ));
        let mut sim = c.simulator();
        sim.options = SolverOptions::precise();
        for (start, stop, step) in [(-1.0, 1.0, 0.05), (1.0, -1.0, -0.05)] {
            let samples = sim
                .dc_sweep(if current { "i1" } else { "vctrl" }, start, stop, step)
                .unwrap();
            let mut on = false;
            for p in samples {
                if p.axis > 0.6 {
                    on = true;
                } else if p.axis < 0.4 {
                    on = false;
                }
                near(
                    p.get(&c, "v(out)").unwrap(),
                    100.0 / (100.0 + if on { 1.0 } else { 1e6 }),
                    1e-10,
                );
            }
        }
        let ac = sim
            .ac(&FrequencyGrid::Linear {
                points: 1,
                start: 1000.0,
                stop: 1000.0,
            })
            .unwrap();
        let idx = c.signals().iter().position(|s| s == "v(out)").unwrap();
        near(ac[0].values[idx].re, 100.0 / 1000100.0, 1e-10);
        near(ac[0].values[idx].im, 0.0, 1e-12);
    }
}

#[test]
fn switch_models_and_controls_are_instance_scoped() {
    let c = circuit(
        "Scoped switches\nVrail rail 0 5\nVctrl ctrl 0 0.5\nX1 rail a ctrl relay params: r=2\nX2 rail b ctrl relay params: r=4\nX3 rail c current_relay\nRa a 0 100\nRb b 0 100\nRc c 0 100\n.model local SW(RON=1meg)\n.subckt relay p out ctl params: r=1\nS1 p out ctl 0 local ON\n.model local SW(RON={r} ROFF=1meg VT=0.5 VH=0.1)\n.ends\n.subckt current_relay p out\nI1 0 ctl 0.8\nVsense ctl 0 0\nW1 p out Vsense local\n.model local CSW(RON=3 ROFF=1meg IT=0.5 IH=0.1)\n.ends\n.end",
    );
    let p = c.simulator().operating_point().unwrap();
    for (name, r) in [("v(a)", 2.0), ("v(b)", 4.0), ("v(c)", 3.0)] {
        near(p.get(&c, name).unwrap(), 500.0 / (100.0 + r), 1e-10);
    }
    assert!(c.signals().contains(&"i(x3:vsense)".into()));
}

#[test]
fn switch_event_timing_and_initial_hints() {
    for on in ["ON", "OFF"] {
        let c = circuit(&format!(
            "Hints\nV1 a 0 1\nVctrl ctrl 0 0.5\nR1 out 0 100\nS1 a out ctrl 0 s {on}\n.model s SW(RON=1 ROFF=1meg VT=0.5 VH=0.1)\n.end"
        ));
        let p = c.simulator().operating_point().unwrap();
        near(
            p.get(&c, "v(out)").unwrap(),
            100.0 / (100.0 + if on == "ON" { 1.0 } else { 1e6 }),
            1e-10,
        );
    }
    let c = circuit(
        "Events\nV1 a 0 1\nVctrl ctrl 0 PWL(0 0 1m 1 2m 0)\nR1 out 0 100\nS1 a out ctrl 0 s\n.model s SW(RON=1 ROFF=1meg VT=0.5 VH=0.1)\n.end",
    );
    for method in [
        IntegrationMethod::BackwardEuler,
        IntegrationMethod::Trapezoidal,
        IntegrationMethod::Bdf2,
    ] {
        let mut s = c.simulator();
        let mut o = TransientOptions::new(0.002, 0.000073);
        o.max_step = 0.0003;
        o.method = method;
        let samples = s.transient(&o).unwrap();
        assert_eq!(s.stats.switch_transitions, 2);
        assert!(s.stats.switch_event_steps > 0);
        for p in samples {
            let on = p.axis > 0.0006 && p.axis < 0.0016;
            near(
                p.get(&c, "v(out)").unwrap(),
                100.0 / (100.0 + if on { 1.0 } else { 1e6 }),
                1e-9,
            );
        }
    }
    for body in [
        ".model s SW(VH=-1)",
        ".model s CSW(IH=-1)",
        ".model s SW(RON=0)",
        ".model s SW(IT=1)",
        "S1 a 0 c 0 s maybe\n.model s SW",
        "W1 a 0 missing s\n.model s CSW",
    ] {
        let text = format!("Bad switch\nV1 a 0 1\n{body}\n.end");
        assert!(
            parse_str(&text)
                .map(|p| p.circuit.compile().is_err())
                .unwrap_or(true),
            "{body}"
        );
    }
}

#[test]
fn buck_refinement_and_power_balance() {
    let p = parse_str(include_str!("../../../examples/buck.cir")).unwrap();
    let c = p.circuit.compile().unwrap();
    let mut options = match &p.analyses[0] {
        Analysis::Transient { options, .. } => options.clone(),
        _ => panic!(),
    };
    let mut sim = c.simulator();
    sim.options = p.solver_options.clone();
    sim.options.transient_reltol = 1e-4;
    let actual = sim.transient(&options).unwrap();
    options.max_step *= 0.5;
    let mut refined = c.simulator();
    refined.options = sim.options.clone();
    let expected = refined.transient(&options).unwrap();
    assert!(sim.stats.switch_transitions >= 38);
    for (a, b) in actual.iter().zip(&expected) {
        near(
            a.get(&c, "v(output)").unwrap(),
            b.get(&c, "v(output)").unwrap(),
            0.015,
        );
        near(
            a.get(&c, "i(l1)").unwrap(),
            b.get(&c, "i(l1)").unwrap(),
            0.005,
        );
    }
    let last = actual.last().unwrap().get(&c, "v(output)").unwrap();
    assert!(last > 4.0 && last < 8.0, "output={last}");
    // The input supplies the load, stored energy, and positive diode/switch loss.
    let integrate = |f: &dyn Fn(&Sample) -> f64| {
        actual
            .windows(2)
            .map(|p| 0.5 * (f(&p[0]) + f(&p[1])) * (p[1].axis - p[0].axis))
            .sum::<f64>()
    };
    let supplied = integrate(&|p| -12.0 * p.get(&c, "i(vin)").unwrap());
    let load = integrate(&|p| p.get(&c, "v(output)").unwrap().powi(2) / 10.0);
    let end = actual.last().unwrap();
    let stored = 0.5 * 100e-6 * end.get(&c, "i(l1)").unwrap().powi(2) + 0.5 * 10e-6 * last * last;
    assert!(
        supplied > load + stored - 1e-5,
        "supplied={supplied}, load={load}, stored={stored}"
    );
    assert!(supplied - load - stored < 0.15 * supplied);
}
#[test]
fn discontinuous_pulse_switch_controls_land_on_edges() {
    for control in ["voltage", "current"] {
        for threshold in [0.2, 0.5, 0.8] {
            let switch = if control == "voltage" {
                format!(
                    "Vc ctrl 0 PULSE(0 1 1u 0 0 1u 3u)\nS1 rail out ctrl 0 sw\n.model sw sw(ron=1 roff=1meg vt={threshold})"
                )
            } else {
                format!(
                    "Ic 0 ctrl PULSE(0 1 1u 0 0 1u 3u)\nVsense ctrl 0 0\nW1 rail out Vsense sw\n.model sw csw(ron=1 roff=1meg it={threshold})"
                )
            };
            let c = circuit(&format!(
                "Pulse switches\nVrail rail 0 1\n{switch}\nR1 out 0 100\n.end"
            ));
            for method in [
                IntegrationMethod::BackwardEuler,
                IntegrationMethod::Trapezoidal,
                IntegrationMethod::Bdf2,
            ] {
                let mut s = c.simulator();
                let mut options = TransientOptions::new(6e-6, 0.1e-6);
                options.method = method;
                let samples = s.transient(&options).unwrap();
                assert_eq!(
                    s.stats.switch_transitions, 4,
                    "{control} {threshold} {method:?}"
                );
                assert!(s.stats.rejected_steps < 10, "{:?}", s.stats);
                for sample in samples {
                    if [1e-6, 2e-6, 4e-6, 5e-6]
                        .iter()
                        .any(|t| (sample.axis - t).abs() < 1e-12)
                    {
                        continue; // Conductance changes at the accepted edge endpoint.
                    }
                    let on = (sample.axis > 1e-6 && sample.axis < 2e-6)
                        || (sample.axis > 4e-6 && sample.axis < 5e-6);
                    near(
                        sample.get(&c, "v(out)").unwrap(),
                        100.0 / (100.0 + if on { 1.0 } else { 1e6 }),
                        1e-9,
                    );
                }
            }
        }
    }
}
#[test]
fn smooth_switch_thresholds_near_output_times_do_not_collapse_steps() {
    for current in [false, true] {
        let control = if current {
            "Ictrl 0 ctrl PWL(0 0 1m 1 2m 0)\nVsense ctrl 0 0\nW1 rail out Vsense gate\n.model gate CSW(RON=2 ROFF=1meg IT=0.5 IH=0.1)"
        } else {
            "Vctrl ctrl 0 PWL(0 0 1m 1 2m 0)\nS1 rail out ctrl 0 gate\n.model gate SW(RON=2 ROFF=1meg VT=0.5 VH=0.1)"
        };
        let c = circuit(&format!(
            "Smooth switch\nVrail rail 0 5\nRload out 0 100\nCload out 0 1u\n{control}\n.end"
        ));
        let mut sim = c.simulator();
        sim.options = SolverOptions::precise();
        let mut options = TransientOptions::new(0.0022, 11e-6);
        options.max_step = 1e-6;
        let samples = sim.transient(&options).unwrap();
        assert_eq!(sim.stats.switch_transitions, 2);
        let off = 500.0 / 1_000_100.0;
        let on = 500.0 / 102.0;
        let tau = 1e-6 / (1.0 / 100.0 + 1.0 / 1e6);
        let expected = off + (on - off) * (-0.0006_f64 / tau).exp();
        near(
            samples.last().unwrap().get(&c, "v(out)").unwrap(),
            expected,
            2e-5,
        );
    }
}
