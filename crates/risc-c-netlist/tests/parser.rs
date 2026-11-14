mod common;
use risc_c_netlist::*;
#[test]
fn hierarchy_and_scoping() {
    let p=parse_str("Hierarchy\n.param r=1k\nV1 in 0 10\nX1 in out divider params: r={r*2}\nRload out 0 2k\n.subckt divider a b params: r=1k\nR1 a b {r}\n.ends divider\n.op\n.end").unwrap();
    let c = p.circuit.compile().unwrap();
    let s = c.simulator().operating_point().unwrap();
    assert!((s.get(&c, "v(out)").unwrap() - 5.0).abs() < 1e-10);
    assert!(p.source_map.contains_key("x1:r1"));
}
#[test]
fn rejects_unsupported_fields() {
    for text in [
        "Title\nV1 a 0 1\n.model m nmos(level=3)\n.end",
        "Title\nV1 a 0 1\nR1 a 0 1k bad=2\n.end",
        "Title\n.control\n.end",
        "Title\n.param a={b} b={a}\n.end",
        "Title\n.subckt s a\nX1 a s\n.ends\nX1 x s\n.end",
    ] {
        assert!(parse_str(text).is_err(), "{text}");
    }
}
#[test]
fn model_cards_and_continuations() {
    let p = parse_str(
        "Diode\nV1 a 0 1\nR1 a out 1k\nD1 out 0 d\n.model d D (is=1e-14\n+ n=1 rs=1)\n.op\n.end",
    )
    .unwrap();
    let c = p.circuit.compile().unwrap();
    assert!(c.simulator().operating_point().is_ok());
}
#[test]
fn initial_conditions() {
    let p = parse_str("IC\nR1 a 0 1k\nC1 a 0 1u ic=1\n.tran 10u 1m uic\n.end").unwrap();
    let c = p.circuit.compile().unwrap();
    let mut s = c.simulator();
    if let Analysis::Transient { options, .. } = &p.analyses[0] {
        let samples = s.transient(options).unwrap();
        assert!((samples[0].get(&c, "v(a)").unwrap() - 1.0).abs() < 1e-10);
    } else {
        panic!();
    }
}
#[test]
fn include_cycle_and_case_sensitive_path() {
    let dir = std::env::temp_dir().join(format!("spice-include-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Models.INC"), ".model small D(is=1e-14)\n").unwrap();
    std::fs::write(
        dir.join("top.cir"),
        "Title\n.include \"Models.INC\"\nV1 a 0 0.5\nD1 a 0 small\n.op\n.end\n",
    )
    .unwrap();
    assert!(parse_file(dir.join("top.cir")).is_ok());
    for include in [
        ".include \"Models.INC\" ; comment",
        ".include 'Models.INC' $ comment",
        ".inc Models.INC;comment",
        ".inc Models.INC $ comment",
    ] {
        std::fs::write(
            dir.join("top.cir"),
            format!("Title\n{include}\nV1 a 0 0.5\nD1 a 0 small\n.op\n.end\n"),
        )
        .unwrap();
        assert!(parse_file(dir.join("top.cir")).is_ok(), "{include}");
    }
    std::fs::write(dir.join("Models.INC"), ".include top.cir\n").unwrap();
    assert!(
        parse_file(dir.join("top.cir"))
            .unwrap_err()
            .to_string()
            .contains("include cycle")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn example_netlists_compile() {
    for text in [
        include_str!("../../../examples/rc.cir"),
        include_str!("../../../examples/amplifier.cir"),
        include_str!("../../../examples/inverter.cir"),
    ] {
        let p = parse_str(text).unwrap();
        let c = p.circuit.compile().unwrap();
        assert!(c.simulator().operating_point().is_ok());
    }
}
#[test]
fn malformed_inputs_return_errors() {
    let mut state = 12345_u64;
    let alphabet = b"abc0123.=()+-*/^{}' \n";
    for _ in 0..2000 {
        let mut text = String::from("Title\n");
        for _ in 0..40 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push(alphabet[(state >> 32) as usize % alphabet.len()] as char);
        }
        text.push_str("\n.end");
        let _ = parse_str(&text);
    }
    assert!(parse_str("Title\n.model d D(is=1e-14\n.end").is_err());
}
#[test]
fn deep_expressions_are_bounded() {
    let expression = format!("{}1{}", "(".repeat(100), ")".repeat(100));
    assert!(
        parse_str(&format!("Title\n.param r={{{expression}}}\n.end"))
            .unwrap_err()
            .to_string()
            .contains("nesting limit")
    );
}

#[test]
fn stiff_diode_transient_converges_under_refinement() {
    let (_, samples) = common::stiff_diode_refinement();
    assert_eq!(samples.len(), 151);
    assert_eq!(samples.last().unwrap().axis, 0.003);
}

#[test]
fn instance_temperature_and_initial_voltage_networks() {
    let text = "Device ICs\nVdd rail 0 5\nR1 a 0 1k\nR2 b 0 2k\nR3 c 0 3k\nC1 a b 1n ic=1\nC2 b c 1n ic=2\nC3 a c 1n ic=3\nRaux aux 0 1k\nD1 aux 0 d TEMP=40 OFF\n.model d D(IS=1e-14)\n.ic v(c)=0.25\n.tran 1n 2n uic\n.end";
    let parsed = parse_str(text).unwrap();
    let c = parsed.circuit.compile().unwrap();
    let options = match &parsed.analyses[0] {
        Analysis::Transient { options, .. } => options,
        _ => panic!(),
    };
    let samples = c.simulator().transient(options).unwrap();
    for (name, value) in [("v(a)", 3.25), ("v(b)", 2.25), ("v(c)", 0.25)] {
        assert!((samples[0].get(&c, name).unwrap() - value).abs() < 1e-12);
    }
    let conflict = text.replace("ic=3", "ic=4");
    let p = parse_str(&conflict).unwrap();
    let c = p.circuit.compile().unwrap();
    assert!(c.simulator().transient(options).is_err());
    let source_ic =
        "Source IC\nV1 a 0 2\nC1 a b 1n\nR1 b 0 1k\n.ic v(a)=2 v(b)=1\n.tran 1n 2n\n.end";
    let p = parse_str(source_ic).unwrap();
    let c = p.circuit.compile().unwrap();
    let o = match &p.analyses[0] {
        Analysis::Transient { options, .. } => options,
        _ => panic!(),
    };
    assert!(c.simulator().transient(o).is_ok());
    let p=parse_str("MOS IC\nM1 d g 0 b n IC=0.2,0.1,-0.1 DTEMP=5 OFF\nRd d 0 1k\nRg g 0 1k\nRb b 0 1k\n.model n NMOS(VTO=1)\n.tran 1n 2n uic\n.end").unwrap();
    let c = p.circuit.compile().unwrap();
    let o = match &p.analyses[0] {
        Analysis::Transient { options, .. } => options,
        _ => panic!(),
    };
    let s = c.simulator().transient(o).unwrap();
    assert!((s[0].get(&c, "v(d)").unwrap() - 0.2).abs() < 1e-12);
}

#[test]
fn bare_off_and_braced_device_initial_conditions() {
    let p = parse_str("Device hints\n.param gain=0.05\nD1 a 0 diode OFF\nQ1 c b 0 transistor OFF 2 IC={gain * 4},{gain * 2}\nM1 d g 0 body mos OFF IC=0.2,{gain * 2},-0.1\nRa a 0 1k\nRc c 0 1k\nRb b 0 1k\nRd d 0 1k\nRg g 0 1k\nRbody body 0 1k\n.model diode D\n.model transistor NPN\n.model mos NMOS\n.tran 1n 2n uic\n.end").unwrap();
    let c = p.circuit.compile().unwrap();
    let options = match &p.analyses[0] {
        Analysis::Transient { options, .. } => options,
        _ => panic!(),
    };
    let samples = c.simulator().transient(options).unwrap();
    for (name, expected) in [
        ("v(c)", 0.1),
        ("v(b)", 0.2),
        ("v(d)", 0.2),
        ("v(g)", 0.1),
        ("v(body)", -0.1),
    ] {
        assert!(
            (samples[0].get(&c, name).unwrap() - expected).abs() < 1e-12,
            "{name}"
        );
    }
}

#[test]
fn every_classic_electrical_parameter_is_accepted() {
    let cases = common::legacy::variants();
    assert_eq!(cases.len(), 152);
    for (kind, parameter, card) in cases {
        let nodes = match kind.as_str() {
            "D" => "D1 a 0 device",
            "NPN" | "PNP" => "Q1 a b 0 device",
            _ => "M1 a b 0 0 device L=2u W=10u",
        };
        let text = format!("Legacy {kind} {parameter}\nVa a 0 0\nVb b 0 0\n{nodes}\n{card}\n.end");
        let p = parse_str(&text).unwrap_or_else(|e| panic!("{text}\n{e}"));
        p.circuit.compile().unwrap();
    }
}

#[test]
fn bistable_latch_and_scaled_pivots() {
    let text = include_str!("../../../examples/bistable-latch.cir");
    for flipped in [false, true] {
        let deck = if flipped {
            text.replace("V(a)=3.3 V(b)=0", "V(a)=0 V(b)=3.3")
        } else {
            text.into()
        };
        let p = parse_str(&deck).unwrap();
        let c = p.circuit.compile().unwrap();
        let mut s = c.simulator();
        s.options = p.solver_options;
        let point = s.operating_point().unwrap();
        let a = point.get(&c, "v(a)").unwrap();
        let b = point.get(&c, "v(b)").unwrap();
        let (high, low) = if flipped { (b, a) } else { (a, b) };
        assert!(
            high > 3.2 && low < 0.2,
            "selected stable state: a={a}, b={b}"
        );
    }
    for resistance in [1e-3, 1.0, 1e6, 1e12] {
        let deck = format!(
            "Scaled pivots\nV1 a b 1\nRa a 0 1e12\nRb b 0 1e-3\nE1 out 0 a b 1e6\nRout out 0 {resistance}\n.op\n.end"
        );
        let p = parse_str(&deck).unwrap();
        let c = p.circuit.compile().unwrap();
        let point = c.simulator().operating_point().unwrap();
        assert!((point.get(&c, "v(out)").unwrap() - 1e6).abs() < 1e-8);
        assert!((point.get(&c, "v(b)").unwrap() + 1e-15).abs() < 1e-25);
    }
}

#[test]
fn pulse_zero_edges_omitted_arguments_and_validation() {
    let square = parse_str(
        "Zero-edge pulse\nV1 in 0 PULSE(0 1 0 0 0 1u 2u)\nR1 in 0 1k\n.tran 0.5u 4u\n.end",
    )
    .unwrap();
    let circuit = square.circuit.compile().unwrap();
    let options = match &square.analyses[0] {
        Analysis::Transient { options, .. } => options,
        _ => panic!(),
    };
    let samples = circuit.simulator().transient(options).unwrap();
    for (time, expected) in [
        (0.0, 1.0),
        (0.5e-6, 1.0),
        (1.0e-6, 0.0),
        (1.5e-6, 0.0),
        (2.0e-6, 1.0),
        (2.5e-6, 1.0),
        (3.0e-6, 0.0),
        (3.5e-6, 0.0),
    ] {
        let sample = samples
            .iter()
            .find(|s| (s.axis - time).abs() < 1e-12)
            .unwrap_or_else(|| panic!("missing sample at {time}"));
        assert!(
            (sample.get(&circuit, "v(in)").unwrap() - expected).abs() < 1e-12,
            "at t={time}: got {}, expected {expected}",
            sample.get(&circuit, "v(in)").unwrap()
        );
    }

    let single_pulse =
        parse_str("Single pulse\nV1 in 0 PULSE(0 5 1n 1n 1n 10n)\nR1 in 0 1k\n.tran 1n 100n\n.end")
            .unwrap();
    let circuit = single_pulse.circuit.compile().unwrap();
    let options = match &single_pulse.analyses[0] {
        Analysis::Transient { options, .. } => options,
        _ => panic!(),
    };
    let samples = circuit.simulator().transient(options).unwrap();
    for (time, expected) in [
        (0.0, 0.0),
        (1e-9, 0.0),
        (2e-9, 5.0),
        (12e-9, 5.0),
        (13e-9, 0.0),
        (50e-9, 0.0),
        (100e-9, 0.0),
    ] {
        let sample = samples
            .iter()
            .find(|s| (s.axis - time).abs() < 1e-18)
            .unwrap_or_else(|| panic!("missing sample at {time}"));
        assert!(
            (sample.get(&circuit, "v(in)").unwrap() - expected).abs() < 1e-12,
            "at t={time}: got {}, expected {expected}",
            sample.get(&circuit, "v(in)").unwrap()
        );
    }

    let short_tran =
        parse_str("Short tran\nV1 in 0 PULSE(0 5 1n 1n 1n 10n)\nR1 in 0 1k\n.tran 1n 5n\n.end")
            .unwrap();
    let circuit = short_tran.circuit.compile().unwrap();
    let options = match &short_tran.analyses[0] {
        Analysis::Transient { options, .. } => options,
        _ => panic!(),
    };
    let samples = circuit.simulator().transient(options).unwrap();
    assert!((samples.last().unwrap().get(&circuit, "v(in)").unwrap() - 5.0).abs() < 1e-12);

    let step_two_args =
        parse_str("Two args\nV1 in 0 PULSE(0 5)\nR1 in 0 1k\n.tran 1n 10n\n.end").unwrap();
    let circuit = step_two_args.circuit.compile().unwrap();
    let options = match &step_two_args.analyses[0] {
        Analysis::Transient { options, .. } => options,
        _ => panic!(),
    };
    let samples = circuit.simulator().transient(options).unwrap();
    assert_eq!(samples[0].get(&circuit, "v(in)").unwrap(), 0.0);
    assert!((samples[1].get(&circuit, "v(in)").unwrap() - 5.0).abs() < 1e-12);
    assert_eq!(samples.last().unwrap().get(&circuit, "v(in)").unwrap(), 0.0);

    let delayed_step =
        parse_str("Three args\nV1 in 0 PULSE(0 5 2n)\nR1 in 0 1k\n.tran 1n 10n\n.end").unwrap();
    let circuit = delayed_step.circuit.compile().unwrap();
    let options = match &delayed_step.analyses[0] {
        Analysis::Transient { options, .. } => options,
        _ => panic!(),
    };
    let samples = circuit.simulator().transient(options).unwrap();
    for (time, expected) in [
        (0.0, 0.0),
        (1e-9, 0.0),
        (2e-9, 0.0),
        (3e-9, 5.0),
        (10e-9, 5.0),
    ] {
        let sample = samples
            .iter()
            .find(|s| (s.axis - time).abs() < 1e-18)
            .unwrap();
        assert!((sample.get(&circuit, "v(in)").unwrap() - expected).abs() < 1e-12);
    }

    for op_deck in [
        "Op two args\nV1 in 0 PULSE(0 5)\nR1 in 0 1k\n.op\n.end",
        "Op three args\nV1 in 0 PULSE(0 5 1n)\nR1 in 0 1k\n.op\n.end",
        "Op six args\nV1 in 0 PULSE(0 5 1n 1n 1n 10n)\nR1 in 0 1k\n.op\n.end",
    ] {
        let parsed = parse_str(op_deck).unwrap();
        let circuit = parsed.circuit.compile().unwrap();
        let op = circuit.simulator().operating_point().unwrap();
        assert_eq!(op.get(&circuit, "v(in)").unwrap(), 0.0);
    }

    for invalid in [
        "Title\nV1 a 0 PULSE(0)\n.end",
        "Title\nV1 a 0 PULSE(0 1 0 1n 1n 2n 5n 99)\n.end",
        "Title\nV1 a 0 PULSE(0 1 -1n 1n 1n 2n 5n)\n.end",
        "Title\nV1 a 0 PULSE(0 1 0 -1n 1n 2n 5n)\n.end",
        "Title\nV1 a 0 PULSE(0 1 0 1n -1n 2n 5n)\n.end",
        "Title\nV1 a 0 PULSE(0 1 0 1n 1n -2n 5n)\n.end",
        "Title\nV1 a 0 PULSE(0 1 0 0 0 0 0)\n.end",
    ] {
        assert!(parse_str(invalid).is_err(), "{invalid}");
    }
}
#[test]
fn bjt_temperature_transients_survive_output_alignment() {
    for (kind, _, card) in common::legacy::variants()
        .into_iter()
        .filter(|(k, p, _)| (k == "NPN" || k == "PNP") && p == "baseline")
    {
        for temp in [0, 27, 80] {
            let p = if kind == "PNP" { -1.0 } else { 1.0 };
            let deck = format!(
                "BJT alignment\nVc c 0 {}\nVb b 0 DC {} SIN({} {} 20meg)\nVs sub 0 {}\nQ1 c b 0 sub device\n{card}\n.temp {temp}\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.end",
                p * 2.0,
                p * 0.65,
                p * 0.65,
                p * 0.001,
                if kind == "PNP" { -3.0 } else { -0.2 }
            );
            let parsed = parse_str(&deck).unwrap();
            let c = parsed.circuit.compile().unwrap();
            let mut sim = c.simulator();
            sim.options = parsed.solver_options;
            let mut options = risc_c_core::TransientOptions::new(10e-9, 0.5e-9);
            options.max_step = 0.0125e-9;
            let actual = sim
                .transient(&options)
                .unwrap_or_else(|e| panic!("{kind}/{temp}: {e}"));
            options.max_step *= 0.5;
            let refined = sim.transient(&options).unwrap();
            for (a, b) in actual.iter().zip(refined) {
                for signal in ["i(vc)", "i(vb)", "i(vs)"] {
                    let va = a.get(&c, signal).unwrap();
                    let vb = b.get(&c, signal).unwrap();
                    assert!(
                        (va - vb).abs() <= 1e-9 + 2e-3 * va.abs().max(vb.abs()),
                        "{kind}/{temp} {signal} at {}: {va} vs {vb}",
                        a.axis
                    );
                }
            }
        }
    }
}
