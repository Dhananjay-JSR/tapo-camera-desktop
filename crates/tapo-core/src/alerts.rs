//! Camera event history (motion, person, sound, tamper…) from the TP-Link notification feed.

use crate::client::{Cloud, APP_NAME};
use crate::error::Result;
use serde::Serialize;
use serde_json::json;

/// Event types to subscribe to; also the filter for the feed query.
pub const ALERT_TYPES: &[&str] = &[
    "Motion",
    "PersonDetected",
    "PersonEnhanced",
    "Audio",
    "BabyCry",
    "tapoCameraAreaIntrusionDetection",
    "tapoCameraLinecrossingDetection",
    "tapoCameraCameraTampering",
    "tapoCameraSDNeedInitialization",
    "tapoCameraSDInsufficientStorage",
    "tapoNewFirmware",
];

#[derive(Clone, Debug, Serialize)]
pub struct Alert {
    pub id: String,
    pub device: String,
    pub kind: String,
    pub text: String,
    pub time_ms: i64,
}

impl Cloud {
    /// Camera events after `since_ms` (oldest first). `index` pages through large results.
    pub fn alerts(&self, since_ms: i64, index: i64) -> Result<Vec<Alert>> {
        let r = self.wap_v2(
            "/api/v2/common/getAppNotificationByPage",
            json!({
                "appType": APP_NAME, "contentVersion": 3,
                "deviceToken": format!("pc-{}", self.session.term_id), // any value works; not used for delivery
                "terminalUUID": self.session.term_id, "direction": "asc", "index": index,
                "indexTime": since_ms, "limit": 50, "locale": "en_US", "mobileType": "ANDROID",
                "msgTypes": ALERT_TYPES,
            }),
        )?;
        let mut out = Vec::new();
        for n in r["notifications"].as_array().cloned().unwrap_or_default() {
            let a = &n["attachments"];
            out.push(Alert {
                id: n["msgId"].as_str().unwrap_or_default().to_string(),
                device: a["alias"].as_str().unwrap_or("Tapo").to_string(),
                kind: n["msgType"].as_str().unwrap_or_default().to_string(),
                text: n["content"]
                    .as_str()
                    .or(n["title"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                time_ms: n["time"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
            });
        }
        Ok(out)
    }
}
