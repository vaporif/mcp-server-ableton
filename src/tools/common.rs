use std::sync::Arc;

use serde::Serialize;

use crate::errors::Error;
use crate::osc::OscClient;

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub tempo: f64,
    pub is_playing: bool,
    pub selected_track: i32,
}

/// Query the session summary from Ableton: tempo, is_playing, selected_track.
pub async fn query_session_summary(osc: &Arc<OscClient>) -> Result<SessionSummary, Error> {
    let tempo: f64 = osc.query_val("/live/song/get/tempo", vec![]).await?;
    let is_playing: bool = osc.query_val("/live/song/get/is_playing", vec![]).await?;
    let selected_track: i32 = osc
        .query_val("/live/view/get/selected_track", vec![])
        .await?;

    Ok(SessionSummary {
        tempo,
        is_playing,
        selected_track,
    })
}

/// Build a JSON tool response with data + session summary.
/// For array data, wraps in `{key: [...], session_summary: {...}}`.
pub fn tool_response_named<T: Serialize>(
    key: &str,
    data: &T,
    summary: &SessionSummary,
) -> Result<String, Error> {
    let obj = serde_json::json!({
        key: serde_json::to_value(data)?,
        "session_summary": serde_json::to_value(summary)?,
    });
    Ok(serde_json::to_string_pretty(&obj)?)
}

/// Build a JSON tool response where data is already a JSON object.
/// Inserts session_summary into the object.
pub fn tool_response_obj(
    data: &impl Serialize,
    summary: &SessionSummary,
) -> Result<String, Error> {
    let mut value = serde_json::to_value(data)?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "session_summary".to_string(),
            serde_json::to_value(summary)?,
        );
    }
    Ok(serde_json::to_string_pretty(&value)?)
}
