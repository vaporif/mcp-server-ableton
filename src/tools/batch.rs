use schemars::JsonSchema;
use serde::Deserialize;

use crate::errors::Error;
use crate::server::AbletonMcpServer;
use crate::tools::clips::Note;
use crate::tools::common::{self, SessionSummary};
use crate::tools::tracks::SetMixerParams;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchParams {
    /// Array of actions to execute sequentially. Each action is a JSON object
    /// with an "action" field plus action-specific fields.
    pub actions: Vec<serde_json::Value>,
    /// Error handling: "continue" (default) executes all actions, "abort" stops on first error
    #[serde(default = "default_on_error")]
    pub on_error: String,
}

fn default_on_error() -> String {
    "continue".to_string()
}

#[derive(Debug, Deserialize)]
struct SetTempoAction {
    bpm: f32,
}

#[derive(Debug, Deserialize)]
struct TrackAction {
    track: i32,
}

#[derive(Debug, Deserialize)]
struct TrackVolumeAction {
    track: i32,
    volume: f32,
}

#[derive(Debug, Deserialize)]
struct TrackNameAction {
    track: i32,
    name: String,
}

#[derive(Debug, Deserialize)]
struct SceneAction {
    scene: i32,
}

#[derive(Debug, Deserialize)]
struct ClipAction {
    track: i32,
    slot: i32,
}

#[derive(Debug, Deserialize)]
struct CreateMidiClipAction {
    track: i32,
    slot: i32,
    length: f32,
}

#[derive(Debug, Deserialize)]
struct AddNotesAction {
    track: i32,
    slot: i32,
    notes: Vec<Note>,
}

#[derive(Debug, Deserialize)]
struct SetDeviceParameterAction {
    track: i32,
    device: i32,
    param: i32,
    value: f32,
}

impl AbletonMcpServer {
    pub async fn do_batch(
        &self,
        params: &BatchParams,
    ) -> Result<(Vec<serde_json::Value>, SessionSummary), Error> {
        let abort_on_error = params.on_error == "abort";
        let mut results = Vec::with_capacity(params.actions.len());

        for action_value in &params.actions {
            let action_name = action_value
                .get("action")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");

            let result = self.dispatch_action(action_name, action_value.clone()).await;

            match result {
                Ok(()) => {
                    results.push(serde_json::json!({
                        "action": action_name,
                        "status": "ok"
                    }));
                }
                Err(e) => {
                    results.push(serde_json::json!({
                        "action": action_name,
                        "status": "error",
                        "error": e.to_string()
                    }));
                    if abort_on_error {
                        break;
                    }
                }
            }
        }

        let osc = self.osc().await?;
        let summary = common::query_session_summary(osc).await?;
        Ok((results, summary))
    }

    async fn dispatch_action(
        &self,
        action: &str,
        value: serde_json::Value,
    ) -> Result<(), Error> {
        match action {
            "play" => {
                self.do_play().await?;
            }
            "stop" => {
                self.do_stop().await?;
            }
            "set_tempo" => {
                let a: SetTempoAction = serde_json::from_value(value)?;
                self.do_set_tempo(a.bpm).await?;
            }
            "set_track_volume" => {
                let a: TrackVolumeAction = serde_json::from_value(value)?;
                self.do_set_track_volume(a.track, a.volume).await?;
            }
            "set_track_name" => {
                let a: TrackNameAction = serde_json::from_value(value)?;
                self.do_set_track_name(a.track, &a.name).await?;
            }
            "mute_track" => {
                let a: TrackAction = serde_json::from_value(value)?;
                self.do_mute_track(a.track).await?;
            }
            "unmute_track" => {
                let a: TrackAction = serde_json::from_value(value)?;
                self.do_unmute_track(a.track).await?;
            }
            "fire_scene" => {
                let a: SceneAction = serde_json::from_value(value)?;
                self.do_fire_scene(a.scene).await?;
            }
            "fire_clip" => {
                let a: ClipAction = serde_json::from_value(value)?;
                self.do_fire_clip(a.track, a.slot).await?;
            }
            "stop_clip" => {
                let a: ClipAction = serde_json::from_value(value)?;
                self.do_stop_clip(a.track, a.slot).await?;
            }
            "create_midi_clip" => {
                let a: CreateMidiClipAction = serde_json::from_value(value)?;
                self.do_create_midi_clip(a.track, a.slot, a.length).await?;
            }
            "add_notes" => {
                let a: AddNotesAction = serde_json::from_value(value)?;
                self.do_add_notes(a.track, a.slot, &a.notes).await?;
            }
            "remove_notes" => {
                let a: ClipAction = serde_json::from_value(value)?;
                self.do_remove_notes(a.track, a.slot).await?;
            }
            "set_device_parameter" => {
                let a: SetDeviceParameterAction = serde_json::from_value(value)?;
                self.do_set_device_parameter(a.track, a.device, a.param, a.value)
                    .await?;
            }
            "set_mixer" => {
                let a: SetMixerParams = serde_json::from_value(value)?;
                self.do_set_mixer(&a).await?;
            }
            unknown => {
                return Err(Error::UnexpectedResponse(format!(
                    "unknown batch action: {unknown}"
                )));
            }
        }
        Ok(())
    }
}
