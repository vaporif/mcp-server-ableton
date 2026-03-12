use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool_handler, tool_router, ServerHandler};

use crate::config::Config;

#[derive(Clone)]
pub struct AbletonMcpServer {
    #[allow(dead_code)]
    config: Arc<Config>,
    tool_router: ToolRouter<Self>,
}

impl AbletonMcpServer {
    #[must_use]
    pub fn new(config: Arc<Config>) -> Self {
        let tool_router = Self::tool_router();
        Self {
            config,
            tool_router,
        }
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
