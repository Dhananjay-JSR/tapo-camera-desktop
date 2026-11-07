//! Camera list and commands, tunnelled through the cloud to each camera's own JSON API.

use crate::client::code_str;
use crate::client::Cloud;
use crate::error::{Error, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize)]
pub struct Camera {
    pub id: String,
    pub name: String,
    pub model: String,
    pub online: bool,
    /// Pan/tilt capable (C200/C210 yes, C310 no). Filled from the feature list on demand.
    pub ptz: bool,
}

impl Cloud {
    /// List the account's Tapo cameras.
    pub fn cameras(&self) -> Result<Vec<Camera>> {
        let url = format!(
            "{}/v2/things?page=0&pageSize=100&deviceTypes=SMART.IPCAMERA",
            self.app_server()?
        );
        let mut req = self
            .http()
            .get(url)
            .header("Authorization", format!("ut|{}", self.token()?));
        for (k, v) in self.app_headers() {
            req = req.header(k, v);
        }
        let r: Value = req.send()?.json()?;
        let mut out = Vec::new();
        for d in r["data"].as_array().cloned().unwrap_or_default() {
            let name = d["nickname"]
                .as_str()
                .and_then(|n| STANDARD.decode(n).ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            out.push(Camera {
                id: d["thingName"].as_str().unwrap_or_default().to_string(),
                name,
                model: d["deviceModel"]
                    .as_str()
                    .or(d["model"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                online: d["status"].as_i64() == Some(1),
                ptz: false,
            });
        }
        Ok(out)
    }

    /// Send a request to the camera's own API via the cloud.
    pub fn device_raw(&self, device_id: &str, request_data: Value) -> Result<Value> {
        let body =
            json!({"inputParams": {"requestData": request_data}, "serviceId": "passthrough"});
        let url = format!(
            "{}/v1/things/{}/services-sync",
            self.app_server()?,
            device_id
        );
        let mut req = self
            .http()
            .post(url)
            .header("Authorization", format!("ut|{}", self.token()?))
            .header("Content-Type", "application/json; charset=UTF-8");
        for (k, v) in self.app_headers() {
            req = req.header(k, v);
        }
        let r: Value = req.json(&body).send()?.json()?;
        let res = &r["outputParams"]["responseData"];
        if res.get("error_code").map(code_str).as_deref() != Some("0") {
            return Err(Error::Camera(res["error_code"].as_i64().unwrap_or(-1)));
        }
        Ok(res.clone())
    }

    /// Run one named camera command through `multipleRequest`.
    pub fn passthrough(&self, device_id: &str, method: &str, params: Value) -> Result<Value> {
        let res = self.device_raw(
            device_id,
            json!({
                "method": "multipleRequest",
                "params": {"requests": [{"method": method, "params": params}]}
            }),
        )?;
        let one = &res["result"]["responses"][0];
        if one.get("error_code").map(code_str).as_deref() != Some("0") {
            return Err(Error::Camera(one["error_code"].as_i64().unwrap_or(-1)));
        }
        Ok(one.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Relative pan/tilt step. x: left −/right +, y: down −/up +.
    pub fn move_motor(&self, device_id: &str, x: i32, y: i32) -> Result<()> {
        self.passthrough(
            device_id,
            "motorMove",
            json!({"motor": {"move": {"x_coord": x.to_string(), "y_coord": y.to_string()}}}),
        )?;
        Ok(())
    }

    pub fn set_led(&self, device_id: &str, on: bool) -> Result<()> {
        self.passthrough(
            device_id,
            "setLedStatus",
            json!({"led": {"config": {"enabled": if on {"on"} else {"off"}}}}),
        )?;
        Ok(())
    }

    /// Privacy mode (lens cover). `on` blanks the camera.
    pub fn set_privacy(&self, device_id: &str, on: bool) -> Result<()> {
        self.passthrough(
            device_id,
            "setLensMaskConfig",
            json!({"lens_mask": {"lens_mask_info": {"enabled": if on {"on"} else {"off"}}}}),
        )?;
        Ok(())
    }

    /// Arm/disarm the on-motion siren + light alarm.
    pub fn set_alarm(&self, device_id: &str, on: bool) -> Result<()> {
        self.passthrough(
            device_id,
            "setAlertConfig",
            json!({"msg_alarm": {"chn1_msg_alarm_info": {"enabled": if on {"on"} else {"off"}}}}),
        )?;
        Ok(())
    }

    pub fn goto_preset(&self, device_id: &str, preset_id: &str) -> Result<()> {
        self.passthrough(
            device_id,
            "motorMoveToPreset",
            json!({"preset": {"goto_preset": {"id": preset_id}}}),
        )?;
        Ok(())
    }

    /// Preset ids paired with their names.
    pub fn presets(&self, device_id: &str) -> Result<Vec<(String, String)>> {
        let r = self.passthrough(
            device_id,
            "getPresetConfig",
            json!({"preset": {"name": ["preset"]}}),
        )?;
        let p = &r["preset"]["preset"];
        let ids = p["id"].as_array().cloned().unwrap_or_default();
        let names = p["name"].as_array().cloned().unwrap_or_default();
        Ok(ids
            .iter()
            .zip(names.iter())
            .map(|(i, n)| {
                (
                    i.as_str().unwrap_or_default().to_string(),
                    n.as_str().unwrap_or_default().to_string(),
                )
            })
            .collect())
    }

    /// Microphone sample rate in Hz (C210 = 8000, C200/C310 = 16000); cameras ignore the relay's request.
    pub fn mic_rate(&self, device_id: &str) -> Result<u32> {
        let r = self.passthrough(
            device_id,
            "getAudioConfig",
            json!({"audio_config": {"name": ["microphone"]}}),
        )?;
        let khz = r["audio_config"]["microphone"]["sampling_rate"]
            .as_str()
            .unwrap_or("16");
        Ok(khz.parse::<u32>().unwrap_or(16) * 1000)
    }
}
