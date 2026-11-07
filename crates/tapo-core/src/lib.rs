//! TP-Link Tapo cloud protocol.
//! GUI-free and platform-agnostic so it can back a desktop app, CLI, or service.

pub mod client;
mod error;
mod sign;

pub use client::{Cloud, Session, APP_NAME, APP_VER};
pub use error::{Error, Result};
