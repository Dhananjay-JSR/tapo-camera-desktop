//! Pull G.711 audio out of the camera's MPEG-TS. Tapo marks it with private stream types
//! 0x90 (A-law) / 0x91 (µ-law), which ffmpeg misreads as MP3, so we extract it ourselves.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Codec {
    Alaw,
    Mulaw,
}

impl Codec {
    /// ffmpeg `-f` demuxer name.
    pub fn ffmpeg(self) -> &'static str {
        match self {
            Codec::Alaw => "alaw",
            Codec::Mulaw => "mulaw",
        }
    }
}

#[derive(Default)]
pub struct AudioDemux {
    pmt_pid: Option<u16>,
    audio_pid: Option<u16>,
    pub codec: Option<Codec>,
}

impl AudioDemux {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed MPEG-TS bytes (any multiple-of-188 slice); returns extracted raw G.711 samples.
    pub fn feed(&mut self, ts: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
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
                continue; // adaptation field only, no payload
            }
            let off = 4 + if afc == 3 { p[4] as usize + 1 } else { 0 };
            if off >= 188 {
                continue;
            }
            let pl = &p[off..];
            if pid == 0 && pusi && self.pmt_pid.is_none() {
                // PAT: first program's PMT pid
                if pl.len() > 1 + 12 {
                    self.pmt_pid = Some((((pl[1 + 10] & 0x1f) as u16) << 8) | pl[1 + 11] as u16);
                }
            } else if Some(pid) == self.pmt_pid && pusi && self.audio_pid.is_none() {
                let sec = &pl[1 + pl[0] as usize..];
                if sec.len() > 12 {
                    let section_len = (((sec[1] & 0xf) as usize) << 8) | sec[2] as usize;
                    let prog_info_len = (((sec[10] & 0xf) as usize) << 8) | sec[11] as usize;
                    let mut j = 12 + prog_info_len;
                    let end = 3 + section_len.saturating_sub(4);
                    while j + 5 <= end.min(sec.len()) {
                        let stype = sec[j];
                        let epid = (((sec[j + 1] & 0x1f) as u16) << 8) | sec[j + 2] as u16;
                        let es_len = (((sec[j + 3] & 0xf) as usize) << 8) | sec[j + 4] as usize;
                        if stype == 0x90 || stype == 0x91 {
                            self.audio_pid = Some(epid);
                            self.codec = Some(if stype == 0x90 { Codec::Alaw } else { Codec::Mulaw });
                        }
                        j += 5 + es_len;
                    }
                }
            } else if Some(pid) == self.audio_pid {
                let mut data = pl;
                if pusi && data.len() > 9 && data[0] == 0 && data[1] == 0 && data[2] == 1 {
                    let header_len = 9 + data[8] as usize;
                    if header_len <= data.len() {
                        data = &data[header_len..]; // skip PES header
                    }
                }
                out.extend_from_slice(data);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 188-byte TS packet with a single-packet payload (PUSI set).
    fn pkt(pid: u16, payload: &[u8]) -> Vec<u8> {
        let mut p = vec![0x47, 0x40 | (pid >> 8) as u8, (pid & 0xff) as u8, 0x10];
        p.extend_from_slice(payload);
        p.resize(188, 0xff);
        p
    }

    #[test]
    fn detects_mulaw_and_extracts_payload() {
        // PAT: pointer(0) + table so that byte[1+10],[1+11] => PMT pid 0x42.
        let mut pat = vec![0u8; 13];
        pat[11] = 0x00;
        pat[12] = 0x42;
        // PMT on pid 0x42: section_len small, prog_info_len 0, one ES: stype 0x91 pid 0x44 es_len 0.
        let mut pmt_sec = vec![0x02u8, 0x00, 0x00];
        pmt_sec[1] = 0x00; // section_len high
        pmt_sec[2] = 18; // section_len: 9 fixed + 5 ES + 4 CRC
        pmt_sec.resize(10, 0);
        pmt_sec.push(0x00); // prog_info_len hi
        pmt_sec.push(0x00); // prog_info_len lo
        pmt_sec.extend_from_slice(&[0x91, 0x00, 0x44, 0x00, 0x00]); // one ES: stype, pid hi, pid lo, es_len hi, es_len lo
        let mut pmt = vec![0u8]; // pointer field
        pmt.extend_from_slice(&pmt_sec);
        // audio packet on pid 0x44: 9-byte PES header + 175 bytes of "audio" = a full 184-byte payload.
        let samples: Vec<u8> = (0..175u16).map(|i| i as u8).collect();
        let mut aud = vec![0, 0, 1, 0xc0, 0, 0, 0x80, 0x00, 0x00];
        aud.extend_from_slice(&samples);

        let mut d = AudioDemux::new();
        let mut got = Vec::new();
        got.extend(d.feed(&pkt(0, &pat)));
        got.extend(d.feed(&pkt(0x42, &pmt)));
        got.extend(d.feed(&pkt(0x44, &aud)));
        assert_eq!(d.codec, Some(Codec::Mulaw));
        assert_eq!(got, samples);
    }
}
