use rosc::OscType;
use serde::Serialize;

use crate::errors::Error;
use crate::osc::extract_strings;
use crate::server::AbletonMcpServer;
use crate::tools::scenes::SceneInfo;

#[derive(Debug, Serialize)]
pub struct SessionState {
    pub tempo: f64,
    pub is_playing: bool,
    pub selected_track: i32,
    pub tracks: Vec<SessionTrackInfo>,
    pub scenes: Vec<SceneInfo>,
}

#[derive(Debug, Serialize)]
pub struct SessionTrackInfo {
    pub index: i32,
    pub name: String,
    pub volume: f32,
    pub mute: bool,
    pub devices: Vec<String>,
}

impl AbletonMcpServer {
    pub async fn do_get_session_state(&self) -> Result<SessionState, Error> {
        let osc = self.osc().await?;

        let tempo: f64 = osc.query_val("/live/song/get/tempo", vec![]).await?;
        let is_playing: bool = osc.query_val("/live/song/get/is_playing", vec![]).await?;
        let selected_track: i32 = osc
            .query_val("/live/view/get/selected_track", vec![])
            .await?;

        let num_tracks: i32 = osc.query_val("/live/song/get/num_tracks", vec![]).await?;
        let num_tracks = num_tracks.max(0);

        let mut tracks = Vec::with_capacity(num_tracks as usize);
        for i in 0..num_tracks {
            let name: String = osc
                .query_val("/live/track/get/name", vec![OscType::Int(i)])
                .await?;
            let volume: f32 = osc
                .query_val("/live/track/get/volume", vec![OscType::Int(i)])
                .await?;
            let mute: bool = osc
                .query_val("/live/track/get/mute", vec![OscType::Int(i)])
                .await?;

            let dev_names_msg = osc
                .query("/live/track/get/devices/name", vec![OscType::Int(i)])
                .await?;
            let device_names = extract_strings(&dev_names_msg.args, 1);

            tracks.push(SessionTrackInfo {
                index: i,
                name,
                volume,
                mute,
                devices: device_names,
            });
        }

        let num_scenes: i32 = osc.query_val("/live/song/get/num_scenes", vec![]).await?;
        let num_scenes = num_scenes.max(0);
        let mut scenes = Vec::with_capacity(num_scenes as usize);
        for i in 0..num_scenes {
            let name: String = osc
                .query_val("/live/scene/get/name", vec![OscType::Int(i)])
                .await?;
            scenes.push(SceneInfo { index: i, name });
        }

        Ok(SessionState {
            tempo,
            is_playing,
            selected_track,
            tracks,
            scenes,
        })
    }
}
