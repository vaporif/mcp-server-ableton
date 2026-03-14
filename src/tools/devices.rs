use rosc::OscType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::Error;
use crate::osc::{extract_floats, extract_strings};
use crate::server::AbletonMcpServer;
use crate::tools::common::{self, SessionSummary};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeviceParams {
    /// Track index (0-based)
    pub track: i32,
    /// Device index (0-based)
    pub device: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetDeviceParameterParams {
    /// Track index (0-based)
    pub track: i32,
    /// Device index (0-based)
    pub device: i32,
    /// Parameter index (0-based)
    pub param: i32,
    /// Parameter value
    pub value: f32,
}

#[derive(Debug, Serialize)]
pub struct DeviceInfo {
    pub index: i32,
    pub name: String,
    pub class_name: String,
}

#[derive(Debug, Serialize)]
pub struct ParameterInfo {
    pub index: i32,
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Serialize)]
pub struct DeviceFull {
    pub track: i32,
    pub device: i32,
    pub name: String,
    pub class_name: String,
    pub parameters: Vec<ParameterInfo>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetDeviceParametersParams {
    /// Track index (0-based)
    pub track: i32,
    /// Device index (0-based)
    pub device: i32,
    /// Array of parameter index/value pairs to set
    pub parameters: Vec<ParameterValue>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ParameterValue {
    /// Parameter index (0-based)
    pub index: i32,
    /// Parameter value
    pub value: f32,
}

#[derive(Debug, Serialize)]
pub struct DeviceListResponse {
    pub track: i32,
    pub device_count: usize,
    pub devices: Vec<DeviceInfo>,
}

#[derive(Debug, Serialize)]
pub struct ParameterListResponse {
    pub track: i32,
    pub device: i32,
    pub parameter_count: usize,
    pub parameters: Vec<ParameterInfo>,
}

impl AbletonMcpServer {
    pub async fn do_list_devices(
        &self,
        track: i32,
    ) -> Result<(DeviceListResponse, SessionSummary), Error> {
        let osc = self.osc().await?;

        let names_msg = osc
            .query("/live/track/get/devices/name", vec![OscType::Int(track)])
            .await?;
        let class_msg = osc
            .query(
                "/live/track/get/devices/class_name",
                vec![OscType::Int(track)],
            )
            .await?;

        let names = extract_strings(&names_msg.args, 1);
        let class_names = extract_strings(&class_msg.args, 1);

        let devices: Vec<DeviceInfo> = names
            .into_iter()
            .zip(class_names)
            .enumerate()
            .map(|(i, (name, class_name))| DeviceInfo {
                index: i as i32,
                name,
                class_name,
            })
            .collect();

        let summary = common::query_session_summary(osc).await?;
        let response = DeviceListResponse {
            track,
            device_count: devices.len(),
            devices,
        };
        Ok((response, summary))
    }

    pub(crate) async fn query_device_parameters(
        &self,
        track: i32,
        device: i32,
    ) -> Result<Vec<ParameterInfo>, Error> {
        let osc = self.osc().await?;
        let args = vec![OscType::Int(track), OscType::Int(device)];

        let names_msg = osc
            .query("/live/device/get/parameters/name", args.clone())
            .await?;
        let values_msg = osc
            .query("/live/device/get/parameters/value", args.clone())
            .await?;
        let min_msg = osc
            .query("/live/device/get/parameters/min", args.clone())
            .await?;
        let max_msg = osc.query("/live/device/get/parameters/max", args).await?;

        let names = extract_strings(&names_msg.args, 2);
        let values = extract_floats(&values_msg.args, 2);
        let mins = extract_floats(&min_msg.args, 2);
        let maxs = extract_floats(&max_msg.args, 2);

        let params: Vec<ParameterInfo> = names
            .into_iter()
            .enumerate()
            .map(|(i, name)| ParameterInfo {
                index: i as i32,
                name,
                value: values.get(i).copied().unwrap_or(0.0),
                min: mins.get(i).copied().unwrap_or(0.0),
                max: maxs.get(i).copied().unwrap_or(1.0),
            })
            .collect();

        Ok(params)
    }

    pub async fn do_list_device_parameters(
        &self,
        track: i32,
        device: i32,
    ) -> Result<(ParameterListResponse, SessionSummary), Error> {
        let params = self.query_device_parameters(track, device).await?;
        let osc = self.osc().await?;
        let summary = common::query_session_summary(osc).await?;
        let response = ParameterListResponse {
            track,
            device,
            parameter_count: params.len(),
            parameters: params,
        };
        Ok((response, summary))
    }

    pub async fn do_set_device_parameter(
        &self,
        track: i32,
        device: i32,
        param: i32,
        value: f32,
    ) -> Result<(ParameterListResponse, SessionSummary), Error> {
        let osc = self.osc().await?;
        osc.send(
            "/live/device/set/parameter/value",
            vec![
                OscType::Int(track),
                OscType::Int(device),
                OscType::Int(param),
                OscType::Float(value),
            ],
        )
        .await?;

        let params = self.query_device_parameters(track, device).await?;
        let summary = common::query_session_summary(osc).await?;
        let response = ParameterListResponse {
            track,
            device,
            parameter_count: params.len(),
            parameters: params,
        };
        Ok((response, summary))
    }

    pub async fn do_get_device_full(
        &self,
        track: i32,
        device: i32,
    ) -> Result<(DeviceFull, SessionSummary), Error> {
        let osc = self.osc().await?;

        let names_msg = osc
            .query("/live/track/get/devices/name", vec![OscType::Int(track)])
            .await?;
        let class_msg = osc
            .query(
                "/live/track/get/devices/class_name",
                vec![OscType::Int(track)],
            )
            .await?;

        let names = extract_strings(&names_msg.args, 1);
        let class_names = extract_strings(&class_msg.args, 1);

        let device_idx = usize::try_from(device).map_err(|_| {
            Error::InvalidInput(format!("device index must be non-negative, got {device}"))
        })?;
        let name = names.get(device_idx).cloned().ok_or_else(|| {
            Error::InvalidInput(format!(
                "device index {device} out of range (0..{})",
                names.len()
            ))
        })?;
        let class_name = class_names.get(device_idx).cloned().unwrap_or_default();

        let parameters = self.query_device_parameters(track, device).await?;
        let summary = common::query_session_summary(osc).await?;

        let full = DeviceFull {
            track,
            device,
            name,
            class_name,
            parameters,
        };
        Ok((full, summary))
    }

    pub async fn do_set_device_parameters(
        &self,
        track: i32,
        device: i32,
        parameters: &[ParameterValue],
    ) -> Result<(ParameterListResponse, SessionSummary), Error> {
        let osc = self.osc().await?;

        for pv in parameters {
            osc.send(
                "/live/device/set/parameter/value",
                vec![
                    OscType::Int(track),
                    OscType::Int(device),
                    OscType::Int(pv.index),
                    OscType::Float(pv.value),
                ],
            )
            .await?;
        }

        let params = self.query_device_parameters(track, device).await?;
        let summary = common::query_session_summary(osc).await?;
        let response = ParameterListResponse {
            track,
            device,
            parameter_count: params.len(),
            parameters: params,
        };
        Ok((response, summary))
    }
}
