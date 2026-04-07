use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MempalaceError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    WalkDir(#[from] walkdir::Error),
}

pub type Result<T> = std::result::Result<T, MempalaceError>;

impl MempalaceError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}
