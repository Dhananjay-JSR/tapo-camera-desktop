//! Play the camera's G.711 audio through the default output device (no ffmpeg).
//! Decodes µ-law/A-law to PCM, linearly resamples mono to the device rate, and feeds cpal.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tapo_core::AudioCodec;

struct Inner {
    buf: VecDeque<f32>,
    prev: f32,
    t: f32, // fractional position between prev and the next input sample (input-sample units)
}

pub struct Speaker {
    _stream: cpal::Stream,
    inner: Arc<Mutex<Inner>>,
    out_rate: u32,
}

impl Speaker {
    pub fn open() -> Option<Self> {
        let device = cpal::default_host().default_output_device()?;
        let config = device.default_output_config().ok()?;
        let out_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let inner = Arc::new(Mutex::new(Inner {
            buf: VecDeque::new(),
            prev: 0.0,
            t: 0.0,
        }));
        let cb = inner.clone();
        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &_| {
                    let mut g = cb.lock().unwrap();
                    for frame in data.chunks_mut(channels) {
                        let s = g.buf.pop_front().unwrap_or(0.0); // silence on underrun
                        frame.fill(s);
                    }
                },
                |e| eprintln!("audio out error: {e}"),
                None,
            )
            .ok()?;
        stream.play().ok()?;
        Some(Speaker {
            _stream: stream,
            inner,
            out_rate,
        })
    }

    /// Decode + resample one chunk of G.711 and queue it for playback.
    pub fn push_g711(&self, codec: AudioCodec, bytes: &[u8], src_rate: u32) {
        let step = src_rate as f32 / self.out_rate as f32; // input advance per output sample
        let mut g = self.inner.lock().unwrap();
        for &b in bytes {
            let cur = decode(codec, b) as f32 / 32768.0;
            while g.t < 1.0 {
                let t = g.t;
                let v = g.prev * (1.0 - t) + cur * t;
                g.buf.push_back(v);
                g.t += step;
            }
            g.t -= 1.0;
            g.prev = cur;
        }
        // Cap latency if the UI thread stalls (keep ~1s).
        let cap = self.out_rate as usize;
        while g.buf.len() > cap {
            g.buf.pop_front();
        }
    }
}

fn decode(codec: AudioCodec, v: u8) -> i16 {
    match codec {
        AudioCodec::Mulaw => ulaw_to_linear(v),
        AudioCodec::Alaw => alaw_to_linear(v),
    }
}

// Canonical G.711 decoders (CCITT/Sun reference).
fn ulaw_to_linear(u: u8) -> i16 {
    let u = !u;
    let t = (((u & 0x0f) as i32) << 3) + 0x84;
    let t = t << ((u & 0x70) >> 4);
    if u & 0x80 != 0 {
        (0x84 - t) as i16
    } else {
        (t - 0x84) as i16
    }
}

fn alaw_to_linear(a: u8) -> i16 {
    let a = a ^ 0x55;
    let seg = (a & 0x70) >> 4;
    let mut t = ((a & 0x0f) as i32) << 4;
    t = match seg {
        0 => t + 8,
        1 => t + 0x108,
        _ => (t + 0x108) << (seg - 1),
    };
    if a & 0x80 != 0 {
        t as i16
    } else {
        -t as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g711_silence_decodes_near_zero() {
        assert_eq!(ulaw_to_linear(0xFF), 0); // µ-law silence
        assert_eq!(alaw_to_linear(0xD5), 8); // A-law silence (±8)
                                             // sign symmetry
        assert!(ulaw_to_linear(0x00) < 0 && ulaw_to_linear(0x80) > 0); // sign symmetry
    }
}
