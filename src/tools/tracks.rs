use rosc::OscType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::Error;
use crate::osc::extract_strings;
use crate::server::AbletonMcpServer;
use crate::tools::common::{self, SessionSummary};
use crate::tools::devices::DeviceInfo;

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

#[derive(Debug, Serialize)]
pub struct TrackDetail {
    pub index: i32,
    pub name: String,
    pub volume: f32,
    pub panning: f32,
    pub mute: bool,
    pub solo: bool,
    pub devices: Vec<DeviceInfo>,
    pub clips: Vec<ClipSlotInfo>,
}

#[derive(Debug, Serialize)]
pub struct ClipSlotInfo {
    pub slot: i32,
    pub has_clip: bool,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetMixerParams {
    /// Track index (0-based)
    pub track: i32,
    /// Volume 0.0-1.0. Omit to leave unchanged.
    pub volume: Option<f32>,
    /// Pan -1.0 to 1.0. Omit to leave unchanged.
    pub pan: Option<f32>,
    /// Mute state. Omit to leave unchanged.
    pub mute: Option<bool>,
    /// Solo state. Omit to leave unchanged.
    pub solo: Option<bool>,
}

impl AbletonMcpServer {
    pub(crate) async fn query_mixer_state(&self, track: i32) -> Result<MixerState, Error> {
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

    pub async fn do_get_track_detail(
        &self,
        track: i32,
    ) -> Result<(TrackDetail, SessionSummary), Error> {
        let osc = self.osc().await?;

        let mixer = self.query_mixer_state(track).await?;

        let name: String = osc
            .query_val("/live/track/get/name", vec![OscType::Int(track)])
            .await?;

        // Get devices
        let names_msg = osc
            .query(
                "/live/track/get/devices/name",
                vec![OscType::Int(track)],
            )
            .await?;
        let class_msg = osc
            .query(
                "/live/track/get/devices/class_name",
                vec![OscType::Int(track)],
            )
            .await?;
        let dev_names = extract_strings(&names_msg.args, 1);
        let class_names = extract_strings(&class_msg.args, 1);
        let devices: Vec<DeviceInfo> = dev_names
            .into_iter()
            .zip(class_names)
            .enumerate()
            .map(|(i, (n, c))| DeviceInfo {
                index: i as i32,
                name: n,
                class_name: c,
            })
            .collect();

        // Get clip slots
        let num_scenes: i32 = osc
            .query_val("/live/song/get/num_scenes", vec![])
            .await?;
        let mut clips = Vec::with_capacity(num_scenes as usize);
        for slot in 0..num_scenes {
            let has_clip: bool = osc
                .query_val(
                    "/live/clip_slot/get/has_clip",
                    vec![OscType::Int(track), OscType::Int(slot)],
                )
                .await?;
            let clip_name = if has_clip {
                let n: String = osc
                    .query_val(
                        "/live/clip/get/name",
                        vec![OscType::Int(track), OscType::Int(slot)],
                    )
                    .await?;
                Some(n)
            } else {
                None
            };
            clips.push(ClipSlotInfo {
                slot,
                has_clip,
                name: clip_name,
            });
        }

        let summary = common::query_session_summary(osc).await?;
        let detail = TrackDetail {
            index: track,
            name,
            volume: mixer.volume,
            panning: mixer.panning,
            mute: mixer.mute,
            solo: mixer.solo,
            devices,
            clips,
        };
        Ok((detail, summary))
    }

    pub async fn do_set_mixer(
        &self,
        params: &SetMixerParams,
    ) -> Result<(MixerState, SessionSummary), Error> {
        let osc = self.osc().await?;

        if let Some(volume) = params.volume {
            osc.send(
                "/live/track/set/volume",
                vec![OscType::Int(params.track), OscType::Float(volume)],
            )
            .await?;
        }
        if let Some(pan) = params.pan {
            osc.send(
                "/live/track/set/panning",
                vec![OscType::Int(params.track), OscType::Float(pan)],
            )
            .await?;
        }
        if let Some(mute) = params.mute {
            osc.send(
                "/live/track/set/mute",
                vec![
                    OscType::Int(params.track),
                    OscType::Int(i32::from(mute)),
                ],
            )
            .await?;
        }
        if let Some(solo) = params.solo {
            osc.send(
                "/live/track/set/solo",
                vec![
                    OscType::Int(params.track),
                    OscType::Int(i32::from(solo)),
                ],
            )
            .await?;
        }

        let mixer = self.query_mixer_state(params.track).await?;
        let summary = common::query_session_summary(osc).await?;
        Ok((mixer, summary))
    }
}
