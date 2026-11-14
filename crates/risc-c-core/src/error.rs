use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum Error {
    #[error("invalid circuit: {0}")]
    Circuit(String),
    #[error("invalid analysis: {0}")]
    Analysis(String),
    #[error("singular or ill-conditioned matrix: {0}")]
    Singular(String),
    #[error("non-finite numerical value in {0}")]
    NonFinite(String),
    #[error("Newton failed after {iterations} iterations (worst scaled residual {residual:.3e})")]
    Convergence { iterations: usize, residual: f64 },
    #[error("time step exhausted at t={time:.9e}: {reason}")]
    TimeStep { time: f64, reason: String },
    #[error("simulation cancelled")]
    Cancelled,
}
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) fn valid(value: f64, name: &str, positive: bool) -> Result<()> {
    if !value.is_finite() || (positive && value <= 0.0) {
        Err(Error::Circuit(format!(
            "{name} must be finite{}",
            if positive { " and positive" } else { "" }
        )))
    } else {
        Ok(())
    }
}
