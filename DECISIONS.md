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
