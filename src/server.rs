use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;

use crate::errors::Error;
use crate::osc::OscClient;
use crate::tools::batch::BatchParams;
use crate::tools::clips::{
    AddNotesParams, AdjustClipSoundParams, ClearAndWriteNotesParams, ClipParams,
    CreateMidiClipParams, CreateMidiClipWithNotesParams, CreateMusicalPhraseParams, GetNotesParams,
};
use crate::tools::common;
use crate::tools::devices::{DeviceParams, SetDeviceParameterParams, SetDeviceParametersParams};
use crate::tools::scenes::SceneIndexParams;
use crate::tools::tracks::{
    CreateFromTemplateParams, SetMixerParams, SetTrackNameParams, SetTrackVolumeParams,
    TrackIndexParams,
};
use crate::tools::transport::SetTempoParams;

#[derive(Clone)]
pub struct AbletonMcpServer {
    osc_cell: Arc<OnceCell<Arc<OscClient>>>,
    cancel: CancellationToken,
    tool_router: ToolRouter<Self>,
}

impl AbletonMcpServer {
    #[must_use]
    pub fn new(cancel: CancellationToken) -> Self {
        let tool_router = Self::tool_router();
        Self {
            osc_cell: Arc::new(OnceCell::new()),
            cancel,
            tool_router,
        }
    }

    pub async fn osc(&self) -> Result<&Arc<OscClient>, Error> {
        self.osc_cell
            .get_or_try_init(|| OscClient::new(self.cancel.child_token()))
            .await
    }

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
        let json = common::tool_response_named("tracks", &tracks, &summary)
            .map_err(rmcp::ErrorData::from)?;
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

    // -- Template tools --

