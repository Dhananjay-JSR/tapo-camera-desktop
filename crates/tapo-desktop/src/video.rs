//! Play a camera MPEG-TS stream by piping it to `ffplay` (video) and its demuxed G.711 to a
//! second `ffplay` (audio). Decoding is delegated to ffmpeg rather than bundled.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tapo_core::relay::Mp2tParts;
use tapo_core::TsDemux;

fn ffplay(args: &[&str]) -> std::io::Result<Child> {
    Command::new("ffplay")
        .args(["-hide_banner", "-loglevel", "error", "-fflags", "nobuffer"])
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
}

/// Pump a stream to ffplay until it ends or `stop` is set. `title` names the video window.
/// `audio_rate` is the camera mic rate (Hz); audio starts once the first G.711 bytes arrive.
pub fn play<R: Read>(mut parts: Mp2tParts<R>, title: &str, audio_rate: u32, stop: Arc<AtomicBool>) {
    // Small probe: ffmpeg can't identify Tapo's G.711 track and would otherwise buffer minutes first.
    let mut video = match ffplay(&[
        "-probesize",
        "100000",
        "-analyzeduration",
        "500000",
        "-an",
        "-window_title",
        title,
        "-i",
        "-",
    ]) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ffplay not found ({e}); install ffmpeg");
            return;
        }
    };
    let mut vin = video.stdin.take().unwrap();
    let mut demux = TsDemux::new();
    let mut audio: Option<Child> = None;
    let rate = audio_rate.to_string();

    while !stop.load(Ordering::Relaxed) {
        match parts.next_payload() {
            Ok(Some(chunk)) => {
                if vin.write_all(&chunk).is_err() {
                    break; // window closed
                }
                let pcm = demux.feed(&chunk).audio;
                if !pcm.is_empty() {
                    if audio.is_none() {
                        if let Some(codec) = demux.audio_codec {
                            audio = ffplay(&[
                                "-nodisp",
                                "-f",
                                codec.ffmpeg(),
                                "-sample_rate",
                                &rate,
                                "-ch_layout",
                                "mono",
                                "-i",
                                "-",
                            ])
                            .ok();
                        }
                    }
                    if let Some(a) = &mut audio {
                        if let Some(ain) = a.stdin.as_mut() {
                            let _ = ain.write_all(&pcm);
                        }
                    }
                }
            }
            _ => break,
        }
    }
    let _ = video.kill();
    if let Some(mut a) = audio {
        let _ = a.kill();
    }
}
