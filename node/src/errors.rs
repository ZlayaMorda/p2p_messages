use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("Connection closed")]
    TcpClosedError(),
    #[error("Period must be more than 0")]
    PeriodValueError(),
}
