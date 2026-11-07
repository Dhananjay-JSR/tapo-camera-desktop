//! A single bidirectional TLS connection to a relay host.
//!
//! The relay is one long HTTP/1.1 POST: the request body carries upstream parts
//! (`----client-stream-boundary--`: playback requests, talk audio) while the response body
//! carries the camera's `----device-stream-boundary--` parts. reqwest sends the whole body
//! before reading the response, so live/playback/talk use this manual socket instead.

use crate::error::{Error, Result};
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, Stream};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

fn tls_config() -> Arc<ClientConfig> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    // TP-Link's private CA, for the direct `*.i.tplinkcloud.com` relay hosts.
    for cert in rustls_pemfile::certs(&mut &include_bytes!("../tplink_ca.pem")[..]).flatten() {
        let _ = roots.add(CertificateDer::from(cert));
    }
    Arc::new(ClientConfig::builder().with_root_certificates(roots).with_no_client_auth())
}

pub struct RelaySocket {
    conn: ClientConnection,
    sock: TcpStream,
}

impl RelaySocket {
    /// Connect, send the POST request line + headers, and leave the stream positioned at the
    /// response body. `headers` are extra `(name, value)` pairs after the fixed relay set.
    pub fn open(host: &str, path_and_query: &str, token: &str, cdn_origin: Option<&str>) -> Result<Self> {
        let server = ServerName::try_from(host.to_string())
            .map_err(|_| Error::Cloud(format!("bad relay host {host}")))?;
        let conn = ClientConnection::new(tls_config(), server)
            .map_err(|e| Error::Cloud(format!("tls: {e}")))?;
        let sock = TcpStream::connect((host, 443))?;
        let mut s = RelaySocket { conn, sock };

        let mut req = format!(
            "POST {path_and_query}&retryTime=0 HTTP/1.1\r\n\
             Host: {host}\r\n\
             User-Agent: Client=TP-Link_Tapo_Android Android 3.22.503/1.3\r\n\
             Keep-Relay: 3600\r\nAccept: */*\r\n\
             Content-Type: multipart/mixed;boundary=--client-stream-boundary--\r\n\
             Content-Length: 21474836470\r\n\
             X-token: {token}\r\nX-Client-Model: pc\r\nX-Pull-Mode: 2\r\n\
             X-Compete: 1\r\nX-Version: 2.0\r\n"
        );
        if let Some(o) = cdn_origin {
            req.push_str(&format!("X-CDN-Origin: {o}\r\n"));
        }
        req.push_str("\r\n");
        s.tls().write_all(req.as_bytes())?;
        s.tls().flush()?;

        let status = s.read_line()?;
        if !status.contains(" 200") {
            return Err(Error::Cloud(format!("relay: {}", status.trim())));
        }
        while !s.read_line()?.trim().is_empty() {} // consume response headers
        Ok(s)
    }

    fn tls(&mut self) -> Stream<'_, ClientConnection, TcpStream> {
        Stream::new(&mut self.conn, &mut self.sock)
    }

    fn read_line(&mut self) -> Result<String> {
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if self.tls().read(&mut byte)? == 0 {
                break;
            }
            line.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        Ok(String::from_utf8_lossy(&line).into_owned())
    }

    /// Send an upstream `----client-stream-boundary--` part.
    pub fn write_part(&mut self, headers: &[(&str, String)], body: &[u8]) -> Result<()> {
        let mut head = String::from("----client-stream-boundary--\r\n");
        for (k, v) in headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
        let mut tls = self.tls();
        tls.write_all(head.as_bytes())?;
        tls.write_all(body)?;
        tls.write_all(b"\r\n")?;
        tls.flush()?;
        Ok(())
    }

    /// Read the next JSON control part the camera sends (used by talk-back for the session id).
    pub fn read_json_part(&mut self) -> Result<String> {
        loop {
            let line = self.read_line()?;
            if line.is_empty() {
                return Err(Error::Cloud("relay closed".into()));
            }
            if !line.starts_with("----device-stream-boundary--") {
                continue;
            }
            let mut len = 0usize;
            loop {
                let h = self.read_line()?;
                let t = h.trim_end();
                if t.is_empty() {
                    break;
                }
                if let Some(v) = t.strip_prefix("Content-Length:").or_else(|| t.strip_prefix("content-length:")) {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; len];
            self.tls().read_exact(&mut body)?;
            return Ok(String::from_utf8_lossy(&body).into_owned());
        }
    }
}

impl Read for RelaySocket {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.tls().read(buf)
    }
}

/// Buffered reader for consuming the response parts after any upstream setup is written.
pub fn into_buf(sock: RelaySocket) -> BufReader<RelaySocket> {
    BufReader::new(sock)
}
#[allow(dead_code)]
fn _assert_bufread<R: BufRead>(_: R) {}
