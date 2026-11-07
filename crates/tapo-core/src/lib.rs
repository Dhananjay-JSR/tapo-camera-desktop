//! TP-Link Tapo cloud protocol.
//! GUI-free and platform-agnostic so it can back a desktop app, CLI, or service.

pub mod client;
mod error;
mod sign;
pub mod device;
pub mod relay;
mod relay_socket;
pub mod ts_mux;
pub mod talk;
pub mod audio;
pub mod alerts;
pub mod playback;

pub use client::{Cloud, Session, APP_NAME, APP_VER};
pub use device::Camera;
pub use relay::StreamType;
pub use audio::{AudioDemux, Codec};
pub use alerts::Alert;
pub use playback::Clip;
pub use talk::TalkSession;
pub use error::{Error, Result};
