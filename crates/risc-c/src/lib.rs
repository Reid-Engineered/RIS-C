#![doc = include_str!("../README.md")]

pub use risc_c_core::*;
/// SPICE parser, declarations, and source-location diagnostics.
pub use risc_c_netlist as netlist;

use netlist::{Analysis, ParsedNetlist};
use std::path::Path;

/// Errors retain parse locations or the index and kind of a failed analysis.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Parse(#[from] netlist::ParseError),
    #[error("circuit compilation failed: {0}")]
    Compile(#[source] Error),
    #[error("no analyses declared; add .op, .dc, .ac, or .tran")]
    NoAnalyses,
    #[error("analysis[{index}] ({kind}) failed: {source}")]
    Analysis {
        /// Zero-based index in declaration order.
        index: usize,
        /// One of `op`, `dc`, `ac`, or `tran`.
        kind: &'static str,
        #[source]
        source: Error,
    },
}

/// Typed samples from one analysis. Values follow [`CompiledCircuit::signals`].
#[derive(Clone, Debug)]
pub enum AnalysisData {
    OperatingPoint(Sample),
    Dc(Vec<Sample>),
    Ac(Vec<AcSample>),
    /// Samples before the declared `.tran` output start are excluded.
    Transient(Vec<Sample>),
}

/// The declaration, samples, and solver statistics for one completed analysis.
#[derive(Clone, Debug)]
pub struct AnalysisResult {
    pub analysis: Analysis,
    pub data: AnalysisData,
    pub stats: RunStats,
}

/// All declared analyses in source order, plus their shared signal definitions.
#[derive(Debug)]
pub struct RunOutput {
    pub circuit: CompiledCircuit,
    pub analyses: Vec<AnalysisResult>,
}

impl RunOutput {
    /// Read the first declared operating-point result, if any, from this completed
    /// run. This accessor does not execute another analysis.
    pub fn operating_point(&self) -> Option<&Sample> {
        self.analyses.iter().find_map(|result| match &result.data {
            AnalysisData::OperatingPoint(point) => Some(point),
            _ => None,
        })
    }
}

/// Parse and run an in-memory deck. The first line is the title; end with `.end`.
///
/// Filesystem `.include` directives require [`run_file`] instead.
pub fn run_str(text: &str) -> std::result::Result<RunOutput, RunError> {
    run(netlist::parse_str(text)?)
}

/// Parse and run a deck, resolving `.include` paths relative to each including file.
pub fn run_file(path: impl AsRef<Path>) -> std::result::Result<RunOutput, RunError> {
    run(netlist::parse_file(path)?)
}

/// Run a parsed deck after optionally adjusting its analyses or solver options.
///
/// Compiles once and runs declarations sequentially with one simulator. Every
/// call starts with a fresh simulator. Returns results only if all analyses
/// succeed; an error never exposes a partial successful run. Samples are kept
/// in memory; use [`Simulator::transient_with`] for large or cancellable runs.
pub fn run(parsed: ParsedNetlist) -> std::result::Result<RunOutput, RunError> {
    if parsed.analyses.is_empty() {
        return Err(RunError::NoAnalyses);
    }
    let circuit = parsed.circuit.compile().map_err(RunError::Compile)?;
    let mut simulator = circuit.simulator();
    simulator.options = parsed.solver_options;
    let mut results = Vec::with_capacity(parsed.analyses.len());
    for (index, analysis) in parsed.analyses.into_iter().enumerate() {
        let execute = |simulator: &mut Simulator| -> Result<AnalysisData> {
            Ok(match &analysis {
                Analysis::OperatingPoint => {
                    AnalysisData::OperatingPoint(simulator.operating_point()?)
                }
                Analysis::Dc {
                    source,
                    start,
                    stop,
                    step,
                } => AnalysisData::Dc(simulator.dc_sweep(source, *start, *stop, *step)?),
                Analysis::Ac(grid) => AnalysisData::Ac(simulator.ac(grid)?),
                Analysis::Transient { options, start } => {
                    let mut samples = simulator.transient(options)?;
                    samples.retain(|sample| sample.axis >= *start);
                    AnalysisData::Transient(samples)
                }
            })
        };
        let data = execute(&mut simulator).map_err(|source| RunError::Analysis {
            index,
            kind: analysis.name(),
            source,
        })?;
        results.push(AnalysisResult {
            analysis,
            data,
            stats: simulator.stats.clone(),
        });
    }
    Ok(RunOutput {
        circuit,
        analyses: results,
    })
}
