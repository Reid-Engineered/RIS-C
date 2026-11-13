mod common;
use spice_netlist::*;
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
