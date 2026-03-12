use rmcp::ErrorData as McpError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("config: {0}")]
    Config(String),
}

impl From<Error> for McpError {
    fn from(err: Error) -> Self {
        let message = err.to_string();
        tracing::error!("{message}");
        Self::internal_error(message, None)
    }
}
