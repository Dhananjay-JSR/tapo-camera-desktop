# Decisions

## D1: Decode video in-app instead of shelling out to ffplay  (2026-11-08, decided by: user, status: accepted)
> In the desktop app, facing a hard runtime dependency on ffmpeg, we chose in-app decoding (openh264 + cpal) over bundling an ffmpeg binary, to ship one self-contained binary, accepting a C compiler at build time and software-decode CPU cost.
- Options:
  - A: openh264 in-app. Pro: self-contained, video inside the window. Con: build needs a C compiler; CPU decode.
  - B: bundle ffplay per OS. Pro: no new code. Con: ~70 MB per platform, separate window, ffmpeg licensing.
- Chosen: A.
- Hypothesis: the cameras send H.264 that openh264 decodes at ≥15 fps for one 1080p stream on a normal laptop.
- Falsified if: decode lags (frame queue grows) or a camera only offers H.265.
- Pre-mortem: a camera firmware update switches the relay stream to H.265 and video goes blank.
- Code: crates/tapo-desktop (decoder), crates/tapo-core/src/tsdemux.rs

## D2: H.264 only; show a message for H.265  (2026-11-08, decided by: assumed, status: accepted)
> Facing openh264's H.264-only support, we chose to decode H.264 and report H.265 clearly, over adding an HEVC decoder, to keep the dependency set small, accepting no video from H.265-only streams.
- Hypothesis: all relay streams observed so far are H.264 (stream_type 0x1b) — true for C200, C210, C310 captures.
- Falsified if: TsDemux reports VideoCodec::Hevc for a camera.

## D3: Decode only the camera being viewed  (2026-11-08, decided by: assumed, status: accepted)
> Facing software-decode CPU cost, we chose one decoded stream at a time over a live grid, to stay responsive on laptops, accepting no multi-camera grid for now.
- Falsified if: users need simultaneous views; then measure per-stream CPU before adding a grid.

## D4: Remove ffplay entirely  (2026-11-08, decided by: assumed, status: accepted)
> We chose to delete the ffplay path over keeping it as a fallback, to truly drop the external dependency, accepting no fallback if openh264 fails on a stream.

## D5: TS demux lives in tapo-core, codecs in tapo-desktop  (2026-11-08, decided by: assumed, status: accepted)
> To keep the core library codec- and GUI-free, the core only splits TS into H.264 access units and G.711 bytes; decoding and playback live in the app.
- Code: crates/tapo-core/src/tsdemux.rs

## D6: GPU does YUV→RGB, not the CPU  (2026-11-08, decided by: user, status: accepted)
> In the desktop renderer, facing per-frame CPU color-convert + upload cost, we chose a glow fragment shader (upload I420 planes, convert on draw) over CPU conversion, to keep the CPU free for decode and make scaling/tiling cheap, accepting ~150 lines of GL that need on-device visual verification.
- Chosen: GPU shader (src/video_gl.rs); CPU yuv_to_rgb kept as reference/fallback.
- Hypothesis: eframe's glow backend is available on all target desktops so the paint callback runs.
- Falsified if: a platform uses a non-glow backend (callback no-ops → black video).
- Open concern: GL render path is compile-verified only; not yet seen on a display.

## D7: Hardware decode behind a trait + feature, openh264 as the floor  (2026-11-08, decided by: user, status: accepted)
> Facing high software-decode CPU for multi-camera, we chose pluggable VideoDecoder backends with native hardware decode (VideoToolbox first) behind `hw-videotoolbox`, falling back to openh264, over requiring ffmpeg (hard self-contained requirement), accepting per-OS unsafe FFI completed and tested on-device.
- Chosen: trait in src/decode.rs; VideoToolbox integration point in src/decode_vt.rs (scaffold).
- Hypothesis: VideoToolbox decodes the cameras' H.264 and NV12→I420 maps cleanly to our YuvFrame.
- Falsified if: VT returns errors for the stream's SPS/PPS, or colors/planes are wrong after conversion.
- Open concern: FFI not yet written/verified; default build uses openh264.

## Verified (2026-11-08)
- Live view confirmed end-to-end on real hardware: cloud relay → openh264 → **GPU glow shader** → egui, with cpal audio. Resolves the D6 open concern (GL path was compile-only). Fixes along the way: JSON error codes are strings (MFA), rustls needs an explicit `ring` provider, and the raw relay socket must send X-Client-UUID / X-Track-Id / X-Redirect-Times / X-Arrive-Latency (HTTP 400 without them).
