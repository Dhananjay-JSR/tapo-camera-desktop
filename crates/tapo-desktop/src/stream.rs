//! Live/playback streaming: TS → video frames (decoded) sent to the UI, audio sent to the speaker.

use crate::audio_out::Speaker;
use crate::decode::{new_decoder, YuvFrame};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tapo_core::relay::Mp2tParts;
use tapo_core::{RelaySocket, TsDemux, VideoCodec};

/// Pump a stream until it ends or `stop` is set. Decoded RGB frames go to `frames`; audio, if a
/// `Speaker` is given, is played. Returns an error string if the camera sends HEVC (unsupported).
/// The camera closes the stream unless the client keeps sending this keepalive.
const HEARTBEAT: Duration = Duration::from_secs(2);

pub fn run(
    mut parts: Mp2tParts<RelaySocket>,
    frames: Sender<YuvFrame>,
    stop: Arc<AtomicBool>,
    speaker: Option<Speaker>,
    audio_rate: u32,
) -> Result<(), String> {
    let mut demux = TsDemux::new();
    let mut dec = new_decoder();
    let mut last_hb = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let Ok(Some(ts)) = parts.next_payload() else {
            break;
        };
        let out = demux.feed(&ts);
        if demux.video_codec == Some(VideoCodec::Hevc) {
            return Err("This camera streams H.265, which this build can't decode.".into());
        }
        for au in out.frames {
            if let Some(d) = &mut dec {
                if let Some(frame) = d.decode(&au) {
                    if frames.send(frame).is_err() {
                        return Ok(()); // UI gone
                    }
                }
            }
        }
        if !out.audio.is_empty() {
            if let (Some(sp), Some(codec)) = (&speaker, demux.audio_codec) {
                sp.push_g711(codec, &out.audio, audio_rate);
            }
        }
        if last_hb.elapsed() >= HEARTBEAT {
            let _ = parts.heartbeat();
            last_hb = Instant::now();
        }
    }
    Ok(())
}
