use std::{error::Error, fmt};

/// CLI exit-code mapping에 사용하는 안정된 오류 분류이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    Cli(String),
    Config(String),
    Auth(String),
    Network(String),
    ApiStructure(String),
    Normalize(String),
    Sqlite(String),
    Package(String),
    Io(String),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, message) = match self {
            Self::Cli(message) => ("CLI", message),
            Self::Config(message) => ("configuration", message),
            Self::Auth(message) => ("authentication", message),
            Self::Network(message) => ("network", message),
            Self::ApiStructure(message) => ("API structure", message),
            Self::Normalize(message) => ("normalization", message),
            Self::Sqlite(message) => ("SQLite", message),
            Self::Package(message) => ("package", message),
            Self::Io(message) => ("I/O", message),
        };
        write!(formatter, "{kind} error: {message}")
    }
}

impl Error for PipelineError {}
