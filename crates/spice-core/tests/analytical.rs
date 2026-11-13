use spice_core::*;
fn near(a: f64, b: f64, eps: f64) {
    assert!((a - b).abs() < eps, "{a:e} vs {b:e}, tolerance {eps:e}");
}
fn rc() -> CompiledCircuit {
    let mut b = CircuitBuilder::new();
    let input = b.node("in");
    let output = b.node("out");
    let z = NodeId::GROUND;
    b.add(
        "v1",
        &[input, z],
        Element::Voltage(Source {
            dc: 0.0,
            ac: Complex64::new(1.0, 0.0),
            waveform: Some(Waveform::Pwl(vec![(0.0, 0.0), (1e-9, 1.0)])),
        }),
    )
    .unwrap();
    b.add("r1", &[input, output], Element::Resistor(1000.0))
        .unwrap();
    b.add(
        "c1",
        &[output, z],
        Element::Capacitor {
            capacitance: 1e-6,
            ic: None,
        },
    )
    .unwrap();
    b.build().compile().unwrap()
}
#[test]
fn divider_and_source_current() {
    let mut b = CircuitBuilder::new();
    let a = b.node("a");
    let o = b.node("out");
    let z = NodeId::GROUND;
    b.add("v1", &[a, z], Element::Voltage(10.0.into())).unwrap();
    b.add("r1", &[a, o], Element::Resistor(1000.0)).unwrap();
    b.add("r2", &[o, z], Element::Resistor(1000.0)).unwrap();
    let c = b.build().compile().unwrap();
    let mut s = c.simulator();
    let p = s.operating_point().unwrap();
    near(p.get(&c, "v(out)").unwrap(), 5.0, 1e-10);
    near(p.get(&c, "i(v1)").unwrap(), -0.005, 1e-12);
    let sweep = s.dc_sweep("v1", 0.0, 10.0, 1.0).unwrap();
    assert_eq!(sweep.len(), 11);
    near(sweep[8].get(&c, "v(out)").unwrap(), 4.0, 1e-10);
}
#[test]
fn ac_rc_cutoff() {
    let c = rc();
    let mut s = c.simulator();
    let f = 1.0 / (std::f64::consts::TAU * 1e-3);
    let p = s
        .ac(&FrequencyGrid::Linear {
            points: 1,
            start: f,
            stop: f,
        })
        .unwrap();
    let k = c.signals().iter().position(|s| s == "v(out)").unwrap();
    near(p[0].values[k].norm(), 1.0 / 2.0_f64.sqrt(), 1e-10);
    near(p[0].values[k].arg(), -std::f64::consts::FRAC_PI_4, 1e-10);
}
#[test]
fn rc_transient_all_methods() {
    for method in [
        IntegrationMethod::BackwardEuler,
        IntegrationMethod::Trapezoidal,
        IntegrationMethod::Bdf2,
    ] {
        let c = rc();
        let mut s = c.simulator();
        s.options = SolverOptions::precise();
        let mut o = TransientOptions::new(0.003, 0.0001);
        o.method = method;
        let samples = s.transient(&o).unwrap();
        assert_eq!(samples.len(), 31);
        let v = samples.last().unwrap().get(&c, "v(out)").unwrap();
        near(
            v,
            1.0 - (-3.0_f64).exp(),
            if method == IntegrationMethod::BackwardEuler {
                0.004
            } else {
                0.0005
            },
        );
        assert!(s.stats.breakpoints >= 1);
    }
}
#[test]
fn singular_is_error() {
    let mut b = CircuitBuilder::new();
    let a = b.node("a");
    let c = b.node("b");
    b.add("r", &[a, c], Element::Resistor(1.0)).unwrap();
    let mut s = b.build().compile().unwrap().simulator();
    assert!(s.operating_point().is_err());
}
#[test]
fn diode_bias() {
    let mut b = CircuitBuilder::new();
    let a = b.node("in");
    let out = b.node("out");
    let z = NodeId::GROUND;
    b.add("v", &[a, z], Element::Voltage(5.0.into())).unwrap();
    b.add("r", &[a, out], Element::Resistor(1000.0)).unwrap();
    b.add(
        "d",
        &[out, z],
        Element::Semiconductor {
            model: Model::new(ModelKind::Diode),
            geometry: Default::default(),
        },
    )
    .unwrap();
    let c = b.build().compile().unwrap();
    let mut s = c.simulator();
    s.options = SolverOptions::precise();
    let p = s.operating_point().unwrap();
    let v = p.get(&c, "v(out)").unwrap();
    let i = 1e-14 * ((v / ((1.38064852e-23 / 1.6021766208e-19) * 300.15)).exp() - 1.0) + 1e-12 * v;
    near((5.0 - v) / 1000.0, i, 1e-11);
}
#[test]
fn dependent_sources() {
    let mut b = CircuitBuilder::new();
    let a = b.node("a");
    let o = b.node("o");
    let z = NodeId::GROUND;
    b.add("v", &[a, z], Element::Voltage(2.0.into())).unwrap();
    b.add("e", &[o, z, a, z], Element::Vcvs(3.0)).unwrap();
    b.add("r", &[o, z], Element::Resistor(1000.0)).unwrap();
    let c = b.build().compile().unwrap();
    let p = c.simulator().operating_point().unwrap();
    near(p.get(&c, "v(o)").unwrap(), 6.0, 1e-10);
}
#[test]
fn concurrent_instances_and_cancellation() {
    let c = rc();
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let c = c.clone();
            std::thread::spawn(move || c.simulator().operating_point().unwrap().values)
        })
        .collect();
    assert_eq!(handles[0].thread().name(), None);
    let values: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(values[0], values[1]);
    let mut s = c.simulator();
    assert!(matches!(
        s.transient_with(&TransientOptions::new(1e-3, 1e-5), |_| {
            std::ops::ControlFlow::Break(())
        }),
        Err(Error::Cancelled)
    ));
    assert!(s.operating_point().is_ok());
}
#[test]
fn rl_and_rlc_transient() {
    for resistance in [10.0, 100.0] {
        let mut b = CircuitBuilder::new();
        let a = b.node("in");
        let o = b.node("out");
        let z = NodeId::GROUND;
        b.add(
            "v",
            &[a, z],
            Element::Voltage(Source {
                dc: 0.0,
                ac: Complex64::new(1.0, 0.0),
                waveform: Some(Waveform::Pwl(vec![(0.0, 0.0), (1e-9, 1.0)])),
            }),
        )
        .unwrap();
        b.add("r", &[a, o], Element::Resistor(resistance)).unwrap();
        b.add(
            "l",
            &[o, z],
            Element::Inductor {
                inductance: 0.01,
                ic: None,
            },
        )
        .unwrap();
        let c = b.build().compile().unwrap();
        let mut s = c.simulator();
        s.options = SolverOptions::precise();
        let tau = 0.01 / resistance;
        let samples = s
            .transient(&TransientOptions::new(3.0 * tau, tau / 20.0))
            .unwrap();
        near(
            samples.last().unwrap().get(&c, "i(l)").unwrap(),
            (1.0 - (-3.0_f64).exp()) / resistance,
            1e-5,
        );
    }
    // Underdamped series RLC, analytical unit-step capacitor voltage.
    let mut b = CircuitBuilder::new();
    let a = b.node("in");
    let m = b.node("mid");
    let o = b.node("out");
    let z = NodeId::GROUND;
    b.add(
        "v",
        &[a, z],
        Element::Voltage(Source {
            dc: 0.0,
            ac: Default::default(),
            waveform: Some(Waveform::Pwl(vec![(0.0, 0.0), (1e-10, 1.0)])),
        }),
    )
    .unwrap();
    b.add("r", &[a, m], Element::Resistor(10.0)).unwrap();
    b.add(
        "l",
        &[m, o],
        Element::Inductor {
            inductance: 0.01,
            ic: None,
        },
    )
    .unwrap();
    b.add(
        "c",
        &[o, z],
        Element::Capacitor {
            capacitance: 1e-6,
            ic: None,
        },
    )
    .unwrap();
    let c = b.build().compile().unwrap();
    let mut s = c.simulator();
    s.options = SolverOptions::precise();
    s.options.transient_reltol = 1e-5;
    let samples = s.transient(&TransientOptions::new(1e-3, 1e-5)).unwrap();
    let alpha = 500.0_f64;
    let w = (1e8 - alpha * alpha).sqrt();
    let t = 1e-3;
    let expected = 1.0 - (-alpha * t).exp() * ((w * t).cos() + alpha / w * (w * t).sin());
    near(
        samples.last().unwrap().get(&c, "v(out)").unwrap(),
        expected,
        0.002,
    );
}
#[test]
fn conflicting_initial_conditions() {
    let mut b = CircuitBuilder::new();
    let a = b.node("a");
    b.initial_voltage(a, 2.0).unwrap();
    b.add(
        "c",
        &[a, NodeId::GROUND],
        Element::Capacitor {
            capacitance: 1e-6,
            ic: Some(1.0),
        },
    )
    .unwrap();
    b.add("r", &[a, NodeId::GROUND], Element::Resistor(1e3))
        .unwrap();
    let mut s = b.build().compile().unwrap().simulator();
    let mut o = TransientOptions::new(1e-3, 1e-5);
    o.uic = true;
    assert!(s.transient(&o).is_err());
}
#[test]
fn tolerances_and_limits_are_checked() {
    let c = rc();
    let mut s = c.simulator();
    s.options.reltol = f64::NAN;
    assert!(s.operating_point().is_err());
    s.options = SolverOptions::precise();
    let mut o = TransientOptions::new(1.0, 1e-5);
    o.max_steps = 1;
    assert!(matches!(s.transient(&o), Err(Error::TimeStep { .. })));
    assert!(
        FrequencyGrid::Decade {
            points_per_decade: 1,
            start: 0.0,
            stop: 1.0
        }
        .values()
        .is_err()
    );
}
#[test]
fn current_controlled_and_transconductance_sources() {
    let mut b = CircuitBuilder::new();
    let a = b.node("a");
    let f = b.node("f");
    let g = b.node("g");
    let h = b.node("h");
    let z = NodeId::GROUND;
    b.add("v1", &[a, z], Element::Voltage(2.0.into())).unwrap();
    b.add("ra", &[a, z], Element::Resistor(1000.0)).unwrap();
    b.add(
        "f1",
        &[f, z],
        Element::Cccs {
            control: "v1".into(),
            gain: 3.0,
        },
    )
    .unwrap();
    b.add("rf", &[f, z], Element::Resistor(1000.0)).unwrap();
    b.add("g1", &[g, z, a, z], Element::Vccs(0.001)).unwrap();
    b.add("rg", &[g, z], Element::Resistor(1000.0)).unwrap();
    b.add(
        "h1",
        &[h, z],
        Element::Ccvs {
            control: "v1".into(),
            gain: 100.0,
        },
    )
    .unwrap();
    b.add("rh", &[h, z], Element::Resistor(1000.0)).unwrap();
    let c = b.build().compile().unwrap();
    let p = c.simulator().operating_point().unwrap();
    near(p.get(&c, "v(f)").unwrap(), 6.0, 1e-10);
    near(p.get(&c, "v(g)").unwrap(), -2.0, 1e-10);
    near(p.get(&c, "v(h)").unwrap(), -0.2, 1e-10);
}
#[test]
fn floating_capacitor_ic_respects_existing_hint() {
    let mut b = CircuitBuilder::new();
    let a = b.node("a");
    let o = b.node("out");
    b.initial_voltage(a, 2.0).unwrap();
    b.add("r", &[a, NodeId::GROUND], Element::Resistor(1000.0))
        .unwrap();
    b.add("r2", &[o, NodeId::GROUND], Element::Resistor(1000.0))
        .unwrap();
    b.add(
        "c",
        &[a, o],
        Element::Capacitor {
            capacitance: 1e-6,
            ic: Some(1.0),
        },
    )
    .unwrap();
    let c = b.build().compile().unwrap();
    let mut s = c.simulator();
    let mut o = TransientOptions::new(1e-3, 1e-5);
    o.uic = true;
    let samples = s.transient(&o).unwrap();
    near(samples[0].get(&c, "v(out)").unwrap(), 1.0, 1e-10);
}

