use std::sync::Arc;

use rmcp::model::{CallToolResult, Content};
use serde::Serialize;

use crate::errors::Error;
use crate::osc::OscClient;

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub tempo: f64,
    pub is_playing: bool,
    pub selected_track: i32,
}

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

fn tool_response_named<T: Serialize>(
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

fn tool_response_obj(data: &impl Serialize, summary: &SessionSummary) -> Result<String, Error> {
    let mut value = serde_json::to_value(data)?;
    let obj = value.as_object_mut().ok_or_else(|| {
        Error::UnexpectedResponse(
            "tool_response_obj requires data that serializes to a JSON object".into(),
        )
    })?;
    obj.insert(
        "session_summary".to_string(),
        serde_json::to_value(summary)?,
    );
    Ok(serde_json::to_string_pretty(&value)?)
}

pub fn call_result_obj(
    data: &impl Serialize,
    summary: &SessionSummary,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let json = tool_response_obj(data, summary)?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

pub fn call_result_named(
    key: &str,
    data: &impl Serialize,
    summary: &SessionSummary,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let json = tool_response_named(key, data, summary)?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

pub fn call_result_json(data: &impl Serialize) -> Result<CallToolResult, rmcp::ErrorData> {
    let json = serde_json::to_string_pretty(data).map_err(Error::from)?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::*;

    fn test_summary() -> SessionSummary {
        SessionSummary {
            tempo: 120.0,
            is_playing: true,
            selected_track: 0,
        }
    }

    #[test]
    fn tool_response_named_has_key_and_summary() {
        let summary = test_summary();
        let data = vec!["a", "b"];
        let json_str = tool_response_named("tracks", &data, &summary).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert!(parsed["tracks"].is_array());
        assert_eq!(parsed["tracks"][0], "a");
        assert_eq!(parsed["tracks"][1], "b");
        assert!(parsed["session_summary"].is_object());
        assert_eq!(parsed["session_summary"]["tempo"], 120.0);
        assert_eq!(parsed["session_summary"]["is_playing"], true);
    }

    #[test]
    fn tool_response_obj_merges_summary() {
        #[derive(Serialize)]
        struct Data {
            name: String,
            index: i32,
        }

        let summary = test_summary();
        let data = Data {
            name: "Track 1".into(),
            index: 0,
        };
        let json_str = tool_response_obj(&data, &summary).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["name"], "Track 1");
        assert_eq!(parsed["index"], 0);
        assert!(parsed.get("session_summary").is_some());
        assert_eq!(parsed["session_summary"]["selected_track"], 0);
    }
}
