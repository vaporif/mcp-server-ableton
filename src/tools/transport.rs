use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::Error;
use crate::server::AbletonMcpServer;
use crate::tools::common::{self, SessionSummary};

#[derive(Debug, Serialize)]
pub struct TransportState {
    pub is_playing: bool,
    pub tempo: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTempoParams {
    /// Tempo in BPM (e.g., 120.0)
    pub bpm: f32,
}

impl AbletonMcpServer {
    pub async fn do_play(&self) -> Result<(TransportState, SessionSummary), Error> {
        let osc = self.osc().await?;
        osc.send("/live/play", vec![]).await?;
        let summary = common::query_session_summary(osc).await?;
        let state = TransportState {
            is_playing: summary.is_playing,
            tempo: summary.tempo,
        };
        Ok((state, summary))
    }

    pub async fn do_stop(&self) -> Result<(TransportState, SessionSummary), Error> {
        let osc = self.osc().await?;
        osc.send("/live/stop", vec![]).await?;
        let summary = common::query_session_summary(osc).await?;
        let state = TransportState {
            is_playing: summary.is_playing,
            tempo: summary.tempo,
        };
        Ok((state, summary))
    }

    pub async fn do_set_tempo(&self, bpm: f32) -> Result<SessionSummary, Error> {
        let osc = self.osc().await?;
        osc.send("/live/song/set/tempo", vec![rosc::OscType::Float(bpm)])
            .await?;
        common::query_session_summary(osc).await
    }
}
