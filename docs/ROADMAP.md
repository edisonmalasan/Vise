# Vise Transcoder — Roadmap

> **Purpose:** A grouped, outcome-based guide to **what** should be built and in what order—not an implementation plan. Tasks below are retained from the previous roadmap; only presentation and status rules were improved.

A feature-by-feature build guide. This describes **what** is built and **in what order**, not how it is implemented. Features are grouped; a group is only complete when every item in it is checked.

Last updated: 2026-10-17  
Target parity: reference transcoder architecture at version 1.11.2 and its user-facing capability set.

> **Scope expansion (2026-10-16):** Groups 13–20 were added after a reverse-engineering
> capture of the shipped reference build (see `re/docs/17-rea-shipped-binary-capture.md`
> through `re/docs/22-rea-feature-parity-matrix.md`). The clone goal is **feature parity
> with a deliberately better interface**, not a minimal subset.

## Status Rules

- `[ ]` = not yet verified as complete. `[x]` = implemented **and verified**.
- **Not started:** no required items verified. **In progress:** some required items pending. **Complete:** all required items verified.
- **Deferred:** intentionally excluded from the current milestone; move it to the Deferred section instead of counting it as done.
- The agent must check actual implementation/tests before marking an item complete; if evidence is missing, leave it unchecked and note the uncertainty.
- Each checkbox should describe an **observable, testable outcome**, not a class name, code snippet, or implementation technique.
- Follow approved priorities; do not assume the next unchecked item is automatically authorized work.

## How progress is tracked

- Use `[ ]` for not started, `[x]` for complete.
- A group is **done** only when all of its items are checked.
- A milestone is reached only when every required group for that milestone is done.
- Optional capabilities are marked with `(optional)`.

## Architecture decision snapshot

- Clean-room implementation: new code, not a fork.
- Core: Rust library with a stable API boundary.
- First desktop frontend: Tauri + web UI.
- Media processing parity with the target 1.11.2 reference architecture is the long-term goal.

## Implementation Notes

- UI-facing work, especially in Group 4 and related preset/settings flows, should use the available design skills: `apple-design` for interaction motion, gestures, sheets, momentum, typography, and reduced-motion behavior; `impeccable` for UI structure, forms, onboarding, empty/error states, accessibility, and polish.
- These skills are implementation aids, not product milestones or roadmap checkboxes.

---

## Group 0 — Project foundation

*Outcome: Core crate, shared data models, session API, and geometry behavior are defined and reusable by later milestones.*

- [x] Rust core crate workspace established
- [x] Canonical job specification schema defined
- [x] Core session/job API defined
- [x] Media buffer and packet model defined
- [x] Progress/state model defined
- [x] Geometry and aspect-ratio behavior defined

**Status:** Complete (6/6 verified)

---

## Group 1 — Media ingestion

*Outcome: Supported input media can be scanned and described for title, chapter, track, and preview data.*

- [x] File source scanning
- [x] Media metadata extraction
- [x] Title discovery and selection
- [x] Chapter discovery
- [x] Audio track enumeration
- [x] Subtitle track enumeration
- [x] Preview frame extraction
- [x] Crop detection
- [ ] DVD source support `(optional)` — deferred; see Deferred section
- [ ] Blu-ray source support `(optional)` — deferred; see Deferred section

**Status:** In progress (8/10 verified; optional DVD/Blu-ray support deferred)

---

## Group 2 — Core transcode baseline

*Outcome: A single input can be transcoded end-to-end with common baseline codecs and output containers.*

- [x] Common video decoding
- [x] Common audio decoding
- [x] H.264 software encoding
- [x] AAC audio encoding
- [x] MP4 muxing
- [x] MKV muxing
- [x] WebM muxing
- [x] End-to-end single-file transcode

**Status:** Complete (8/8 verified)

---

## Group 3 — Command-line interface

*Outcome: Jobs can be run, queued, monitored, and cancelled from the CLI.*

- [x] CLI job execution
- [x] Source/title/output arguments
- [x] Preset loading from CLI
- [x] Batch queue execution
- [x] Progress output
- [x] Cancellation support

**Status:** Complete (6/6 verified)

---

## Group 4 — Desktop application shell

*Outcome: Desktop users can select sources, configure jobs, queue them, and monitor encoding.*

