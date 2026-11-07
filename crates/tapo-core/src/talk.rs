//! Talk-back: push microphone audio (G.711 µ-law 16 kHz) to the camera speaker.
//!
//! Half-duplex: open a `streamType=2` relay, send a `talk` start request, read the camera's
//! `session_id`, then stream audio parts tagged with it. Dropping the session stops talking.

use crate::client::Cloud;
use crate::error::{Error, Result};
use crate::relay::{split_url, StreamType};
use crate::relay_socket::RelaySocket;
use crate::ts_mux::TsMux;
use serde_json::{json, Value};

pub struct TalkSession {
    sock: RelaySocket,
    mux: TsMux,
    session_id: String,
}

impl Cloud {
    /// Begin a talk-back session. Fails with the camera's error if the line is busy.
    pub fn start_talk(&self, device_id: &str) -> Result<TalkSession> {
        let relay = self.relay_request(
            device_id,
            StreamType::Talk,
            json!({"mode": "half_duplex"}),
            true,
        )?;
        let (chost, _) = split_url(&relay.cdn_url);
        let (bhost, bpq) = split_url(&relay.business_url);
        let mut sock = RelaySocket::open(chost, bpq, &relay.token, Some(bhost))?;

        let req = json!({
            "type": "request", "seq": 1, "method": "get",
            "params": {"talk": {"channel": 0, "deviceId": device_id, "mode": "half_duplex", "playerId": self.session.term_id}}
        });
        sock.write_part(
            &[("Content-Type", "application/json".into())],
            serde_json::to_vec(&req)?.as_slice(),
        )?;

        let reply: Value = serde_json::from_str(&sock.read_json_part()?)?;
        let code = reply["params"]["error_code"].as_i64().unwrap_or(-1);
        if code != 0 {
            return Err(Error::Camera(code));
        }
        let session_id = reply["params"]["session_id"]
            .as_str()
            .unwrap_or("0")
            .to_string();
        Ok(TalkSession {
            sock,
            mux: TsMux::new(),
            session_id,
        })
    }
}

impl TalkSession {
    /// Send one chunk of G.711 µ-law 16 kHz audio (≈1024 bytes = 64 ms is one part).
    pub fn send(&mut self, pcm: &[u8]) -> Result<()> {
        let ts = self.mux.frame(pcm);
        self.sock.write_part(
            &[
                ("X-Session-Id", self.session_id.clone()),
                ("Content-Type", "audio/mp2t".into()),
            ],
            &ts,
        )
    }
}
