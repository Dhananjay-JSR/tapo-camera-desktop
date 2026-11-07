//! Minimal MPEG-TS muxer for one G.711 audio stream, for talk-back uplink.
//! Produces the camera's layout: PAT (pid 0) + PMT (pid 0x42, stream_type 0x91) + PES on pid 0x44.

const PMT_PID: u16 = 0x42;
const AUDIO_PID: u16 = 0x44;

#[derive(Default)]
pub struct TsMux {
    cc_pat: u8,
    cc_pmt: u8,
    cc_aud: u8,
    pts: u64,
}

impl TsMux {
    pub fn new() -> Self {
        Self::default()
    }

    /// One talk-back part for `pcm` G.711 bytes (16 kHz → 1024 bytes ≈ 64 ms per part).
    pub fn frame(&mut self, pcm: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(188 * (2 + pcm.len() / 184 + 1));
        out.extend_from_slice(&psi(0, &pat(), &mut self.cc_pat));
        out.extend_from_slice(&psi(PMT_PID, &pmt(), &mut self.cc_pmt));

        // PES: start code + stream_id 0xC0 + length + flags + 5-byte PTS, then the audio.
        let mut pes = vec![0x00, 0x00, 0x01, 0xC0];
        let payload_len = (pcm.len() + 8) as u16; // 3 bytes (flags+hdrlen) +5 PTS + audio? => 8 header bytes after length field
        pes.extend_from_slice(&payload_len.to_be_bytes());
        pes.extend_from_slice(&[0x80, 0x80, 0x05]);
        pes.extend_from_slice(&encode_pts(self.pts));
        pes.extend_from_slice(pcm);
        self.pts = self.pts.wrapping_add(90_000 * pcm.len() as u64 / 16_000);

        let mut first = true;
        let mut i = 0;
        while i < pes.len() {
            let chunk = &pes[i..(i + 184).min(pes.len())];
            out.extend_from_slice(&audio_packet(chunk, first, &mut self.cc_aud));
            first = false;
            i += 184;
        }
        out
    }
}

fn encode_pts(pts: u64) -> [u8; 5] {
    // '0010' marker + 3 PTS fields each followed by a marker bit.
    [
        0x21 | (((pts >> 30) & 0x07) << 1) as u8,
        ((pts >> 22) & 0xff) as u8,
        0x01 | (((pts >> 15) & 0x7f) << 1) as u8,
        ((pts >> 7) & 0xff) as u8,
        0x01 | ((pts & 0x7f) << 1) as u8,
    ]
}

fn pat() -> Vec<u8> {
    // table_id 0, program 1 -> PMT_PID; section covers 9 fixed + 4 program + 4 CRC(unused by our reader).
    let mut s = vec![0x00, 0xb0, 0x0d, 0x00, 0x01, 0xc1, 0x00, 0x00];
    s.extend_from_slice(&[
        0x00,
        0x01,
        0xe0 | (PMT_PID >> 8) as u8,
        (PMT_PID & 0xff) as u8,
    ]);
    s.extend_from_slice(&[0, 0, 0, 0]); // CRC placeholder (camera does not validate for uplink)
    s
}

fn pmt() -> Vec<u8> {
    // table_id 2, one ES: stream_type 0x91 on AUDIO_PID, prog_info_len 0, es_info_len 0.
    let mut s = vec![0x02, 0xb0, 0x12, 0x00, 0x01, 0xc1, 0x00, 0x00];
    s.extend_from_slice(&[
        0xe0 | (AUDIO_PID >> 8) as u8,
        (AUDIO_PID & 0xff) as u8,
        0x00,
        0x00,
    ]);
    s.extend_from_slice(&[
        0x91,
        0xe0 | (AUDIO_PID >> 8) as u8,
        (AUDIO_PID & 0xff) as u8,
        0x00,
        0x00,
    ]);
    s.extend_from_slice(&[0, 0, 0, 0]); // CRC placeholder
    s
}

fn psi(pid: u16, section: &[u8], cc: &mut u8) -> [u8; 188] {
    let mut p = [0xffu8; 188];
    p[0] = 0x47;
    p[1] = 0x40 | (pid >> 8) as u8; // PUSI
    p[2] = (pid & 0xff) as u8;
    p[3] = 0x10 | (*cc & 0x0f);
    *cc = cc.wrapping_add(1);
    p[4] = 0x00; // pointer field
    p[5..5 + section.len()].copy_from_slice(section);
    p
}

fn audio_packet(payload: &[u8], pusi: bool, cc: &mut u8) -> [u8; 188] {
    let mut p = [0u8; 188];
    p[0] = 0x47;
    p[1] = ((pusi as u8) << 6) | (AUDIO_PID >> 8) as u8;
    p[2] = (AUDIO_PID & 0xff) as u8;
    if payload.len() == 184 {
        p[3] = 0x10 | (*cc & 0x0f);
        p[4..].copy_from_slice(payload);
    } else {
        // adaptation field pads the short (last) packet so it stays 188 bytes.
        p[3] = 0x30 | (*cc & 0x0f);
        let afl = 184 - payload.len() - 1;
        p[4] = afl as u8;
        if afl >= 1 {
            p[5] = 0x00; // adaptation flags
            for b in p.iter_mut().take(5 + afl).skip(6) {
                *b = 0xff;
            }
        }
        p[188 - payload.len()..].copy_from_slice(payload);
    }
    *cc = cc.wrapping_add(1);
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{AudioDemux, Codec};

    #[test]
    fn roundtrips_through_demux() {
        let samples: Vec<u8> = (0..1024u16).map(|i| (i & 0xff) as u8).collect();
        let ts = TsMux::new().frame(&samples);
        assert_eq!(ts.len() % 188, 0);
        let mut d = AudioDemux::new();
        let got = d.feed(&ts);
        assert_eq!(d.codec, Some(Codec::Mulaw));
        assert_eq!(got, samples, "muxed audio must survive a demux round-trip");
    }
}