- [x] Source selection
- [x] Scan and title display
- [x] Title and track selection
- [x] Job queue
- [x] Encode progress display
- [x] Log display
- [x] Pause/resume/cancel controls

**Status:** Complete (7/7 verified)

---

## Group 5 — Presets and configuration

*Outcome: Built-in and user presets can be applied, edited, persisted, and exchanged.*

- [x] Built-in preset library
- [x] Preset application to jobs
- [x] User preset creation
- [x] User preset editing
- [x] User preset deletion
- [x] Settings persistence
- [x] Preset import/export

**Status:** Complete (7/7 verified)

---

## Group 6 — Video processing

*Outcome: Supported video filters and frame-rate/color behavior are available in the encode pipeline.*

- [x] Crop and scale
- [x] Deinterlacing
- [x] Comb detection
- [x] Detelecine
- [x] Denoise
- [x] Sharpen
- [x] Rotate
- [x] Pad
- [x] Color space conversion
- [x] Grayscale
- [x] Chroma smoothing
- [x] Constant frame rate handling
- [x] Variable frame rate handling
- [x] 10-bit processing
- [x] HDR metadata preservation

**Status:** Complete (15/15 verified)

---

## Group 7 — Audio pipeline

*Outcome: Audio tracks can be selected, mixed, resampled, encoded, or passed through.*

- [x] Multiple audio tracks
- [x] Audio mixdowns
- [x] Audio resampling
- [x] Audio dithering
- [x] Audio delay/gain
- [x] Audio passthrough
- [x] Vorbis encoding
- [x] Opus encoding
- [x] FLAC encoding
- [x] MP3 encoding
- [x] AC3/E-AC3 passthrough

**Status:** Complete (11/11 verified)

---

## Group 8 — Subtitles

*Outcome: Text and bitmap subtitles can be passed through or burned in with common format support.*

- [x] Text subtitle passthrough
- [x] Bitmap subtitle passthrough
- [x] Subtitle burn-in
- [x] SRT to MP4 text conversion
- [x] SSA/ASS handling
- [x] Closed caption handling
- [x] Foreign audio scan

**Status:** Complete (7/7 verified)

---

## Group 9 — Chapters and metadata

*Outcome: Chapters and media metadata can be copied or edited through the transcode workflow.*

- [x] Chapter copy
- [x] Chapter editing
- [x] Metadata copy
- [x] Metadata editing
- [x] Cover art handling
- [x] Attachment handling

**Status:** Complete (6/6 verified)

---

## Group 10 — Encoding control and job reliability

*Outcome: Encoding modes, retries, queue persistence, and safe cancellation are reliable.*

- [x] Constant quality encoding
- [x] Average bitrate encoding
- [x] Target file size mode
- [x] Two-pass encoding
- [x] Lossless encoding where supported
- [x] Job retry and failure reporting
- [x] Queue persistence across app restarts
- [x] Cancel behavior without corrupting output
- [ ] Job validation before pipeline start (mux, codec, filter and audio-layout compatibility)
- [ ] Engine crash isolation (a worker failure fails only its job)

**Status:** In progress (8/10 verified; validation and crash isolation pending)

---

## Group 11 — Hardware acceleration

*Outcome: Hardware decode/encode paths are discovered and used with software fallback where needed.*

- [x] Hardware decode detection
- [x] Hardware video decoding
- [x] Hardware H.264 encoding
- [x] Hardware H.265 encoding
- [x] Hardware AV1 encoding where available
- [x] Software fallback on unsupported hardware
- [x] Capability reporting to UI

**Status:** Complete (7/7 verified)

---

## Group 12 - Platform packaging and release

*Outcome: Installable platform builds, release notes, and reproducible releases are available.*

- [ ] Windows build and installer
- [ ] macOS build and app bundle
- [ ] Linux build and package
- [ ] Bundle FFmpeg and ffprobe with release builds
- [ ] Prefer bundled media tooling before PATH discovery
- [x] Versioned release notes
- [x] Reproducible build process
- [x] Update delivery mechanism `(optional)`

**Status:** In progress (3/8 verified; platform installers and bundled media tooling pending)

---

## Group 13 — Worker process and JSON IPC

*Outcome: Encoding and scanning run in a background worker exposed over a loopback, token-authenticated JSON API, independent of the UI process.*

