use std::fmt;

#[derive(Debug)]
pub enum Error {
    Http(reqwest::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
    /// TP-Link cloud returned a non-zero `error_code` (WAP) with its message.
    Cloud(String),
    /// A camera `passthrough` command returned a non-zero `error_code`.
    Camera(i64),
    /// Login needs the emailed MFA code; the caller must supply it.
    MfaRequired { process_id: String, email: String },
    Auth(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Http(e) => write!(f, "network error: {e}"),
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::Json(e) => write!(f, "bad response: {e}"),
            Error::Cloud(m) => write!(f, "cloud error: {m}"),
            Error::Camera(c) => write!(f, "camera error {c}"),
            Error::MfaRequired { email, .. } => write!(f, "verification code sent to {email}"),
            Error::Auth(m) => write!(f, "login failed: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error { fn from(e: reqwest::Error) -> Self { Error::Http(e) } }
impl From<std::io::Error> for Error { fn from(e: std::io::Error) -> Self { Error::Io(e) } }
impl From<serde_json::Error> for Error { fn from(e: serde_json::Error) -> Self { Error::Json(e) } }

pub type Result<T> = std::result::Result<T, Error>;