    #[tool(description = "List all template tracks (named with [TPL] prefix) in the session")]
    pub async fn ableton_list_templates(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let (templates, summary) = self
            .do_list_templates()
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_named("templates", &templates, &summary)
            .map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Create a new track from a template track (duplicates and renames)")]
    pub async fn ableton_create_from_template(
        &self,
        Parameters(params): Parameters<CreateFromTemplateParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (track_info, summary) = self
            .do_create_from_template(&params.template_name)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json =
            common::tool_response_obj(&track_info, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // -- Scene tools --

    #[tool(description = "List all scenes in the Ableton Live session")]
    pub async fn ableton_list_scenes(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let (scenes, summary) = self.do_list_scenes().await.map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_named("scenes", &scenes, &summary)
            .map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Fire (launch) a scene in Ableton Live")]
    pub async fn ableton_fire_scene(
        &self,
        Parameters(params): Parameters<SceneIndexParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (state, summary) = self
            .do_fire_scene(params.scene)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&state, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // -- Clip tools --

    #[tool(description = "Fire (launch) a clip in Ableton Live")]
    pub async fn ableton_fire_clip(
        &self,
        Parameters(params): Parameters<ClipParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (state, summary) = self
            .do_fire_clip(params.track, params.slot)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&state, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Stop a clip in Ableton Live")]
    pub async fn ableton_stop_clip(
        &self,
        Parameters(params): Parameters<ClipParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let summary = self
            .do_stop_clip(params.track, params.slot)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|e| rmcp::ErrorData::from(Error::from(e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get the name of a clip in Ableton Live")]
    pub async fn ableton_get_clip_name(
        &self,
        Parameters(params): Parameters<ClipParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (info, summary) = self
            .do_get_clip_name(params.track, params.slot)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&info, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Create a MIDI clip in Ableton Live")]
    pub async fn ableton_create_midi_clip(
        &self,
        Parameters(params): Parameters<CreateMidiClipParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let summary = self
            .do_create_midi_clip(params.track, params.slot, params.length)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|e| rmcp::ErrorData::from(Error::from(e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Add notes to a MIDI clip in Ableton Live")]
    pub async fn ableton_add_notes(
        &self,
        Parameters(params): Parameters<AddNotesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (response, summary) = self
            .do_add_notes(params.track, params.slot, &params.notes)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&response, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get all notes from a MIDI clip in Ableton Live")]
    pub async fn ableton_get_notes(
        &self,
        Parameters(params): Parameters<GetNotesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (response, summary) = self
            .do_get_notes(params.track, params.slot)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&response, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Remove all notes from a MIDI clip in Ableton Live")]
    pub async fn ableton_remove_notes(
        &self,
        Parameters(params): Parameters<ClipParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let summary = self
            .do_remove_notes(params.track, params.slot)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|e| rmcp::ErrorData::from(Error::from(e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // -- Device tools --

    #[tool(description = "List all devices on a track in Ableton Live")]
    pub async fn ableton_list_devices(
        &self,
        Parameters(params): Parameters<TrackIndexParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (response, summary) = self
            .do_list_devices(params.track)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&response, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List all parameters of a device in Ableton Live")]
    pub async fn ableton_list_device_parameters(
        &self,
        Parameters(params): Parameters<DeviceParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (response, summary) = self
            .do_list_device_parameters(params.track, params.device)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&response, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Set a device parameter value in Ableton Live")]
    pub async fn ableton_set_device_parameter(
        &self,
        Parameters(params): Parameters<SetDeviceParameterParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (response, summary) = self
            .do_set_device_parameter(params.track, params.device, params.param, params.value)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&response, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // -- Batch tool --

    #[tool(
        description = "Execute multiple actions in sequence. Supports: play, stop, set_tempo, set_track_volume, set_track_name, mute_track, unmute_track, fire_scene, fire_clip, stop_clip, create_midi_clip, add_notes, remove_notes, set_device_parameter, set_mixer"
    )]
    pub async fn ableton_batch(
        &self,
        Parameters(params): Parameters<BatchParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (results, summary) = self
            .do_batch(&params)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_named("results", &results, &summary)
            .map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // -- Compound read tools --

    #[tool(description = "Get full session state: tempo, tracks with mixer/devices, and scenes")]
    pub async fn ableton_get_session_state(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let state = self
            .do_get_session_state()
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| rmcp::ErrorData::from(Error::from(e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get detailed info for a single track: mixer, devices, and clip slots")]
    pub async fn ableton_get_track_detail(
        &self,
        Parameters(params): Parameters<TrackIndexParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (detail, summary) = self
            .do_get_track_detail(params.track)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&detail, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get full device info including all parameters")]
    pub async fn ableton_get_device_full(
        &self,
        Parameters(params): Parameters<DeviceParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (full, summary) = self
            .do_get_device_full(params.track, params.device)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&full, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // -- Compound write tools --

    #[tool(description = "Create a MIDI clip and add notes in one call")]
    pub async fn ableton_create_midi_clip_with_notes(
        &self,
        Parameters(params): Parameters<CreateMidiClipWithNotesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (response, summary) = self
            .do_create_midi_clip_with_notes(params.track, params.slot, params.length, &params.notes)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&response, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Clear all notes in a clip and write new notes")]
    pub async fn ableton_clear_and_write_notes(
        &self,
        Parameters(params): Parameters<ClearAndWriteNotesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (response, summary) = self
            .do_clear_and_write_notes(params.track, params.slot, &params.notes)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&response, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Set multiple device parameters in one call")]
    pub async fn ableton_set_device_parameters(
        &self,
        Parameters(params): Parameters<SetDeviceParametersParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (response, summary) = self
            .do_set_device_parameters(params.track, params.device, &params.parameters)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&response, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Create a MIDI clip with notes and optionally set device parameters in one call"
    )]
    pub async fn ableton_create_musical_phrase(
        &self,
        Parameters(params): Parameters<CreateMusicalPhraseParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (response, summary) = self
            .do_create_musical_phrase(&params)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&response, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Adjust a clip's notes and/or device parameters. Can add notes, clear and replace notes, and tweak device params."
    )]
    pub async fn ableton_adjust_clip_sound(
        &self,
        Parameters(params): Parameters<AdjustClipSoundParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (response, summary) = self
            .do_adjust_clip_sound(&params)
            .await
            .map_err(rmcp::ErrorData::from)?;
        let json = common::tool_response_obj(&response, &summary).map_err(rmcp::ErrorData::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Set mixer properties (volume, pan, mute, solo) for a track")]
    pub async fn ableton_set_mixer(
        &self,
        Parameters(params): Parameters<SetMixerParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let (mixer, summary) = self
            .do_set_mixer(&params)
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