#[test]
fn octave_grid_matches_rc_transfer_function() {
    for (start, stop, count) in [(1.0, 16.0, 4), (0.1, 1.6, 3), (10.0, 50.0, 2)] {
        let grid = FrequencyGrid::Octave {
            points_per_octave: count,
            start,
            stop,
        };
        let frequencies = grid.values().unwrap();
        let expected = ((stop / start).log2() * count as f64).floor() as usize + 1;
        assert_eq!(frequencies.len(), expected);
        near(frequencies[0], start, 1e-14);
        for pair in frequencies.windows(2) {
            near(pair[1] / pair[0], 2.0_f64.powf(1.0 / count as f64), 1e-14);
        }
        assert!(*frequencies.last().unwrap() <= stop * (1.0 + 1e-14));
        let c = rc();
        let samples = c.simulator().ac(&grid).unwrap();
        let k = c.signals().iter().position(|s| s == "v(out)").unwrap();
        for (sample, frequency) in samples.iter().zip(frequencies) {
            near(sample.frequency, frequency, 1e-14);
            let expected = Complex64::new(1.0, 0.0)
                / Complex64::new(1.0, std::f64::consts::TAU * frequency * 1e-3);
            assert!((sample.values[k] - expected).norm() < 1e-12);
        }
    }
    assert_eq!(
        FrequencyGrid::Octave {
            points_per_octave: 4,
            start: 2.0,
            stop: 2.0
        }
        .values()
        .unwrap(),
        vec![2.0]
    );
    for grid in [
        FrequencyGrid::Octave {
            points_per_octave: 0,
            start: 1.0,
            stop: 2.0,
        },
        FrequencyGrid::Octave {
            points_per_octave: 2,
            start: 0.0,
            stop: 2.0,
        },
        FrequencyGrid::Octave {
            points_per_octave: 2,
            start: 2.0,
            stop: 1.0,
        },
    ] {
        assert!(grid.values().is_err());
    }
}

