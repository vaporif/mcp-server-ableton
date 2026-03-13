use rmcp::ErrorData as McpError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("config: {0}")]
    Config(String),

    #[error("OSC timeout: AbletonOSC not responding — is the Max for Live device loaded in your Ableton session?")]
    OscTimeout,

    #[error("OSC decode error: {0}")]
    OscDecode(String),

    #[error("unexpected OSC response: {0}")]
    UnexpectedResponse(String),

    #[error("installer error: {0}")]
    Installer(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<Error> for McpError {
    fn from(err: Error) -> Self {
        let message = err.to_string();
        tracing::error!("{message}");
        Self::internal_error(message, None)
    }
}
