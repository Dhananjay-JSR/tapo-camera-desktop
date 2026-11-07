//! Cloud relay: request a session, open the stream, parse the camera's multipart MPEG-TS.

use crate::client::{code_str, Cloud, APP_VER};
use crate::error::{Error, Result};
use crate::relay_socket::RelaySocket;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read};
use uuid::Uuid;

/// Relay purpose (the `streamType` value).
#[derive(Clone, Copy, PartialEq)]
pub enum StreamType {
    Live = 0,
    Playback = 1,
    Talk = 2,
}

pub struct RelayInfo {
    pub business_url: String,
    pub relay_url: String,
    pub cdn_url: String,
    pub token: String,
}

impl Cloud {
    /// Ask the cloud for a relay session for `device_id`.
    pub fn relay_request(
        &self,
        device_id: &str,
        stream: StreamType,
        custom: Value,
        channel: bool,
    ) -> Result<RelayInfo> {
        let mut params = json!({
            "cloudType": 2, "customParams": custom.to_string(), "dataTimeoutCount": 0,
            "deviceId": device_id, "deviceType": "SMART.IPCAMERA", "playerId": self.session.term_id,
            "preConnection": 0, "resolution": "HD", "rootCaVer": "1",
            "streamType": stream as i32, "trackId": Uuid::new_v4().to_string(),
        });
        if channel {
            params["channelId"] = json!("0");
        }
        let body = serde_json::to_vec(&params)?;
        // X-Request-Signature is just a random 10-char request id, not a signature.
        let sig: String = Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(10)
            .collect();
        let url = format!("{}/v2/relay/request", self.cipc()?);
        let r: Value = self
            .http()
            .post(url)
            .header("X-Request-Signature", sig)
            .header("Authorization", self.token()?)
            .header("X-Client-Id", &self.session.term_id)
            .header("X-Source", "tapo-app")
            .header("X-Brand", "TPLINK")
            .header("X-Ca-Type", "cloud-self")
            .header("Content-Type", "application/json; charset=UTF-8")
            .header(
                "User-Agent",
                format!("Tapo_APP/pc/{APP_VER}_Android/Android 14"),
            )
            .body(body)
            .send()?
            .json()?;
        if code_str(&r["errorCode"]) != "0" {
            return Err(Error::Cloud(format!("relay/request {}", r["errorCode"])));
        }
        let res = &r["result"];
        Ok(RelayInfo {
            business_url: res["relayBusinessUrl"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            relay_url: res["relayUrl"].as_str().unwrap_or_default().to_string(),
            cdn_url: res["cdnUrl"].as_str().unwrap_or_default().to_string(),
            token: res["relayToken"].as_str().unwrap_or_default().to_string(),
        })
    }

    /// Open a live stream and return a reader over its multipart MPEG-TS body.
    /// The direct relay host works for live; if it refuses, fall back to the CDN host.
    pub fn open_stream(&self, device_id: &str, custom: Value) -> Result<Mp2tParts<RelaySocket>> {
        let relay = self.relay_request(device_id, StreamType::Live, custom, false)?;
        let (bhost, bpq) = split_url(&relay.business_url);
        let (chost, _) = split_url(&relay.cdn_url);
        let candidates: [(&str, &str, Option<&str>); 3] = [
            (bhost, bpq, None),
            (chost, bpq, Some(bhost)),
            (bhost, bpq, None),
        ];
        let mut last = Error::Cloud("relay refused the stream".into());
        for (host, pq, origin) in candidates {
            match RelaySocket::open(host, pq, &relay.token, origin, &self.session.term_id) {
                Ok(sock) => return Ok(Mp2tParts::new(BufReader::new(sock))),
                Err(e) => last = e,
            }
        }
        Err(last)
    }
}

/// Streams the `----device-stream-boundary--` parts, yielding only `video/mp2t` payloads.
pub struct Mp2tParts<R: Read> {
    reader: BufReader<R>,
}

impl<R: Read> Mp2tParts<R> {
    pub(crate) fn new(reader: BufReader<R>) -> Self {
        Mp2tParts { reader }
    }

    /// Next MPEG-TS payload, or `Ok(None)` at end of stream. Non-video parts (e.g. JSON) are skipped.
    pub fn next_payload(&mut self) -> Result<Option<Vec<u8>>> {
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line)? == 0 {
                return Ok(None);
            }
            if !line.starts_with("----device-stream-boundary--") {
                continue;
            }
            let mut content_type = String::new();
            let mut len = 0usize;
            let mut encrypted = false;
            loop {
                let mut h = String::new();
                if self.reader.read_line(&mut h)? == 0 {
                    return Ok(None);
                }
                let h = h.trim_end();
                if h.is_empty() {
                    break;
                }
                let (k, v) = h
                    .split_once(':')
                    .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
                    .unwrap_or_default();
                match k.as_str() {
                    "content-type" => content_type = v,
                    "content-length" => len = v.parse().unwrap_or(0),
                    "x-if-encrypt" => encrypted = v != "0",
                    _ => {}
                }
            }
            let mut body = vec![0u8; len];
            self.reader.read_exact(&mut body)?;
            if encrypted {
                // shortcut: encrypted streams unsupported (cameras send X-If-Encrypt: 0 here); add decryption if seen.
                return Err(Error::Cloud("camera sent an encrypted stream".into()));
            }
            if content_type == "video/mp2t" {
                return Ok(Some(body));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_two_mp2t_parts() {
        let part = b"----device-stream-boundary--\r\nContent-Type: video/mp2t\r\nContent-Length: 3\r\nX-If-Encrypt: 0\r\n\r\nG\x00\x01";
        let mut data = Vec::new();
        data.extend_from_slice(part);
        data.extend_from_slice(b"\r\n");
        data.extend_from_slice(part);
        let mut p = Mp2tParts::new(BufReader::new(Cursor::new(data)));
        assert_eq!(p.next_payload().unwrap(), Some(vec![b'G', 0, 1]));
        assert_eq!(p.next_payload().unwrap(), Some(vec![b'G', 0, 1]));
        assert_eq!(p.next_payload().unwrap(), None);
    }

    #[test]
    fn url_helpers() {
        assert_eq!(
            split_url("https://host.example/relayservice?a=1"),
            ("host.example", "/relayservice?a=1")
        );
        assert_eq!(split_url("https://h/x"), ("h", "/x"));
    }
}

/// Split an `https://host/path?query` URL into `(host, "/path?query")`.
pub(crate) fn split_url(url: &str) -> (&str, &str) {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    }
}
