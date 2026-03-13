use rosc::OscType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::Error;
use crate::server::AbletonMcpServer;
use crate::tools::common::{self, SessionSummary};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TrackIndexParams {
    /// Track index (0-based)
    pub track: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTrackVolumeParams {
    /// Track index (0-based)
    pub track: i32,
    /// Volume 0.0 to 1.0
    pub volume: f32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTrackNameParams {
    /// Track index (0-based)
    pub track: i32,
    /// New track name
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TrackInfo {
    pub index: i32,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct MixerState {
    pub track: i32,
    pub volume: f32,
    pub panning: f32,
    pub mute: bool,
    pub solo: bool,
}

impl AbletonMcpServer {
    async fn query_mixer_state(&self, track: i32) -> Result<MixerState, Error> {
        let osc = self.osc().await?;
        let volume: f32 = osc
            .query_val("/live/track/get/volume", vec![OscType::Int(track)])
            .await?;
        let panning: f32 = osc
            .query_val("/live/track/get/panning", vec![OscType::Int(track)])
            .await?;
        let mute: bool = osc
            .query_val("/live/track/get/mute", vec![OscType::Int(track)])
            .await?;
        let solo: bool = osc
            .query_val("/live/track/get/solo", vec![OscType::Int(track)])
            .await?;
        Ok(MixerState {
            track,
            volume,
            panning,
            mute,
            solo,
        })
    }

    pub async fn do_list_tracks(
        &self,
    ) -> Result<(Vec<TrackInfo>, SessionSummary), Error> {
        let osc = self.osc().await?;
        let num_tracks: i32 = osc
            .query_val("/live/song/get/num_tracks", vec![])
            .await?;
        let mut tracks = Vec::with_capacity(num_tracks as usize);
        for i in 0..num_tracks {
            let name: String = osc
                .query_val("/live/track/get/name", vec![OscType::Int(i)])
                .await?;
            tracks.push(TrackInfo { index: i, name });
        }
        let summary = common::query_session_summary(osc).await?;
        Ok((tracks, summary))
    }

    pub async fn do_set_track_volume(
        &self,
        track: i32,
        volume: f32,
    ) -> Result<(MixerState, SessionSummary), Error> {
        let osc = self.osc().await?;
        osc.send(
            "/live/track/set/volume",
            vec![OscType::Int(track), OscType::Float(volume)],
        )
        .await?;
        let mixer = self.query_mixer_state(track).await?;
        let summary = common::query_session_summary(osc).await?;
        Ok((mixer, summary))
    }

    pub async fn do_set_track_name(
        &self,
        track: i32,
        name: &str,
    ) -> Result<(TrackInfo, SessionSummary), Error> {
        let osc = self.osc().await?;
        osc.send(
            "/live/track/set/name",
            vec![OscType::Int(track), OscType::String(name.to_string())],
        )
        .await?;
        let updated_name: String = osc
            .query_val("/live/track/get/name", vec![OscType::Int(track)])
            .await?;
        let summary = common::query_session_summary(osc).await?;
        Ok((TrackInfo { index: track, name: updated_name }, summary))
    }

    pub async fn do_mute_track(
        &self,
        track: i32,
    ) -> Result<(MixerState, SessionSummary), Error> {
        let osc = self.osc().await?;
        osc.send(
            "/live/track/set/mute",
            vec![OscType::Int(track), OscType::Int(1)],
        )
        .await?;
        let mixer = self.query_mixer_state(track).await?;
        let summary = common::query_session_summary(osc).await?;
        Ok((mixer, summary))
    }

    pub async fn do_unmute_track(
        &self,
        track: i32,
    ) -> Result<(MixerState, SessionSummary), Error> {
        let osc = self.osc().await?;
        osc.send(
            "/live/track/set/mute",
            vec![OscType::Int(track), OscType::Int(0)],
        )
        .await?;
        let mixer = self.query_mixer_state(track).await?;
        let summary = common::query_session_summary(osc).await?;
        Ok((mixer, summary))
    }
}
