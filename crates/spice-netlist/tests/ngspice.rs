//! Explicitly opt-in oracle suite: NGSPICE_BIN=/path/to/ngspice cargo test --test ngspice -- --ignored
mod common;
use spice_core::*;
use spice_netlist::*;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn oracle(circuit: &str, command: &str, signals: &str) -> Vec<Vec<f64>> {
    let bin =
        std::env::var("NGSPICE_BIN").expect("set NGSPICE_BIN to the pinned ngspice 47 executable");
    let dir = std::env::temp_dir().join(format!(
        "spice-oracle-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let output = dir.join("data.txt");
    let net = dir.join("input.cir");
    let text = circuit.trim_end().strip_suffix(".end").unwrap();
    std::fs::write(&net,format!("{text}\n.control\nset wr_singlescale\nset wr_vecnames\nset numdgt=15\n{command}\nwrdata {} {signals}\nquit\n.endc\n.end\n",output.display())).unwrap();
    let run = Command::new(PathBuf::from(bin))
        .args(["-b", "-n"])
        .arg(&net)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let text = std::fs::read_to_string(output).unwrap_or_else(|e| {
        panic!(
            "{e}: {} {}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        )
    });
    let data = text
        .lines()
        .skip(1)
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.split_whitespace().map(|n| n.parse().unwrap()).collect())
        .collect();
    std::fs::remove_dir_all(dir).unwrap();
    data
}
fn compare(a: f64, b: f64, rel: f64, abs: f64) {
    assert!(
        (a - b).abs() <= abs + rel * a.abs().max(b.abs()),
        "Rust {a:.12e}, ngspice {b:.12e}"
    );
}
#[test]
#[ignore = "requires pinned external ngspice"]
fn nonlinear_operating_points() {
    let circuits = [
        (
            "Diode\nV1 a 0 5\nR1 a out 1k\nD1 out 0 d\n.model d D(IS=1e-14 N=1.2 RS=2)\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.op\n.end",
            "v(out)",
        ),
        (
            "BJT\nVcc vcc 0 5\nVb b 0 0.7\nR1 vcc c 1k\nQ1 c b 0 q\n.model q NPN(IS=1e-14 BF=150 VAF=100 IKF=0.1)\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.op\n.end",
            "v(c)",
        ),
        (
            "MOS\nVcc vcc 0 5\nVg gate 0 2\nR1 vcc out 1k\nM1 out gate 0 0 n L=1u W=10u\n.model n NMOS(LEVEL=1 VTO=1 KP=100u GAMMA=0.2 PHI=0.6 LAMBDA=0.02)\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.op\n.end",
            "v(out)",
        ),
        (
            "PNP\nVcc vcc 0 5\nVb b 0 4.3\nR1 c 0 1k\nQ1 c b vcc q\n.model q PNP(IS=1e-14 BF=150 VAF=100 IKF=0.1)\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.op\n.end",
            "v(c)",
        ),
    ];
    for (text, signal) in circuits {
        let parsed = parse_str(text).unwrap();
        let c = parsed.circuit.compile().unwrap();
        let mut s = c.simulator();
        s.options = parsed.solver_options;
        let point = s.operating_point().unwrap();
        let expected = oracle(text, "op", signal);
        compare(point.get(&c, signal).unwrap(), expected[0][1], 1e-6, 1e-8);
    }
}
#[test]
#[ignore = "requires pinned external ngspice"]
fn rc_ac_and_transient() {
    let text = "RC\nV1 in 0 DC 0 AC 1 PULSE(0 1 0 1n 1n 10m 20m)\nR1 in out 1k\nC1 out 0 1u\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.end";
    let parsed = parse_str(text).unwrap();
    let c = parsed.circuit.compile().unwrap();
    let k = c.signals().iter().position(|s| s == "v(out)").unwrap();
    let mut s = c.simulator();
    s.options = SolverOptions::precise();
    let ac = s
        .ac(&FrequencyGrid::Decade {
            points_per_decade: 5,
            start: 1.0,
            stop: 1e5,
        })
        .unwrap();
    let expected = oracle(text, "ac dec 5 1 100k", "v(out)");
    assert_eq!(ac.len(), expected.len());
    for (actual, row) in ac.iter().zip(expected) {
        compare(actual.values[k].re, row[1], 1e-6, 1e-8);
        compare(actual.values[k].im, row[2], 1e-6, 1e-8);
    }
    let tran = s.transient(&TransientOptions::new(0.003, 1e-5)).unwrap();
    let expected = oracle(text, "tran 10u 3m 0 1u\nlinearize v(out)", "v(out)");
    for row in expected {
        if let Some(actual) = tran.iter().find(|s| (s.axis - row[0]).abs() < 1e-12) {
            compare(actual.values[k], row[1], 1e-3, 1e-6);
        }
    }
}
#[test]
#[ignore = "requires pinned external ngspice"]
fn temperature_breakdown_and_base_resistance() {
    for temperature in [0, 27, 80] {
        for (body, signal) in [
            (
                "V1 a 0 -10\nR1 a out 1k\nD1 out 0 d\n.model d D(IS=1e-14 N=1.2 BV=5.1 IBV=1m RS=2 CJO=2p TT=1n)",
                "v(out)",
            ),
            (
                "Vcc vcc 0 5\nVb b 0 0.65\nR1 vcc out 1k\nQ1 out b 0 q\n.model q NPN(IS=1e-14 BF=150 VAF=100 IKF=0.1 ISE=1e-14 NE=1.5 RB=100 RBM=10 IRB=10u RC=1 RE=1)",
                "v(out)",
            ),
            (
                "Vcc vcc 0 5\nVg g 0 2\nR1 vcc out 1k\nM1 out g 0 0 n L=1u W=10u\n.model n NMOS(LEVEL=1 VTO=1 TOX=100n GAMMA=0.3 LAMBDA=0.02)",
                "v(out)",
            ),
        ] {
            let text = format!(
                "Temperature\n{body}\n.temp {temperature}\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.end"
            );
            let parsed = parse_str(&text).unwrap();
            let c = parsed.circuit.compile().unwrap();
            let mut s = c.simulator();
            s.options = parsed.solver_options;
            let point = s
                .operating_point()
                .unwrap_or_else(|e| panic!("{text}\n{e}"));
            let expected = oracle(&text, "op", signal);
            compare(point.get(&c, signal).unwrap(), expected[0][1], 1e-6, 1e-8);
        }
    }
}
#[test]
#[ignore = "requires pinned external ngspice"]
fn semiconductor_ac_capacitances() {
    for (body, signal) in [
        (
            "Vcc vcc 0 5\nVb b 0 DC 0.65 AC 1\nR1 vcc out 1k\nQ1 out b 0 q\n.model q NPN(IS=1e-14 BF=150 VAF=100 CJE=10p CJC=3p TF=1n RB=100 RBM=10 IRB=10u XCJC=0.5)",
            "v(out)",
        ),
        (
            "Vcc vcc 0 5\nVg g 0 DC 2 AC 1\nR1 vcc out 1k\nM1 out g 0 0 n L=1u W=10u AD=10p AS=10p\n.model n NMOS(LEVEL=1 VTO=1 KP=100u TOX=100n GAMMA=0.3 LAMBDA=0.02 CGSO=100p CGDO=100p CJ=100u)",
            "v(out)",
        ),
    ] {
        let text =
            format!("AC models\n{body}\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.end");
        let parsed = parse_str(&text).unwrap();
        let c = parsed.circuit.compile().unwrap();
        let k = c.signals().iter().position(|s| s == signal).unwrap();
        let mut s = c.simulator();
        s.options = parsed.solver_options;
        let samples = s
            .ac(&FrequencyGrid::Decade {
                points_per_decade: 2,
                start: 1.0,
                stop: 1e9,
            })
            .unwrap();
        let expected = oracle(&text, "ac dec 2 1 1g", signal);
        assert_eq!(samples.len(), expected.len());
        for (sample, row) in samples.iter().zip(expected) {
            compare(sample.values[k].re, row[1], 1e-6, 1e-8);
            compare(sample.values[k].im, row[2], 1e-6, 1e-8);
        }
    }
}
#[test]
#[ignore = "requires pinned external ngspice"]
fn switching_rejections_and_restart() {
    let text = include_str!("../../../examples/inverter.cir");
    let parsed = parse_str(text).unwrap();
    let c = parsed.circuit.compile().unwrap();
    let mut s = c.simulator();
    s.options = SolverOptions::precise();
    let options = parsed
        .analyses
        .iter()
        .find_map(|a| {
            if let Analysis::Transient { options, .. } = a {
                Some(options)
            } else {
                None
            }
        })
        .unwrap();
    let samples = s.transient(options).unwrap();
    assert!(s.stats.rejected_steps > 0);
    assert!(s.stats.breakpoints > 4);
    let expected = oracle(text, "tran 2n 2u 0 100p\nlinearize v(out)", "v(out)");
    let mut compared = 0;
    for row in expected {
        if let Some(sample) = samples.iter().find(|s| (s.axis - row[0]).abs() < 1e-15) {
            compare(sample.get(&c, "v(out)").unwrap(), row[1], 0.01, 0.02);
            compared += 1;
        }
    }
    assert!(compared > 900);
}

#[test]
#[ignore = "requires pinned external ngspice"]
fn stiff_nonlinear_transient_matches_reference_and_refinement() {
    let text = common::STIFF_NETLIST;
    let (circuit, coarse) = common::stiff_diode_refinement();
    let expected = oracle(
        text,
        "tran 20u 3m 0 1u\nlinearize v(fast) v(slow)",
        "v(fast) v(slow)",
    );
    assert_eq!(
        coarse.len(),
        expected.len(),
        "every requested output sample must be compared"
    );
    for (sample, row) in coarse.iter().zip(expected) {
        compare(sample.axis, row[0], 1e-12, 1e-15);
        compare(sample.get(&circuit, "v(fast)").unwrap(), row[1], 1e-3, 1e-6);
        compare(sample.get(&circuit, "v(slow)").unwrap(), row[2], 1e-3, 1e-6);
    }
    let last = coarse.last().unwrap();
    assert!(last.get(&circuit, "v(fast)").unwrap() > 0.6);
    assert!(last.get(&circuit, "v(slow)").unwrap() > 0.5);
}

#[test]
#[ignore = "requires pinned external ngspice"]
fn continuation_recovery_matches_original_ngspice_bias() {
    for (supply, iterations, method) in [
        (1.0, 8, DcMethod::GminStepping),
        (20.0, 12, DcMethod::SourceStepping),
    ] {
        let text = format!(
            "Continuation recovery\nV1 in 0 {supply}\nR1 in out 10k\nD1 out 0 junction\n.model junction D(IS=1e-14 N=1)\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.end"
        );
        let parsed = parse_str(&text).unwrap();
        let circuit = parsed.circuit.compile().unwrap();
        let mut s = circuit.simulator();
        s.options = parsed.solver_options;
        s.options.max_iterations = iterations;
        let point = s.operating_point().unwrap();
        assert_eq!(s.stats.dc_method, Some(method));
        assert_eq!(s.stats.direct_failures, 1);
        let expected = oracle(&text, "op", "v(out) i(v1)");
        assert_eq!(expected.len(), 1);
        compare(
            point.get(&circuit, "v(out)").unwrap(),
            expected[0][1],
            1e-6,
            1e-8,
        );
        compare(
            point.get(&circuit, "i(v1)").unwrap(),
            expected[0][2],
            1e-6,
            1e-11,
        );
        compare(point.get(&circuit, "v(in)").unwrap(), supply, 0.0, 1e-12);
    }
}

#[test]
#[ignore = "requires pinned external ngspice"]
fn mos1_process_and_sheet_resistance_match_reference() {
    for kind in ["NMOS", "PMOS"] {
        for tpg in [-1, 0, 1] {
            for temp in [0, 27, 80] {
                let p = if kind == "NMOS" { 1.0 } else { -1.0 };
                let text = format!(
                    "MOS process\nVd d 0 {}\nVg g 0 DC {} AC 1\nVb b 0 {}\nM1 d g 0 b process W=10u L=2u NRD=3 NRS=2\n.model process {kind}(TOX=100n NSUB=1e16 NSS=1e10 TPG={tpg} RSH=20 LAMBDA=0.02 CGSO=100p)\n.temp {temp}\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.end",
                    p * 2.0,
                    p * 4.0,
                    -p * 0.2
                );
                let parsed = parse_str(&text).unwrap();
                let c = parsed.circuit.compile().unwrap();
                let mut s = c.simulator();
                s.options = parsed.solver_options;
                let op = s.operating_point().unwrap();
                let reference = oracle(&text, "op", "i(vd)");
                compare(op.get(&c, "i(vd)").unwrap(), reference[0][1], 1e-6, 1e-11);
                let ac = s
                    .ac(&FrequencyGrid::Linear {
                        points: 1,
                        start: 1e6,
                        stop: 1e6,
                    })
                    .unwrap();
                let k = c.signals().iter().position(|n| n == "i(vd)").unwrap();
                let reference = oracle(&text, "ac lin 1 1meg 1meg", "i(vd)");
                compare(ac[0].values[k].re, reference[0][1], 1e-6, 1e-11);
                compare(ac[0].values[k].im, reference[0][2], 1e-6, 1e-11);
            }
        }
    }
}

#[test]
#[ignore = "requires pinned external ngspice"]
fn bjt_excess_phase_matches_reference() {
    for kind in ["NPN", "PNP"] {
        let p = if kind == "NPN" { 1.0 } else { -1.0 };
        let text = format!(
            "Excess phase\nVcc rail 0 {}\nVb b 0 DC {} AC 1 SIN({} {} 5meg)\nR1 rail out 1k\nQ1 out b 0 q\n.model q {kind}(IS=1e-14 BF=100 TF=20n PTF=45 CJE=1p CJC=1p)\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.end",
            p * 5.0,
            p * 0.65,
            p * 0.65,
            p * 1e-3
        );
        let parsed = parse_str(&text).unwrap();
        let c = parsed.circuit.compile().unwrap();
        let mut s = c.simulator();
        s.options = parsed.solver_options;
        let ac = s
            .ac(&FrequencyGrid::Decade {
                points_per_decade: 2,
                start: 1e3,
                stop: 1e8,
            })
            .unwrap();
        let k = c.signals().iter().position(|n| n == "v(out)").unwrap();
        let reference = oracle(&text, "ac dec 2 1k 100meg", "v(out)");
        assert_eq!(ac.len(), reference.len());
        for (sample, row) in ac.iter().zip(reference) {
            compare(sample.values[k].re, row[1], 1e-6, 1e-8);
            compare(sample.values[k].im, row[2], 1e-6, 1e-8);
        }
        let mut options = TransientOptions::new(400e-9, 1e-9);
        options.max_step = 0.25e-9;
        let samples = s.transient(&options).unwrap();
        let reference = oracle(&text, "tran 1n 400n 0 0.125n\nlinearize v(out)", "v(out)");
        assert_eq!(samples.len(), reference.len());
        for (sample, row) in samples.iter().zip(reference) {
            compare(sample.values[k], row[1], 1e-5, 1e-6);
        }
    }
}

#[test]
#[ignore = "requires pinned external ngspice"]
fn classic_parameter_matrix_matches_ngspice() {
    let cases = common::legacy::variants();
    assert_eq!(cases.len(), 152);
    for (kind, parameter, card) in cases {
        let p = if kind == "PNP" || kind == "PMOS" {
            -1.0
        } else {
            1.0
        };
        let temp = if ["eg", "xti", "xtb", "tnom"].contains(&parameter.as_str()) {
            80
        } else {
            27
        };
        let (body, signals) = match kind.as_str() {
            "D" => {
                let bias = if ["bv", "ibv"].contains(&parameter.as_str()) {
                    -5.3
                } else {
                    0.65
                };
                (
                    format!("Vp a 0 DC {bias} AC 1\nD1 a 0 device"),
                    vec!["i(vp)"],
                )
            }
            "NPN" | "PNP" => (
                format!(
                    "Vc c 0 {}\nVb b 0 DC {} AC 1\nVs sub 0 {}\nQ1 c b 0 sub device",
                    p * 2.0,
                    p * 0.65,
                    -p * 0.2
                ),
                vec!["i(vc)", "i(vb)", "i(vs)"],
            ),
            _ => (
                format!(
                    "Vd d 0 {}\nVg g 0 DC {} AC 1\nVb b 0 {}\nM1 d g 0 b device W=10u L=2u AD=10p AS=10p PD=20u PS=20u NRD=3 NRS=2",
                    p * 2.0,
                    p * 2.0,
                    -p * 0.2
                ),
                vec!["i(vd)", "i(vg)", "i(vb)"],
            ),
        };
        let text = format!(
            "Legacy {kind} {parameter}\n{body}\n{card}\n.temp {temp}\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.end"
        );
        let parsed = parse_str(&text).unwrap();
        let circuit = parsed.circuit.compile().unwrap();
        let mut s = circuit.simulator();
        s.options = parsed.solver_options;
        let point = s.operating_point().unwrap();
        let expected = oracle(&text, "op", &signals.join(" "));
        for (k, signal) in signals.iter().enumerate() {
            let a = point.get(&circuit, signal).unwrap();
            let b = expected[0][k + 1];
            assert!(
                (a - b).abs() <= 1e-11 + 1e-6 * a.abs().max(b.abs()),
                "DC {kind}/{parameter}/{signal}: {a} vs {b}"
            );
        }
        let samples = s
            .ac(&FrequencyGrid::Linear {
                points: 1,
                start: 1e7,
                stop: 1e7,
            })
            .unwrap();
        let expected = oracle(&text, "ac lin 1 10meg 10meg", &signals.join(" "));
        for (k, signal) in signals.iter().enumerate() {
            let column = circuit.signals().iter().position(|n| n == signal).unwrap();
            let a = samples[0].values[column];
            let b = Complex64::new(expected[0][k * 2 + 1], expected[0][k * 2 + 2]);
            assert!(
                (a - b).norm() <= 1e-11 + 1e-6 * a.norm().max(b.norm()),
                "AC {kind}/{parameter}/{signal}: {a} vs {b}"
            );
        }
    }
}

#[test]
#[ignore = "requires pinned external ngspice"]
fn numerical_stress_corpus_matches_ngspice() {
    let latch = include_str!("../../../examples/bistable-latch.cir");
    for flipped in [false, true] {
        let text = if flipped {
            latch.replace("V(a)=3.3 V(b)=0", "V(a)=0 V(b)=3.3")
        } else {
            latch.into()
        };
        let p = parse_str(&text).unwrap();
        let c = p.circuit.compile().unwrap();
        let mut s = c.simulator();
        s.options = p.solver_options;
        let actual = s.operating_point().unwrap();
        let expected = oracle(&text, "op", "v(a) v(b)");
        compare(actual.get(&c, "v(a)").unwrap(), expected[0][1], 1e-6, 1e-8);
        compare(actual.get(&c, "v(b)").unwrap(), expected[0][2], 1e-6, 1e-8);
    }
    for resistance in [1e-3, 1.0, 1e6, 1e12] {
        let text = format!(
            "Scaled pivots\nV1 a b 1\nRa a 0 1e12\nRb b 0 1e-3\nE1 out 0 a b 1e6\nRout out 0 {resistance}\n.options reltol=1e-8 vntol=1e-10 abstol=1e-20\n.end"
        );
        let p = parse_str(&text).unwrap();
        let c = p.circuit.compile().unwrap();
        let mut s = c.simulator();
        s.options = p.solver_options;
        let actual = s.operating_point().unwrap();
        let expected = oracle(&text, "op", "v(out) i(v1)");
        compare(
            actual.get(&c, "v(out)").unwrap(),
            expected[0][1],
            1e-6,
            1e-8,
        );
        compare(
            actual.get(&c, "i(v1)").unwrap(),
            expected[0][2],
            1e-6,
            1e-20,
        );
    }
    let text = include_str!("../../../examples/stiff-bjt.cir");
    let p = parse_str(text).unwrap();
    let c = p.circuit.compile().unwrap();
    let mut options = p
        .analyses
        .iter()
        .find_map(|a| match a {
            Analysis::Transient { options, .. } => Some(options.clone()),
            _ => None,
        })
        .unwrap();
    let mut s = c.simulator();
    s.options = p.solver_options;
    let actual = s.transient(&options).unwrap();
    assert!(s.stats.minimum_step.unwrap() < 1e-8);
    options.max_step = 25e-6;
    let refined = s.transient(&options).unwrap();
    let expected = oracle(
        text,
        "tran 100u 50m 0 20u\nlinearize v(collector) v(emitter)",
        "v(collector) v(emitter)",
    );
    assert_eq!(actual.len(), expected.len());
    assert_eq!(actual.len(), refined.len());
    for ((sample, fine), row) in actual.iter().zip(refined).zip(expected) {
        for (k, signal) in ["v(collector)", "v(emitter)"].iter().enumerate() {
            compare(
                sample.get(&c, signal).unwrap(),
                fine.get(&c, signal).unwrap(),
                1e-3,
                1e-6,
            );
            compare(sample.get(&c, signal).unwrap(), row[k + 1], 1e-3, 1e-6);
        }
    }
}

#[test]
#[ignore = "requires pinned external ngspice"]
fn classic_transient_temperature_matrix_matches_ngspice() {
    let mut worst = std::collections::BTreeMap::new();
    for (kind, _, card) in common::legacy::variants()
        .into_iter()
        .filter(|(_, parameter, _)| parameter == "baseline")
    {
        for temp in [0, 27, 80] {
            let p = if kind == "PNP" || kind == "PMOS" {
                -1.0
            } else {
                1.0
            };
            let (body, signals) = match kind.as_str() {
                "D" => (
                    "Vp a 0 DC 0.55 SIN(0.55 0.01 20meg)\nD1 a 0 device".into(),
                    vec!["i(vp)"],
                ),
                "NPN" | "PNP" => (
                    format!(
                        "Vc c 0 {}\nVb b 0 DC {} SIN({} {} 20meg)\nVs sub 0 {}\nQ1 c b 0 sub device",
                        p * 2.0,
                        p * 0.65,
                        p * 0.65,
                        p * 0.001,
                        -p * 0.2
                    ),
                    vec!["i(vc)", "i(vb)", "i(vs)"],
                ),
                _ => (
                    format!(
                        "Vd d 0 {}\nVg g 0 DC {} SIN({} {} 20meg)\nVb b 0 {}\nM1 d g 0 b device W=10u L=2u AD=10p AS=10p PD=20u PS=20u NRD=3 NRS=2",
                        p * 2.0,
                        p * 2.0,
                        p * 2.0,
                        p * 0.01,
                        -p * 0.2
                    ),
                    vec!["i(vd)", "i(vg)", "i(vb)"],
                ),
            };
            let text = format!(
                "Transient matrix {kind} at {temp}C\n{body}\n{card}\n.temp {temp}\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.end"
            );
            let parsed = parse_str(&text).unwrap();
            let c = parsed.circuit.compile().unwrap();
            let mut s = c.simulator();
            s.options = parsed.solver_options;
            let mut options = TransientOptions::new(50e-9, 0.5e-9);
            options.max_step = 0.0125e-9;
            let actual = s.transient(&options).unwrap();
            let expected = oracle(
                &text,
                &format!("tran 0.5n 50n 0 0.00625n\nlinearize {}", signals.join(" ")),
                &signals.join(" "),
            );
            assert_eq!(actual.len(), expected.len());
            for (sample, row) in actual.iter().zip(expected) {
                for (k, signal) in signals.iter().enumerate() {
                    let a = sample.get(&c, signal).unwrap();
                    let b = row[k + 1];
                    let error = (a - b).abs();
                    if error > 1e-9 + 2e-3 * a.abs().max(b.abs()) {
                        let key = format!("{kind}/{temp}/{signal}");
                        let old = worst.entry(key).or_insert((0.0, 0.0, 0.0, 0.0));
                        if error > old.0 {
                            *old = (error, sample.axis, a, b);
                        }
                    }
                }
            }
        }
    }
    assert!(worst.is_empty(), "worst matrix errors: {worst:?}");
}

#[test]
#[ignore = "requires pinned external ngspice"]
fn instance_temperature_overrides_match_ngspice() {
    for setting in ["TEMP=60", "DTEMP=20", "TEMP=60 DTEMP=20"] {
        for (kind, instance, signals) in [
            ("D", "D1 a 0 device", "i(va)"),
            ("NPN", "Q1 a b 0 device", "i(va)"),
            ("NMOS", "M1 a b 0 0 device W=10u L=2u", "i(va)"),
        ] {
            let parameters = match kind {
                "D" => "IS=1e-14 RS=2",
                "NPN" => "IS=1e-14 BF=150 VAF=100",
                "NMOS" => "VTO=1 KP=100u",
                _ => unreachable!(),
            };
            let bias = if kind == "D" { 0.65 } else { 2.0 };
            let gate = if kind == "NMOS" { 2.0 } else { 0.65 };
            let text = format!(
                "Instance temp\nVa a 0 {bias}\nVb b 0 {gate}\n{instance} {setting}\n.model device {kind}({parameters})\n.temp 40\n.options reltol=1e-8 vntol=1e-10 abstol=1e-14\n.end"
            );
            let p = parse_str(&text).unwrap();
            let c = p.circuit.compile().unwrap();
            let mut s = c.simulator();
            s.options = p.solver_options;
            let actual = s.operating_point().unwrap();
            let expected = oracle(&text, "op", signals);
            compare(
                actual.get(&c, signals).unwrap(),
                expected[0][1],
                1e-6,
                1e-11,
            );
        }
    }
}