- [x] Worker process lifecycle (start, handshake, monitor parent, shutdown)
- [x] Loopback HTTP listener with token authentication
- [x] JSON init contract
- [x] JSON scan contract (start, poll progress, titles, main title)
- [x] JSON encode contract (start, pause, resume, stop, poll progress)
- [x] JSON preview contract
- [x] JSON logging contract (all messages, messages from index, reset)
- [x] Worker crash detection and automatic restart
- [x] Portable-mode disable switch for process isolation

**Status:** Complete (9/9 verified; `crates/vise-worker` serves the API, `crates/vise-ipc` provides the client, supervisor, and portable-mode switch, and the desktop shell drives the worker instead of the pipeline)

## Group 14 — Desktop parity with reference product

*Outcome: The desktop application exposes every reference surface with an improved, consistent interface.*

- [ ] Summary tab parity
- [ ] Dimensions tab parity (crop modes, scale, anamorphic, resolution limits, flip/rotate)
- [ ] Filters tab parity (all filters, presets, tunes)
- [ ] Video tab parity (encoder, rate control, quality, profile, tune, level, colour range, framerate mode)
- [ ] Audio tab parity (per-track behaviour, defaults, mixdown, DRC, dither, delay, naming)
- [ ] Subtitles tab parity (behaviour, burn-in behaviour, import, search)
- [ ] Chapters tab parity
- [ ] Metadata tab parity
- [ ] Queue window and queue selection window parity
- [ ] Activity/log window parity
- [ ] Static preview window parity
- [x] Source selection control parity (source menu, scan modes)
- [x] Point-to-point range and angle selection parity
- [ ] Error view, copy-error, countdown alert
- [ ] About view
- [ ] Preset add/manage views parity
- [ ] Right-to-left layout support
- [ ] Theme selection (system/light/dark)

