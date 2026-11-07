# tapo-desktop

Remote live view, pan/tilt, playback, talk-back and alerts for TP-Link Tapo cameras
on **Linux, macOS and Windows** — the way the Tapo phone app works (TP-Link cloud relay),
with no RTSP and no LAN access required.

## Workspace

- **`crates/tapo-core`** — protocol library (login + MFA, camera list, cloud relay video/audio,
  camera commands, playback, alerts). No GUI, no platform assumptions. Reusable.
- **`crates/tapo-desktop`** — the cross-platform desktop app built on `tapo-core`.

## Status

Protocol fully decoded and verified against real cameras; see the core crate.

## Requirements

`ffmpeg`/`ffplay` on PATH for video+audio playback (decoding is delegated, not bundled).
