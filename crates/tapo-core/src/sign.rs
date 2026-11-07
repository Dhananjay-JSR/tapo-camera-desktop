//! Request signing for the TP-Link app WAP endpoints.
//!
//! Every `n-*.i.tplinkcloud.com/api/v2/...` call carries `Content-MD5` of the body and an
//! `X-Authorization: Timestamp=.., Nonce=.., AccessKey=.., Signature=..` header, where the
//! signature is `HMAC-SHA1(SECRET, "<md5>\n<ts>\n<nonce>\n<path>")` hex-encoded. ACCESS_KEY and
//! SECRET are constants required by the TP-Link cloud API (not user secrets).

use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use sha1::Sha1;

pub const ACCESS_KEY: &str = "4d11b6b9d5ea4d19a829adbb9714b057";
const SECRET: &[u8] = b"6ed7d97f3e73467f8a5bab90b577ba4c";
/// The app sends a fixed far-future timestamp; the server does not reject it.
pub const TIMESTAMP: &str = "9999999999";

/// Returns `(content_md5_base64, x_authorization_header)` for a request body + path + nonce.
pub fn sign(body: &[u8], path: &str, nonce: &str) -> (String, String) {
    let md5 = STANDARD.encode(Md5::digest(body));
    let mut mac = Hmac::<Sha1>::new_from_slice(SECRET).expect("hmac key");
    mac.update(format!("{md5}\n{TIMESTAMP}\n{nonce}\n{path}").as_bytes());
    let sig = mac.finalize().into_bytes();
    let hex: String = sig.iter().map(|b| format!("{b:02x}")).collect();
    (
        md5,
        format!("Timestamp={TIMESTAMP}, Nonce={nonce}, AccessKey={ACCESS_KEY}, Signature={hex}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known-good vector for the getAppServiceUrl signature.
    #[test]
    fn matches_app_signature() {
        let (md5, auth) = sign(
            br#"{"serviceIds":["cipc.api.cloud"]}"#,
            "/api/v2/common/getAppServiceUrl",
            "1d16e4dc-8671-4288-8702-8d82838b7586",
        );
        assert_eq!(md5, "F6ZG767UdbygPCZbHUvUFg==");
        assert!(auth.ends_with("Signature=b691465de17f5a015a8e46fbbd5b543bc5bfa9c2"), "{auth}");
    }
}
