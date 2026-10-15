# Vise Transcoder Roadmap

A feature-by-feature build guide. This describes **what** is built and **in what order**, not how it is implemented. Features are grouped; a group is only complete when every item in it is checked.

Last updated: 2026-10-15  
Target parity: HandBrake 1.11.2 architecture and user-facing capability set.

## How progress is tracked

- Use `[ ]` for not started, `[x]` for complete.
- A group is **done** only when all of its items are checked.
- A milestone is reached only when every required group for that milestone is done.
- Optional capabilities are marked with `(optional)`.

## Architecture decision snapshot

- Clean-room implementation: new code, not a fork.
- Core: Rust library with a stable API boundary.
- First desktop frontend: Tauri + web UI.
- Media processing parity with HandBrake 1.11.2 is the long-term goal.

---

## Group 0 — Project foundation

- [ ] Rust core crate workspace established
- [ ] Canonical job specification schema defined
- [ ] Core session/job API defined
- [ ] Media buffer and packet model defined
- [ ] Progress/state model defined
- [ ] Geometry and aspect-ratio behavior defined

**Group status: 0 / 6**

## Group 1 — Media ingestion

- [ ] File source scanning
- [ ] Media metadata extraction
- [ ] Title discovery and selection
- [ ] Chapter discovery
- [ ] Audio track enumeration
- [ ] Subtitle track enumeration
- [ ] Preview frame extraction
- [ ] Crop detection
- [ ] DVD source support `(optional)`
- [ ] Blu-ray source support `(optional)`

**Group status: 0 / 10**

## Group 2 — Core transcode baseline

- [ ] Common video decoding
- [ ] Common audio decoding
- [ ] H.264 software encoding
- [ ] AAC audio encoding
- [ ] MP4 muxing
- [ ] MKV muxing
- [ ] WebM muxing
- [ ] End-to-end single-file transcode

**Group status: 0 / 8**

## Group 3 — Command-line interface

- [ ] CLI job execution
- [ ] Source/title/output arguments
- [ ] Preset loading from CLI
- [ ] Batch queue execution
- [ ] Progress output
- [ ] Cancellation support

**Group status: 0 / 6**

## Group 4 — Desktop application shell

- [ ] Source selection
- [ ] Scan and title display
- [ ] Title and track selection
- [ ] Job queue
- [ ] Encode progress display
- [ ] Log display
- [ ] Pause/resume/cancel controls

**Group status: 0 / 7**

## Group 5 — Presets and configuration

- [ ] Built-in preset library
- [ ] Preset application to jobs
- [ ] User preset creation
- [ ] User preset editing
- [ ] User preset deletion
- [ ] Settings persistence
- [ ] Preset import/export

**Group status: 0 / 7**

## Group 6 — Video processing

- [ ] Crop and scale
- [ ] Deinterlacing
- [ ] Comb detection
- [ ] Detelecine
- [ ] Denoise
- [ ] Sharpen
- [ ] Rotate
- [ ] Pad
- [ ] Color space conversion
- [ ] Grayscale
- [ ] Chroma smoothing
- [ ] Constant frame rate handling
- [ ] Variable frame rate handling
- [ ] 10-bit processing
- [ ] HDR metadata preservation

**Group status: 0 / 14**

## Group 7 — Audio pipeline

- [ ] Multiple audio tracks
- [ ] Audio mixdowns
- [ ] Audio resampling
- [ ] Audio dithering
- [ ] Audio delay/gain
- [ ] Audio passthrough
- [ ] Vorbis encoding
- [ ] Opus encoding
- [ ] FLAC encoding
- [ ] MP3 encoding
- [ ] AC3/E-AC3 passthrough

**Group status: 0 / 11**

## Group 8 — Subtitles

- [ ] Text subtitle passthrough
- [ ] Bitmap subtitle passthrough
- [ ] Subtitle burn-in
- [ ] SRT to MP4 text conversion
- [ ] SSA/ASS handling
- [ ] Closed caption handling
- [ ] Foreign audio scan

**Group status: 0 / 7**

## Group 9 — Chapters and metadata

- [ ] Chapter copy
- [ ] Chapter editing
- [ ] Metadata copy
- [ ] Metadata editing
- [ ] Cover art handling
- [ ] Attachment handling

**Group status: 0 / 6**

## Group 10 — Encoding control and job reliability

- [ ] Constant quality encoding
- [ ] Average bitrate encoding
- [ ] Target file size mode
- [ ] Two-pass encoding
- [ ] Lossless encoding where supported
- [ ] Job retry and failure reporting
- [ ] Queue persistence across app restarts
- [ ] Cancel behavior without corrupting output

**Group status: 0 / 8**

## Group 11 — Hardware acceleration

- [ ] Hardware decode detection
- [ ] Hardware video decoding
- [ ] Hardware H.264 encoding
- [ ] Hardware H.265 encoding
- [ ] Hardware AV1 encoding where available
- [ ] Software fallback on unsupported hardware
- [ ] Capability reporting to UI

**Group status: 0 / 7**

## Group 12 — Platform packaging and release

- [ ] Windows build and installer
- [ ] macOS build and app bundle
- [ ] Linux build and package
- [ ] Versioned release notes
- [ ] Reproducible build process
- [ ] Update delivery mechanism `(optional)`

**Group status: 0 / 6**

---

## Milestones

### Milestone 1 — First usable transcoder
Required groups: 0, 1, 2, 4, 5

### Milestone 2 — CLI and batch workflows
Required groups: 3, 10

### Milestone 3 — Editing and fidelity parity
Required groups: 6, 7, 8, 9

### Milestone 4 — Performance and platform parity
Required groups: 11, 12

---

## Notes

- Items may be refined as implementation progresses.
- Features marked `(optional)` are not required for HandBrake parity but are useful for Vise.
- A group should not be marked complete until every checkbox is checked.