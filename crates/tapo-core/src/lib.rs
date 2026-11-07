//! TP-Link Tapo cloud protocol.
//! GUI-free and platform-agnostic so it can back a desktop app, CLI, or service.

pub mod alerts;
pub mod client;
pub mod device;
mod error;
pub mod playback;
pub mod relay;
mod relay_socket;
pub use relay_socket::RelaySocket;
mod sign;
pub mod talk;
pub mod ts_mux;
pub mod tsdemux;

pub use alerts::Alert;
pub use client::{Cloud, Session, APP_NAME, APP_VER};

/// This crate's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub use device::Camera;
pub use error::{Error, Result};
pub use playback::Clip;
pub use relay::StreamType;
pub use talk::TalkSession;
pub use tsdemux::{AudioCodec, Demuxed, TsDemux, VideoCodec};
