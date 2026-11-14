use risc_c_core::*;
use risc_c_netlist::{Analysis, parse_file};
use std::{
    error::Error,
    fmt::Write as _,
    path::{Path, PathBuf},
};

fn main() {
    if let Err(e) = run() {
        eprintln!("spice: {e}");
        std::process::exit(1);
    }
}
fn usage() -> &'static str {
    "Usage: spice run CIRCUIT [--analysis op|dc|ac|tran] [--output results.csv]\n       spice --version\nRuns declared analyses; multiple analyses produce suffixed files. --output - writes one analysis to stdout."
}
fn run() -> std::result::Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 1 && matches!(args[0].as_str(), "--version" | "-V") {
        println!("spice {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        println!("{}", usage());
        return Ok(());
    }
    if args[0] != "run" || args.len() < 2 {
        return Err(usage().into());
    }
    let path = Path::new(&args[1]);
    let mut output = PathBuf::from("results.csv");
    let mut analysis = None;
    let mut k = 2;
    while k < args.len() {
        if k + 1 >= args.len() {
            return Err(usage().into());
        }
        match args[k].as_str() {
            "--analysis" => {
                if !["op", "dc", "ac", "tran"].contains(&args[k + 1].as_str()) {
                    return Err("unknown analysis".into());
                }
                analysis = Some(args[k + 1].as_str());
            }
            "--output" => output = PathBuf::from(&args[k + 1]),
            _ => return Err(format!("unknown argument {}", args[k]).into()),
        }
        k += 2;
    }
    let parsed = parse_file(path)?;
    let circuit = parsed.circuit.compile()?;
    let mut simulator = circuit.simulator();
    simulator.options = parsed.solver_options;
    let mut analyses = parsed.analyses;
    if let Some(kind) = analysis {
        analyses.retain(|a| a.name() == kind);
        if analyses.is_empty() {
            if kind == "op" {
                analyses.push(Analysis::OperatingPoint);
            } else {
                return Err(format!("no .{kind} analysis declared").into());
            }
        }
    }
    if analyses.is_empty() {
        return Err("no analyses declared; add .op, .dc, .ac, or .tran".into());
    }
    if output == Path::new("-") && analyses.len() != 1 {
        return Err("stdout output requires exactly one analysis".into());
    }
    let mut outputs = Vec::new();
    for (index, a) in analyses.iter().enumerate() {
        let mut csv = String::new();
        match a {
            Analysis::OperatingPoint => {
                header(&mut csv, "operating_point", circuit.signals(), false);
                let s = simulator.operating_point()?;
                row(&mut csv, &s);
            }
            Analysis::Dc {
                source,
                start,
                stop,
                step,
            } => {
                header(&mut csv, source, circuit.signals(), false);
                for s in simulator.dc_sweep(source, *start, *stop, *step)? {
                    row(&mut csv, &s);
                }
            }
            Analysis::Ac(grid) => {
                header(&mut csv, "frequency_hz", circuit.signals(), true);
                for s in simulator.ac(grid)? {
                    write!(&mut csv, "{:.16e}", s.frequency)?;
                    for v in s.values {
                        write!(&mut csv, ",{:.16e},{:.16e}", v.re, v.im)?;
                    }
                    csv.push('\n');
                }
            }
            Analysis::Transient { options, start } => {
                header(&mut csv, "time_s", circuit.signals(), false);
                for s in simulator.transient(options)? {
                    if s.axis >= *start {
                        row(&mut csv, &s);
                    }
                }
            }
        }
        let stats = &simulator.stats;
        eprintln!(
            "{}: {} Newton iterations, {} factorizations, {} accepted / {} rejected steps",
            a.name(),
            stats.newton_iterations,
            stats.factorizations,
            stats.accepted_steps,
            stats.rejected_steps
        );
        let dest = if analyses.len() == 1 {
            output.clone()
        } else {
            let stem = output
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("results");
            let extension = output.extension().and_then(|s| s.to_str()).unwrap_or("csv");
            let duplicate = analyses
                .iter()
                .filter(|other| other.name() == a.name())
                .count()
                > 1;
            let suffix = if duplicate {
                format!("{}-{}", a.name(), index + 1)
            } else {
                a.name().into()
            };
            output.with_file_name(format!("{stem}.{suffix}.{extension}"))
        };
        outputs.push((dest, csv));
    }
    // Finish all requested simulations before exposing any successful result file.
    for (dest, csv) in outputs {
        if dest == Path::new("-") {
            print!("{csv}");
        } else {
            std::fs::write(&dest, csv)?;
            eprintln!("wrote {}", dest.display());
        }
    }
    Ok(())
}
fn escaped(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn header(csv: &mut String, axis: &str, names: &[String], complex: bool) {
    csv.push_str(&escaped(axis));
    for name in names {
        if complex {
            csv.push(',');
            csv.push_str(&escaped(&format!("{name}.re")));
            csv.push(',');
            csv.push_str(&escaped(&format!("{name}.im")));
        } else {
            csv.push(',');
            csv.push_str(&escaped(name));
        }
    }
    csv.push('\n');
}
fn row(csv: &mut String, s: &Sample) {
    write!(csv, "{:.16e}", s.axis).unwrap();
    for v in &s.values {
        write!(csv, ",{v:.16e}").unwrap();
    }
    csv.push('\n');
}
