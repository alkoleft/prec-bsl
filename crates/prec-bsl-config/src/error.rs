use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: Option<PathBuf>,
        source: serde_json::Error,
    },
    Validation {
        errors: Vec<String>,
    },
}

impl ConfigError {
    pub fn validation_messages(&self) -> &[String] {
        match self {
            Self::Validation { errors } => errors,
            Self::Io { .. } | Self::Json { .. } => &[],
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::Json { path, source } => {
                if let Some(path) = path {
                    write!(formatter, "failed to parse {}: {source}", path.display())
                } else {
                    write!(formatter, "failed to parse config JSON: {source}")
                }
            }
            Self::Validation { errors } => {
                write!(formatter, "invalid configuration: {}", errors.join("; "))
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Validation { .. } => None,
        }
    }
}
