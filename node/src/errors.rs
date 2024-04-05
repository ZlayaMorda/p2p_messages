use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("Connection closed")]
    TcpClosedError,
    #[error("Write error: {0}")]
    TcpWriteError(std::io::Error),
    #[error("Period set to not correct value")]
    PeriodValueError,
    #[error("Tried connect itself")]
    ItselfConnectionError,
    #[error("{0}")]
    TracingSubscriberError(#[from] tracing::subscriber::SetGlobalDefaultError),
    #[error("Invalid ip v4 address")]
    InvalidIpV4,
    #[error("Unexpected mode number")]
    UnexpectedMode,
}
