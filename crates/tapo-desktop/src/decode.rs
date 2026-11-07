//! Video decoding behind a backend trait: software (openh264, always available) now, with
//! hardware backends (e.g. VideoToolbox) slotting in behind features. Frames are YUV (I420) so the
//! GPU can do the YUV→RGB conversion and scaling at draw time.

use openh264::decoder::Decoder;
use openh264::formats::YUVSource;

/// One decoded picture as tightly-packed I420 planes (no row padding).
pub struct YuvFrame {
    pub w: usize,
    pub h: usize,
    pub y: Vec<u8>, // w * h
    pub u: Vec<u8>, // (w/2) * (h/2)
    pub v: Vec<u8>, // (w/2) * (h/2)
}

/// A video decoder that turns H.264 Annex-B access units into YUV frames.
pub trait VideoDecoder: Send {
    /// `None` until a picture is available (e.g. before the first keyframe).
    fn decode(&mut self, au: &[u8]) -> Option<YuvFrame>;
}

/// Pick the best available decoder. Hardware backends (feature-gated) go here first; openh264 is
/// the always-present fallback so the app works on every platform with no external dependency.
pub fn new_decoder() -> Option<Box<dyn VideoDecoder>> {
    #[cfg(all(target_os = "macos", feature = "hw-videotoolbox"))]
    if let Some(hw) = crate::decode_vt::VideoToolbox::new() {
        return Some(Box::new(hw));
    }
    Some(Box::new(Openh264Dec::new()?))
}

struct Openh264Dec {
    dec: Decoder,
}

impl Openh264Dec {
    fn new() -> Option<Self> {
        Decoder::new().ok().map(|dec| Openh264Dec { dec })
    }
}

impl VideoDecoder for Openh264Dec {
    fn decode(&mut self, au: &[u8]) -> Option<YuvFrame> {
        let yuv = self.dec.decode(au).ok()??;
        let (w, h) = yuv.dimensions();
        let (sy, su, sv) = yuv.strides();
        Some(YuvFrame {
            w,
            h,
            y: pack(yuv.y(), sy, w, h),
            u: pack(yuv.u(), su, w / 2, h / 2),
            v: pack(yuv.v(), sv, w / 2, h / 2),
        })
    }
}

/// Copy a strided plane into a tightly-packed `width * height` buffer.
fn pack(plane: &[u8], stride: usize, width: usize, height: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * height);
    for row in 0..height {
        let start = row * stride;
        out.extend_from_slice(&plane[start..start + width]);
    }
    out
}

/// Scalar BT.601 I420→RGB8 — CPU reference/fallback (GPU shader does this in production).
#[allow(dead_code)]
pub fn yuv_to_rgb(f: &YuvFrame) -> Vec<u8> {
    let mut rgb = vec![0u8; f.w * f.h * 3];
    let cw = f.w / 2;
    for y in 0..f.h {
        for x in 0..f.w {
            let yy = f.y[y * f.w + x] as f32;
            let uu = f.u[(y / 2) * cw + x / 2] as f32 - 128.0;
            let vv = f.v[(y / 2) * cw + x / 2] as f32 - 128.0;
            let i = (y * f.w + x) * 3;
            rgb[i] = (yy + 1.402 * vv).clamp(0.0, 255.0) as u8;
            rgb[i + 1] = (yy - 0.344 * uu - 0.714 * vv).clamp(0.0, 255.0) as u8;
            rgb[i + 2] = (yy + 1.772 * uu).clamp(0.0, 255.0) as u8;
        }
    }
    rgb
}

#[cfg(test)]
mod tests {
    use super::*;
    use openh264::encoder::Encoder;
    use openh264::formats::YUVBuffer;

    #[test]
    fn decodes_to_packed_i420() {
        let mut enc = Encoder::new().unwrap();
        let bits = enc.encode(&YUVBuffer::new(64, 48)).unwrap().to_vec();
        let f = new_decoder().unwrap().decode(&bits).expect("a picture");
        assert_eq!((f.w, f.h), (64, 48));
        assert_eq!(f.y.len(), 64 * 48); // tightly packed
        assert_eq!(f.u.len(), 32 * 24);
        assert_eq!(f.v.len(), 32 * 24);
    }
}
