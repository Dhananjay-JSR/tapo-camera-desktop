//! Microphone capture for talk-back: default input → 16 kHz mono → G.711 µ-law → `Cmd::Mic`.
//! The cpal stream is held on the UI thread (it is `!Send`); audio bytes cross to the worker.

use crate::worker::Cmd;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// Keeps the capture stream alive; dropping it stops the mic.
pub struct Mic {
    _stream: cpal::Stream,
}

struct Resampler {
    step: f32, // input samples per output sample
    pos: f32,
    out: Vec<u8>,
    tx: Sender<Cmd>,
}

impl Resampler {
    fn push_mono(&mut self, sample: f32) {
        // Decimate to 16 kHz: emit an output sample each time `pos` crosses a step boundary.
        self.pos += 1.0;
        if self.pos >= self.step {
            self.pos -= self.step;
            let s = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
            self.out.push(linear_to_ulaw(s));
            if self.out.len() >= 1024 {
                let _ = self.tx.send(Cmd::Mic(std::mem::take(&mut self.out)));
            }
        }
    }
}

pub fn start(tx: Sender<Cmd>) -> Option<Mic> {
    let device = cpal::default_host().default_input_device()?;
    let config = device.default_input_config().ok()?;
    let channels = config.channels() as usize;
    let state = Arc::new(Mutex::new(Resampler {
        step: config.sample_rate().0 as f32 / 16_000.0,
        pos: 0.0,
        out: Vec::with_capacity(2048),
        tx,
    }));
    let err = |e| crate::logbuf::log(format!("mic error: {e}"));
    let s = state.clone();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| {
                let mut r = s.lock().unwrap();
                for frame in data.chunks(channels) {
                    r.push_mono(frame[0]);
                }
            },
            err,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &_| {
                let mut r = s.lock().unwrap();
                for frame in data.chunks(channels) {
                    r.push_mono(frame[0] as f32 / 32768.0);
                }
            },
            err,
            None,
        ),
        _ => return None, // uncommon formats: skip rather than mis-decode
    }
    .ok()?;
    stream.play().ok()?;
    Some(Mic { _stream: stream })
}

/// Standard G.711 µ-law encoder.
fn linear_to_ulaw(sample: i16) -> u8 {
    const BIAS: i32 = 0x84;
    const CLIP: i32 = 32635;
    let sign = if sample < 0 { 0x80 } else { 0x00 };
    let mut mag = (sample as i32).abs().min(CLIP) + BIAS;
    let mut exponent = 7;
    let mut mask = 0x4000;
    while exponent > 0 && (mag & mask) == 0 {
        exponent -= 1;
        mask >>= 1;
    }
    let mantissa = (mag >> (exponent + 3)) & 0x0F;
    mag = sign | (exponent << 4) | mantissa;
    !mag as u8
}

#[cfg(test)]
mod tests {
    use super::linear_to_ulaw;

    #[test]
    fn ulaw_anchors() {
        assert_eq!(linear_to_ulaw(0), 0xFF); // silence
        assert_eq!(linear_to_ulaw(i16::MIN), 0x00); // max negative clips to 0x00
        assert_eq!(linear_to_ulaw(100) & 0x80, 0x80); // positive: sign bit set
        assert_eq!(linear_to_ulaw(-100) & 0x80, 0x00); // negative: sign bit clear
    }
}
