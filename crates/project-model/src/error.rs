use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("Could not read or write the project file: {0}")]
    Io(#[from] std::io::Error),
    #[error("This project file is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
}

impl ProjectError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}
