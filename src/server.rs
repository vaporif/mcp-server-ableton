use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::errors::Error;
use crate::osc::OscClient;
use crate::tools::common;
use crate::tools::tracks::{SetTrackNameParams, SetTrackVolumeParams, TrackIndexParams};
use crate::tools::transport::SetTempoParams;

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
    // -- Transport tools --

    #[tool(description = "Start playback in Ableton Live")]
    pub async fn ableton_play(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let (state, summary) = self.do_play().await.map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&state, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Stop playback in Ableton Live")]
    pub async fn ableton_stop(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let (state, summary) = self.do_stop().await.map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&state, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get current session info (tempo, playing state, selected track)")]
    pub async fn ableton_get_tempo(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let osc = self.osc_mcp().await?;
        let summary = common::query_session_summary(osc)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|e| rmcp::ErrorData::from(Error::from(e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Set the tempo in Ableton Live")]
    pub async fn ableton_set_tempo(
        &self,
        Parameters(params): Parameters<SetTempoParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let summary = self
            .do_set_tempo(params.bpm)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|e| rmcp::ErrorData::from(Error::from(e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // -- Track tools --

    #[tool(description = "List all tracks in the Ableton Live session")]
    pub async fn ableton_list_tracks(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let (tracks, summary) = self.do_list_tracks().await.map_err(rmcp::ErrorData::from)?;
        let json =
            common::tool_response_named("tracks", &tracks, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Set a track's volume in Ableton Live")]
    pub async fn ableton_set_track_volume(
        &self,
        Parameters(params): Parameters<SetTrackVolumeParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (mixer, summary) = self
            .do_set_track_volume(params.track, params.volume)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&mixer, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Set a track's name in Ableton Live")]
    pub async fn ableton_set_track_name(
        &self,
        Parameters(params): Parameters<SetTrackNameParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (track_info, summary) = self
            .do_set_track_name(params.track, &params.name)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json =
            common::tool_response_obj(&track_info, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Mute a track in Ableton Live")]
    pub async fn ableton_mute_track(
        &self,
        Parameters(params): Parameters<TrackIndexParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (mixer, summary) = self
            .do_mute_track(params.track)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&mixer, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Unmute a track in Ableton Live")]
    pub async fn ableton_unmute_track(
        &self,
        Parameters(params): Parameters<TrackIndexParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (mixer, summary) = self
            .do_unmute_track(params.track)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&mixer, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for AbletonMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("mcp-server-ableton", env!("CARGO_PKG_VERSION")),
        )
    }
}
