use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool_handler, tool_router, ServerHandler};
use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::errors::Error;
use crate::osc::OscClient;

#[derive(Clone)]
pub struct AbletonMcpServer {
    #[allow(dead_code)]
    config: Arc<Config>,
    osc_cell: Arc<OnceCell<Arc<OscClient>>>,
    cancel: CancellationToken,
    tool_router: ToolRouter<Self>,
}

impl AbletonMcpServer {
    #[must_use]
    pub fn new(config: Arc<Config>, cancel: CancellationToken) -> Self {
        let tool_router = Self::tool_router();
        Self {
            config,
            osc_cell: Arc::new(OnceCell::new()),
            cancel,
            tool_router,
        }
    }

    /// Get or lazily initialize the `OscClient`.
    pub async fn osc(&self) -> Result<&Arc<OscClient>, Error> {
        self.osc_cell
            .get_or_try_init(|| OscClient::new(self.cancel.child_token()))
            .await
    }

    /// Convenience: get `OscClient` with `McpError` conversion for use in tool handlers.
    #[allow(dead_code)]
    pub async fn osc_mcp(&self) -> Result<&Arc<OscClient>, rmcp::ErrorData> {
        self.osc().await.map_err(rmcp::ErrorData::from)
    }
}

#[tool_router]
impl AbletonMcpServer {
    // TODO: add tools here
}

#[tool_handler]
impl ServerHandler for AbletonMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("mcp-server-ableton", env!("CARGO_PKG_VERSION")),
        )
    }
}