**Status:** In progress (2/18 verified; `apps/vise-desktop` drives the worker from a validated file or folder picker with drag-and-drop and recent sources, shows scan progress without blocking the window, lists the scanned titles with their facts, and resolves range, angle, and track selection in the window so what the page renders is covered by unit tests. Angle selection is wired to the contract's `angle_count`, which this build never reports above one until optical-disc sources arrive, so the control stays hidden until then.)

## Group 15 — Preferences surface

*Outcome: Users can configure the application through the full preferences surface.*

- [ ] General preferences tab
- [ ] Output preferences tab (folders, overwrite, autonaming collision)
- [ ] Advanced preferences tab (process isolation, log level, priority)
- [ ] Hardware preferences tab (encoder availability, decode device)
- [ ] Updates preferences tab (check frequency, channel)
- [ ] When-done preferences tab
- [ ] Reset-to-defaults flow
- [ ] Settings persistence and migration

**Status:** Not started (0/8 verified)

## Group 16 — Preset catalog and versioning

*Outcome: Built-in and user presets match the reference catalog and support versioning, migration, and exchange.*

- [ ] Built-in preset catalog with all six categories (General, Web, Devices, Matroska, Hardware, Professional)
- [ ] General quality families (Very Fast / Fast / HQ / Super HQ at 480p30, 576p25, 720p30, 1080p30, 2160p60 4K, HEVC and AV1 variants)
- [ ] Web presets (Creator 720p60-2160p60, Social 10 MB / 25 MB with duration caps)
- [ ] Device presets (Amazon Fire, Android, Apple, Chromecast, Playstation, Roku, Xbox)
- [ ] Matroska presets (H.264, H.265, VP9, AV1)
- [ ] Hardware presets (QSV, NVENC, VCN, Media Foundation, VideoToolbox)
- [ ] Professional presets (ProRes, DNxHR, FFV1 preservation)
- [ ] Preset schema with explicit version triple and migration
- [ ] Preset clean/normalize operation
- [ ] Preset import/export round-trip
- [ ] Preset clone and reorder operations
- [ ] Hardware-dependent preset availability resolution with software fallback
- [ ] Preset descriptions with device compatibility guidance

**Status:** Not started (0/13 verified)

## Group 17 — System integration

*Outcome: The application integrates with the desktop the way the reference product does.*

- [ ] System tray icon and menus
- [ ] Windows toast notifications on completion/failure
- [ ] Power management (prevent sleep during encode)
- [ ] Pre/post encode actions
- [ ] Automatic update checking with appcast support
- [ ] Portable mode via portable.ini
- [ ] Queue persistence and recovery after restart
- [ ] Disk-space-aware queue pause behavior
- [ ] Process priority and NUMA awareness

**Status:** Not started (0/9 verified)

## Group 18 — Localization

*Outcome: The user interface is available in the reference language set and switches at runtime.*

- [ ] UI language selection
- [ ] Runtime language switching
- [ ] Translation resource pipeline
- [ ] Right-to-left locale handling
- [ ] Localized preset descriptions
- [ ] Locale fallback behavior

**Status:** Not started (0/6 verified)

## Group 19 — Media depth

*Outcome: Media handling reaches the reference product's coverage in the areas still missing.*

- [ ] DVD source support `(optional)`
- [ ] Blu-ray source support `(optional)`
- [ ] Deblock filter
- [ ] Lossless FFV1/PCM/FLAC MKV modes
- [ ] Dolby Vision profile and RPU handling
- [ ] HDR10+ dynamic metadata
- [ ] MP4 behaviors (web optimized, iPod 5G, common metadata passthrough)
- [ ] Chapter import from CSV, TXT, and XML
- [ ] Cover art and attachment handling with container restrictions
- [ ] Foreign audio scan parity

**Status:** Not started (0/10 verified)

## Group 20 — Diagnostics and developer tools

*Outcome: Support and debugging workflows match the reference product's operational tooling.*

- [ ] Per-encode log files and log rotation
- [ ] Activity log streaming to the UI
- [ ] Quality sweep tool for the queue
- [ ] Queue result data extraction tool
- [ ] Hardware capability report export
- [ ] Reproducible diagnostic bundle export

**Status:** Not started (0/6 verified)

## Deferred / Future Work

*List intentionally postponed outcomes, with their reason or target release when known. Do not mark them complete.*

- [ ] DVD source support `(optional)` — optional input capability; not required for core Milestone 1.
- [ ] Blu-ray source support `(optional)` — optional input capability; not required for core Milestone 1.
- [ ] Update delivery mechanism `(optional)` — optional release enhancement; not required for initial platform packaging.

## Milestones

### Milestone 1 - First usable transcoder
Required groups: 0, 1, 2, 4, 5

### Milestone 2 - CLI and batch workflows
Required groups: 3, 10

### Milestone 3 - Editing and fidelity parity
Required groups: 6, 7, 8, 9

### Milestone 4 - Performance and platform parity
Required groups: 11, 12

### Milestone 5 - Reference-product parity
Required groups: 13, 14, 15, 16

### Milestone 6 - Desktop polish and operations
Required groups: 17, 18, 20

### Milestone 7 - Media depth
Required groups: 19

## Status Summary

| Group | Milestone | Status |
|---|---|---|
| 0 | Project foundation | Complete (6/6) |
| 1 | Media ingestion | In progress (8/10) |
| 2 | Core transcode baseline | Complete (8/8) |
| 3 | Command-line interface | Complete (6/6) |
| 4 | Desktop application shell | Complete (7/7) — scaffold only; full parity in Group 14 |
| 5 | Presets and configuration | Complete (7/7) — catalog parity in Group 16 |
| 6 | Video processing | Complete (15/15) |
| 7 | Audio pipeline | Complete (11/11) |
| 8 | Subtitles | Complete (7/7) |
| 9 | Chapters and metadata | Complete (6/6) |
| 10 | Encoding control and job reliability | In progress (8/10) |
| 11 | Hardware acceleration | Complete (7/7) |
| 12 | Platform packaging and release | In progress (3/8) |
| 13 | Worker process and JSON IPC | Complete (9/9) |
| 14 | Desktop parity with reference product | In progress (2/18) |
| 15 | Preferences surface | Not started (0/8) |
| 16 | Preset catalog and versioning | Not started (0/13) |
| 17 | System integration | Not started (0/9) |
| 18 | Localization | Not started (0/6) |
| 19 | Media depth | Not started (0/10) |
| 20 | Diagnostics and developer tools | Not started (0/6) |

## Notes

- Items may be refined as implementation progresses.
- Features marked `(optional)` are not required for target parity but are useful for Vise.
- A group should not be marked complete until every checkbox is checked.
- Revisit this roadmap when scope or priorities change.
- Update statuses only after verification; distinguish untested from failed.
- Keep implementation details in code, code review, or a task-specific plan—not here.
- Preserve reasons for significant technical decisions in an existing authoritative project document, adding a dedicated one only if needed.
"