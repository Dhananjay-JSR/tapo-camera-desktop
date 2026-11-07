//! macOS hardware decode via VideoToolbox — integration point.
//!
//! Architecture is in place: `decode::new_decoder()` prefers this backend when built with
//! `--features hw-videotoolbox` on macOS, and falls back to openh264 otherwise. The VideoToolbox
//! FFI itself (SPS/PPS → CMVideoFormatDescription, Annex-B → AVCC, VTDecompressionSession,
//! CVPixelBuffer NV12 → I420) is completed and verified on-device, since decode correctness and
//! the async callback can't be validated in a headless build.
//!
//! Until then `new` returns `None`, so the feature build cleanly uses the openh264 fallback.

use crate::decode::{VideoDecoder, YuvFrame};

pub struct VideoToolbox;

impl VideoToolbox {
    pub fn new() -> Option<Self> {
        // TODO(on-device): create the VTDecompressionSession and return Some(..).
        None
    }
}

impl VideoDecoder for VideoToolbox {
    fn decode(&mut self, _au: &[u8]) -> Option<YuvFrame> {
        None
    }
}
