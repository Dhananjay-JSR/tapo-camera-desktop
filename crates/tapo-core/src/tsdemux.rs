//! Split the camera's MPEG-TS into H.264 video access units and G.711 audio.
//!
//! Video and audio PIDs are discovered from the PMT (stream_type 0x1b = H.264, 0x24 = HEVC;
//! 0x90 = G.711 A-law, 0x91 = µ-law). The H.264 elementary stream in TS is already Annex-B, so
//! video frames are the PES payloads concatenated per access unit. G.711 we hand back as raw bytes.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AudioCodec {
    Alaw,
    Mulaw,
}

impl AudioCodec {
    /// ffmpeg `-f` demuxer name (used by any ffmpeg-based consumer).
    pub fn ffmpeg(self) -> &'static str {
        match self {
            AudioCodec::Alaw => "alaw",
            AudioCodec::Mulaw => "mulaw",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum VideoCodec {
    H264,
    Hevc,
}

/// Output of one `feed` call: completed video access units, plus raw G.711 bytes.
#[derive(Default)]
pub struct Demuxed {
    pub frames: Vec<Vec<u8>>,
    pub audio: Vec<u8>,
}

#[derive(Default)]
pub struct TsDemux {
    pmt_pid: Option<u16>,
    video_pid: Option<u16>,
    audio_pid: Option<u16>,
    pub video_codec: Option<VideoCodec>,
    pub audio_codec: Option<AudioCodec>,
    frame: Vec<u8>, // current video access unit being accumulated
}

impl TsDemux {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed MPEG-TS bytes (any multiple-of-188 slice).
    pub fn feed(&mut self, ts: &[u8]) -> Demuxed {
        let mut out = Demuxed::default();
        let mut i = 0;
        while i + 188 <= ts.len() {
            let p = &ts[i..i + 188];
            i += 188;
            if p[0] != 0x47 {
                continue;
            }
            let pid = (((p[1] & 0x1f) as u16) << 8) | p[2] as u16;
            let pusi = p[1] & 0x40 != 0;
            let afc = (p[3] >> 4) & 3;
            if afc == 2 {
                continue; // adaptation only
            }
            let off = 4 + if afc == 3 { p[4] as usize + 1 } else { 0 };
            if off >= 188 {
                continue;
            }
            let pl = &p[off..];

            if pid == 0 && pusi && self.pmt_pid.is_none() {
                if pl.len() > 1 + 11 {
                    self.pmt_pid = Some((((pl[1 + 10] & 0x1f) as u16) << 8) | pl[1 + 11] as u16);
                }
            } else if Some(pid) == self.pmt_pid
                && pusi
                && self.audio_pid.is_none()
                && self.video_pid.is_none()
            {
                self.parse_pmt(pl);
            } else if Some(pid) == self.video_pid {
                let es = strip_pes(pl, pusi);
                if pusi && !self.frame.is_empty() {
                    out.frames.push(std::mem::take(&mut self.frame)); // previous access unit complete
                }
                self.frame.extend_from_slice(es);
            } else if Some(pid) == self.audio_pid {
                out.audio.extend_from_slice(strip_pes(pl, pusi));
            }
        }
        out
    }

    fn parse_pmt(&mut self, pl: &[u8]) {
        let sec = &pl[1 + pl[0] as usize..];
        if sec.len() <= 12 {
            return;
        }
        let section_len = (((sec[1] & 0xf) as usize) << 8) | sec[2] as usize;
        let prog_info_len = (((sec[10] & 0xf) as usize) << 8) | sec[11] as usize;
        let mut j = 12 + prog_info_len;
        let end = (3 + section_len.saturating_sub(4)).min(sec.len());
        while j + 5 <= end {
            let stype = sec[j];
            let epid = (((sec[j + 1] & 0x1f) as u16) << 8) | sec[j + 2] as u16;
            let es_len = (((sec[j + 3] & 0xf) as usize) << 8) | sec[j + 4] as usize;
            match stype {
                0x1b => {
                    self.video_pid = Some(epid);
                    self.video_codec = Some(VideoCodec::H264);
                }
                0x24 => {
                    self.video_pid = Some(epid);
                    self.video_codec = Some(VideoCodec::Hevc);
                }
                0x90 => {
                    self.audio_pid = Some(epid);
                    self.audio_codec = Some(AudioCodec::Alaw);
                }
                0x91 => {
                    self.audio_pid = Some(epid);
                    self.audio_codec = Some(AudioCodec::Mulaw);
                }
                _ => {}
            }
            j += 5 + es_len;
        }
    }
}

/// Return the elementary-stream bytes of a TS payload, skipping the PES header on a unit start.
fn strip_pes(pl: &[u8], pusi: bool) -> &[u8] {
    if pusi && pl.len() > 9 && pl[0] == 0 && pl[1] == 0 && pl[2] == 1 {
        let header_len = 9 + pl[8] as usize;
        if header_len <= pl.len() {
            return &pl[header_len..];
        }
    }
    pl
}

#[cfg(test)]
mod tests {
    use super::*;

    // 188-byte packet; short payloads are padded with adaptation-field stuffing, as real TS does.
    fn pkt(pid: u16, payload: &[u8]) -> Vec<u8> {
        let mut p = vec![0x47, 0x40 | (pid >> 8) as u8, (pid & 0xff) as u8];
        if payload.len() == 184 {
            p.push(0x10);
        } else {
            p.push(0x30);
            let afl = 184 - payload.len() - 1;
            p.push(afl as u8);
            if afl > 0 {
                p.push(0x00);
                p.extend(std::iter::repeat(0xff).take(afl - 1));
            }
        }
        p.extend_from_slice(payload);
        assert_eq!(p.len(), 188);
        p
    }

    // PMT carrying both an H.264 video ES and a µ-law audio ES.
    fn pmt_av() -> Vec<u8> {
        let mut sec = vec![0x02u8, 0xb0, 0x00]; // table id + section_len placeholder
        sec.resize(10, 0);
        sec.push(0x00); // prog_info_len hi
        sec.push(0x00); // prog_info_len lo
        sec.extend_from_slice(&[0x1b, 0x00, 0x44, 0x00, 0x00]); // H.264 on pid 0x44
        sec.extend_from_slice(&[0x91, 0x00, 0x45, 0x00, 0x00]); // µ-law on pid 0x45
        sec.extend_from_slice(&[0, 0, 0, 0]); // CRC
        let body = sec.len() - 3; // section_length counts bytes after the length field
        sec[2] = body as u8;
        let mut pl = vec![0u8]; // pointer field
        pl.extend_from_slice(&sec);
        pl
    }

    fn pes(stream_id: u8, data: &[u8]) -> Vec<u8> {
        let mut v = vec![0, 0, 1, stream_id, 0, 0, 0x80, 0x00, 0x00]; // 9-byte PES header, no PTS
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn splits_video_and_audio() {
        let mut pat = vec![0u8; 13];
        pat[11] = 0x00;
        pat[12] = 0x42; // PMT pid 0x42
        let mut d = TsDemux::new();
        d.feed(&pkt(0, &pat));
        d.feed(&pkt(0x42, &pmt_av()));
        assert_eq!(d.video_codec, Some(VideoCodec::H264));
        assert_eq!(d.audio_codec, Some(AudioCodec::Mulaw));

        // two video access units (each PES is one frame) + one audio chunk
        let out1 = d.feed(&pkt(0x44, &pes(0xe0, &[0, 0, 0, 1, 0x65, 0xaa]))); // IDR-ish NAL
        assert!(out1.frames.is_empty()); // first frame still accumulating
        let out2 = d.feed(&pkt(0x44, &pes(0xe0, &[0, 0, 0, 1, 0x41, 0xbb])));
        assert_eq!(out2.frames.len(), 1); // previous frame emitted on next unit start
        assert_eq!(out2.frames[0], vec![0, 0, 0, 1, 0x65, 0xaa]);

        let a = d.feed(&pkt(0x45, &pes(0xc0, &[0x10, 0x20, 0x30])));
        assert_eq!(a.audio, vec![0x10, 0x20, 0x30]);
    }
}
