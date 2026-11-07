# tapo-desktop

Remote live view, pan/tilt, playback, talk-back and alerts for **TP-Link Tapo cameras** on
**Linux, macOS and Windows** — the way the Tapo phone app works, over the TP-Link cloud relay.
No RTSP, no LAN access, nothing to enable on the camera.

Sign in with your TP-Link account (two-factor supported); the app finds your cameras and streams
them from anywhere.

## Features

- **Live view + audio** for every camera (1080p H.264 / G.711), decoded and shown in-window.
- **Pan / tilt** and **presets** (C200, C210; C310 is fixed).
- **Talk-back** — push your microphone to the camera speaker.
- **Playback** of SD-card recordings by day.
- **Camera controls** — LED, privacy mode, siren/alarm arming.
- **Alerts** — desktop-polled motion / person / sound notifications.

## Workspace

| Crate | Role |
|-------|------|
| [`crates/tapo-core`](crates/tapo-core) | Protocol library: login + MFA, camera list, cloud relay (live/playback/talk), device commands, alerts. GUI-free, reusable, unit-tested. |
| [`crates/tapo-desktop`](crates/tapo-desktop) | The cross-platform [egui](https://github.com/emilk/egui) app built on `tapo-core`. |

The core is fully decoupled from the UI — it can also back a CLI, a service, or Home Assistant.

## Build & run

Self-contained — **no ffmpeg needed**. H.264 is decoded in-app (openh264, compiled in) and audio
plays via the OS. You only need a Rust toolchain with a C compiler (for openh264).

```sh
# Linux also needs: sudo apt-get install libasound2-dev libxkbcommon-dev libwayland-dev libxcb1-dev
cargo run --release -p tapo-desktop
```

## How it works

The protocol was built for the TP-Link cloud.
Highlights: HMAC-SHA1 request signing, a cloud relay that streams MPEG-TS over a long HTTPS POST,
G.711 audio carried under private stream types, and a half-duplex talk-back uplink. Video is
decoded with the bundled openh264; audio uses cpal — so the app has no external runtime deps. The embedded
`tplink_ca.pem` is TP-Link's public app CA, needed to validate the cloud hosts.

## Notes

- Session (incl. the cloud token) is stored under your OS config dir, readable only by you.
- This is an unofficial client and may break if TP-Link changes its cloud.

## License

MIT
