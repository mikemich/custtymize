//! Error types.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: invalid TOML: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("generated TOML does not parse: {0}")]
    InvalidToml(String),
    #[error("starship rejected the config: {0}")]
    Starship(String),
    #[error("could not run starship: {0}")]
    StarshipMissing(String),
    #[error("cannot find the home directory")]
    NoHome,
    #[error("terminal error: {0}")]
    Terminal(#[from] std::io::Error),
}

impl Error {
    pub fn io(path: &Path, source: std::io::Error) -> Error {
        Error::Io { path: path.to_path_buf(), source }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
