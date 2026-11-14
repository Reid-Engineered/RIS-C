use risc_c::{AnalysisData, Complex64, Error, RunError, netlist, run, run_file, run_str};

#[test]
fn declared_analyses_keep_order_units_start_filter_and_separate_stats() {
    let output = run_str("Mixed analyses\nV1 in 0 DC 10 AC 1\nR1 in out 1k\nR2 out 0 1k\nC1 out 0 1n\n.op\n.dc V1 0 10 5\n.ac lin 1 1k 1k\n.tran 0.1u 1u 0.5u 0.1u\n.end").unwrap();
    let c = &output.circuit;
    assert_eq!(
        output
            .analyses
            .iter()
            .map(|r| r.analysis.name())
            .collect::<Vec<_>>(),
        ["op", "dc", "ac", "tran"]
    );
    let op = output.operating_point().unwrap();
    assert!((op.get(c, "v(out)").unwrap() - 5.0).abs() < 1e-9);
    let AnalysisData::Dc(dc) = &output.analyses[1].data else {
        panic!()
    };
    assert_eq!(
        dc.iter().map(|s| s.axis).collect::<Vec<_>>(),
        [0.0, 5.0, 10.0]
    );
    let AnalysisData::Ac(ac) = &output.analyses[2].data else {
        panic!()
    };
    assert_eq!(ac[0].frequency, 1000.0);
    let expected =
        Complex64::new(1.0, 0.0) / Complex64::new(2.0, std::f64::consts::TAU * 1000.0 * 1e-6);
    assert!((ac[0].get(c, "V(OUT)").unwrap() - expected).norm() < 1e-10);
    assert_eq!(ac[0].get(c, "v(0)"), Some(Complex64::new(0.0, 0.0)));
    assert_eq!(ac[0].get(c, "missing"), None);
    let AnalysisData::Transient(tran) = &output.analyses[3].data else {
        panic!()
    };
    assert!(tran.iter().all(|s| s.axis >= 0.5e-6));
    assert_eq!(tran.last().unwrap().axis, 1e-6);
    assert_eq!(output.analyses[0].stats.accepted_steps, 0);
    assert!(output.analyses[3].stats.accepted_steps > 0);
}

#[test]
fn errors_preserve_source_locations_and_failed_analysis_identity() {
    let RunError::Parse(error) = run_str("Title\n.unimplemented\n.end").unwrap_err() else {
        panic!()
    };
    assert_eq!(error.span.line, 2);
    assert!(matches!(
        run_str("Title\nR1 a 0 1k\n.end"),
        Err(RunError::NoAnalyses)
    ));
    assert!(matches!(
        run_str("Title\nR1 a b 1k\n.op\n.end"),
        Err(RunError::Analysis {
            index: 0,
            kind: "op",
            source: Error::Singular(_)
        })
    ));
    assert!(matches!(
        run_str("Title\nV1 a 0 1\nR1 a 0 1k\n.op\n.tran 0 1\n.end"),
        Err(RunError::Analysis {
            index: 1,
            kind: "tran",
            ..
        })
    ));
}

#[test]
fn parsed_decks_can_override_options_and_repeat_without_shared_state() {
    let deck = "Title\nV1 a 0 10\nR1 a 0 1k\n.op\n.end";
    let mut parsed = netlist::parse_str(deck).unwrap();
    parsed.solver_options.max_iterations = 0;
    assert!(matches!(
        run(parsed),
        Err(RunError::Analysis {
            source: Error::Analysis(_),
            ..
        })
    ));
    for _ in 0..2 {
        let output = run_str(deck).unwrap();
        assert!(
            (output
                .operating_point()
                .unwrap()
                .get(&output.circuit, "v(a)")
                .unwrap()
                - 10.0)
                .abs()
                < 1e-12
        );
    }
}

#[test]
fn files_resolve_includes_relative_to_the_deck() {
    let dir = std::env::temp_dir().join(format!("risc-c-runner-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Load.INC"), "R1 in out 1k\nR2 out 0 1k\n").unwrap();
    let deck = "Divider\nV1 in 0 10\n.include \"Load.INC\" ; preserve case\n.op\n.end";
    std::fs::write(dir.join("divider.cir"), deck).unwrap();
    let output = run_file(dir.join("divider.cir")).unwrap();
    assert!(
        (output
            .operating_point()
            .unwrap()
            .get(&output.circuit, "v(out)")
            .unwrap()
            - 5.0)
            .abs()
            < 1e-9
    );
    assert!(matches!(run_str(deck), Err(RunError::Parse(_))));
    std::fs::remove_dir_all(dir).unwrap();
}
