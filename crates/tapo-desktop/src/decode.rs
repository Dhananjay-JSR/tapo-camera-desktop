//! H.264 → RGB using openh264 (compiled into the binary; no system ffmpeg).

use openh264::decoder::Decoder;
use openh264::formats::YUVSource;

pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

pub struct H264 {
    dec: Decoder,
}

impl H264 {
    pub fn new() -> Option<Self> {
        Decoder::new().ok().map(|dec| H264 { dec })
    }

    /// Decode one Annex-B access unit. `None` until the decoder has a picture (e.g. before the first keyframe).
    pub fn decode(&mut self, au: &[u8]) -> Option<Frame> {
        let yuv = self.dec.decode(au).ok()??;
        let (width, height) = yuv.dimensions();
        let mut rgb = vec![0u8; width * height * 3];
        yuv.write_rgb8(&mut rgb);
        Some(Frame { width, height, rgb })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openh264::encoder::Encoder;
    use openh264::formats::YUVBuffer;

    #[test]
    fn decodes_an_encoded_frame() {
        let mut enc = Encoder::new().unwrap();
        let bits = enc.encode(&YUVBuffer::new(64, 48)).unwrap().to_vec();
        let f = H264::new().unwrap().decode(&bits).expect("a picture");
        assert_eq!((f.width, f.height), (64, 48));
        assert_eq!(f.rgb.len(), 64 * 48 * 3);
    }
}