#[test]
fn ac_restores_nominal_nonlinear_bias_after_sweep() {
    let mut b = CircuitBuilder::new();
    let input = b.node("in");
    let out = b.node("out");
    let ground = NodeId::GROUND;
    b.add(
        "v",
        &[input, ground],
        Element::Voltage(Source {
            dc: 0.1,
            ac: Complex64::new(1.0, 0.0),
            waveform: None,
        }),
    )
    .unwrap();
    b.add("r", &[input, out], Element::Resistor(1000.0))
        .unwrap();
    b.add(
        "d",
        &[out, ground],
        Element::Semiconductor {
            model: Model::new(ModelKind::Diode),
            geometry: Default::default(),
        },
    )
    .unwrap();
    let c = b.build().compile().unwrap();
    let mut reference = c.simulator();
    reference.options = SolverOptions::precise();
    let grid = FrequencyGrid::Linear {
        points: 1,
        start: 100.0,
        stop: 100.0,
    };
    let expected = reference.ac(&grid).unwrap();
    let k = c.signals().iter().position(|s| s == "v(out)").unwrap();
    let mut reused = c.simulator();
    reused.options = SolverOptions::precise();
    let swept = reused.dc_sweep("v", 0.1, 5.0, 0.1).unwrap();
    assert!(swept.last().unwrap().get(&c, "v(out)").unwrap() > 0.6);
    let actual = reused.ac(&grid).unwrap();
    assert!((actual[0].values[k] - expected[0].values[k]).norm() < 1e-10);
    assert!(
        actual[0].values[k].norm() > 0.99,
        "must not linearize at the final swept 5 V bias"
    );
    let nominal = reused.operating_point().unwrap();
    near(nominal.get(&c, "v(in)").unwrap(), 0.1, 1e-12);
}
