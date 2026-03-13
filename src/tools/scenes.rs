use rosc::OscType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::Error;
use crate::server::AbletonMcpServer;
use crate::tools::common::{self, SessionSummary};
use crate::tools::transport::TransportState;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SceneIndexParams {
    /// Scene index (0-based)
    pub scene: i32,
}

#[derive(Debug, Serialize)]
pub struct SceneInfo {
    pub index: i32,
    pub name: String,
}

impl AbletonMcpServer {
    pub async fn do_list_scenes(&self) -> Result<(Vec<SceneInfo>, SessionSummary), Error> {
        let osc = self.osc().await?;
        let num_scenes: i32 = osc.query_val("/live/song/get/num_scenes", vec![]).await?;
        let num_scenes = num_scenes.max(0);
        let mut scenes = Vec::with_capacity(num_scenes as usize);
        for i in 0..num_scenes {
            let name: String = osc
                .query_val("/live/scene/get/name", vec![OscType::Int(i)])
                .await?;
            scenes.push(SceneInfo { index: i, name });
        }
        let summary = common::query_session_summary(osc).await?;
        Ok((scenes, summary))
    }

    pub async fn do_fire_scene(
        &self,
        scene: i32,
    ) -> Result<(TransportState, SessionSummary), Error> {
        let osc = self.osc().await?;
        osc.send("/live/scene/fire", vec![OscType::Int(scene)])
            .await?;
        let summary = common::query_session_summary(osc).await?;
        let state = TransportState {
            is_playing: summary.is_playing,
            tempo: summary.tempo,
        };
        Ok((state, summary))
    }
}
