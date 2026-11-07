//! SD-card recordings: list days, list clips, and stream a clip over the relay.

use crate::client::Cloud;
use crate::error::Result;
use crate::relay::{split_url, Mp2tParts, StreamType};
use crate::relay_socket::RelaySocket;
use serde::Serialize;
use serde_json::json;
use std::io::BufReader;

#[derive(Clone, Debug, Serialize)]
pub struct Clip {
    pub start: i64,
    pub end: i64,
}

impl Cloud {
    /// The camera-assigned user id, required by playback search.
    pub fn user_id(&self, device_id: &str) -> Result<i64> {
        let r = self.passthrough(
            device_id,
            "getUserID",
            json!({"system": {"get_user_id": "null"}}),
        )?;
        Ok(r["user_id"].as_i64().unwrap_or(1))
    }

    /// Dates (YYYYMMDD) in `[start_date, end_date]` that have recordings.
    pub fn recording_days(
        &self,
        device_id: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<String>> {
        let r = self.passthrough(device_id, "searchDateWithVideo", json!({
            "playback": {"search_year_utility": {"channel": [0], "start_date": start_date, "end_date": end_date}}
        }))?;
        Ok(r["playback"]["search_results"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|o| o.as_object().and_then(|m| m.values().next()))
            .filter_map(|v| v["date"].as_str().map(str::to_string))
            .collect())
    }

    /// Clips on one day (YYYYMMDD).
    pub fn clips(&self, device_id: &str, date: &str) -> Result<Vec<Clip>> {
        let uid = self.user_id(device_id)?;
        let r = self.passthrough(device_id, "searchVideoOfDay", json!({
            "playback": {"search_video_utility": {"channel": 0, "date": date, "start_index": 0, "end_index": 999, "id": uid}}
        }))?;
        Ok(r["playback"]["search_video_results"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|o| o.as_object().and_then(|m| m.values().next()).cloned())
            .map(|c| Clip {
                start: c["startTime"].as_i64().unwrap_or(0),
                end: c["endTime"].as_i64().unwrap_or(0),
            })
            .collect())
    }

    /// Open a recorded clip and return a reader over its MPEG-TS parts.
    pub fn open_playback(&self, device_id: &str, clip: &Clip) -> Result<Mp2tParts<RelaySocket>> {
        let uid = self.user_id(device_id)?;
        let relay = self.relay_request(
            device_id,
            StreamType::Playback,
            json!({"audio_config": {"encode_type": "G711ulaw", "sample_rate": "16"}}),
            true,
        )?;
        let (chost, _) = split_url(&relay.cdn_url);
        let (bhost, bpq) = split_url(&relay.business_url);
        let mut sock = RelaySocket::open(chost, bpq, &relay.token, Some(bhost))?;
        let req = json!({
            "type": "request", "seq": 1, "method": "get",
            "params": {"playback": {"client_id": uid, "channels": [0, 1], "scale": "1/1",
                "start_time": clip.start.to_string(), "end_time": clip.end.to_string(), "event_type": [1, 2]}}
        });
        sock.write_part(
            &[("Content-Type", "application/json".into())],
            serde_json::to_vec(&req)?.as_slice(),
        )?;
        Ok(Mp2tParts::new(BufReader::new(sock)))
    }
}
