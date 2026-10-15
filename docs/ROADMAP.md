# Vise Transcoder — Roadmap

> **Purpose:** A grouped, outcome-based guide to **what** should be built and in what order—not an implementation plan. Tasks below are retained from the previous roadmap; only presentation and status rules were improved.

A feature-by-feature build guide. This describes **what** is built and **in what order**, not how it is implemented. Features are grouped; a group is only complete when every item in it is checked.

Last updated: 2026-10-16  
Target parity: reference transcoder architecture at version 1.11.2 and its user-facing capability set.

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

- [ ] Source selection
- [ ] Scan and title display
- [ ] Title and track selection
- [ ] Job queue
- [ ] Encode progress display
- [ ] Log display
- [ ] Pause/resume/cancel controls

**Status:** Not started (0/7 verified)

---

## Group 5 — Presets and configuration

*Outcome: Built-in and user presets can be applied, edited, persisted, and exchanged.*

- [ ] Built-in preset library
- [ ] Preset application to jobs
- [ ] User preset creation
- [ ] User preset editing
- [ ] User preset deletion
- [ ] Settings persistence
- [ ] Preset import/export

**Status:** Not started (0/7 verified)

---

## Group 6 — Video processing

*Outcome: Supported video filters and frame-rate/color behavior are available in the encode pipeline.*

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

**Status:** Not started (0/15 verified)

---

## Group 7 — Audio pipeline

*Outcome: Audio tracks can be selected, mixed, resampled, encoded, or passed through.*

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

**Status:** Not started (0/11 verified)

---

## Group 8 — Subtitles

*Outcome: Text and bitmap subtitles can be passed through or burned in with common format support.*

- [ ] Text subtitle passthrough
- [ ] Bitmap subtitle passthrough
- [ ] Subtitle burn-in
- [ ] SRT to MP4 text conversion
- [ ] SSA/ASS handling
- [ ] Closed caption handling
- [ ] Foreign audio scan

**Status:** Not started (0/7 verified)

---

## Group 9 — Chapters and metadata

*Outcome: Chapters and media metadata can be copied or edited through the transcode workflow.*

- [ ] Chapter copy
- [ ] Chapter editing
- [ ] Metadata copy
- [ ] Metadata editing
- [ ] Cover art handling
- [ ] Attachment handling

**Status:** Not started (0/6 verified)

---

## Group 10 — Encoding control and job reliability

*Outcome: Encoding modes, retries, queue persistence, and safe cancellation are reliable.*

- [ ] Constant quality encoding
- [ ] Average bitrate encoding
- [ ] Target file size mode
- [ ] Two-pass encoding
- [ ] Lossless encoding where supported
- [ ] Job retry and failure reporting
- [ ] Queue persistence across app restarts
- [ ] Cancel behavior without corrupting output

**Status:** Not started (0/8 verified)

---

## Group 11 — Hardware acceleration

*Outcome: Hardware decode/encode paths are discovered and used with software fallback where needed.*

- [ ] Hardware decode detection
- [ ] Hardware video decoding
- [ ] Hardware H.264 encoding
- [ ] Hardware H.265 encoding
- [ ] Hardware AV1 encoding where available
- [ ] Software fallback on unsupported hardware
- [ ] Capability reporting to UI

**Status:** Not started (0/7 verified)

---

## Group 12 — Platform packaging and release

*Outcome: Installable platform builds, release notes, and reproducible releases are available.*

- [ ] Windows build and installer
- [ ] macOS build and app bundle
- [ ] Linux build and package
- [ ] Versioned release notes
- [ ] Reproducible build process
- [ ] Update delivery mechanism `(optional)`

**Status:** Not started (0/6 verified)

---

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

## Status Summary

| Group | Milestone | Status |
|---|---|---|
| 0 | Project foundation | Complete (6/6) |
| 1 | Media ingestion | In progress (8/10) |
| 2 | Core transcode baseline | Complete (8/8) |
| 3 | Command-line interface | Complete (6/6) |
| 4 | Desktop application shell | Not started (0/7) |
| 5 | Presets and configuration | Not started (0/7) |
| 6 | Video processing | Not started (0/15) |
| 7 | Audio pipeline | Not started (0/11) |
| 8 | Subtitles | Not started (0/7) |
| 9 | Chapters and metadata | Not started (0/6) |
| 10 | Encoding control and job reliability | Not started (0/8) |
| 11 | Hardware acceleration | Not started (0/7) |
| 12 | Platform packaging and release | Not started (0/6) |

## Notes

- Items may be refined as implementation progresses.
- Features marked `(optional)` are not required for target parity but are useful for Vise.
- A group should not be marked complete until every checkbox is checked.
- Revisit this roadmap when scope or priorities change.
- Update statuses only after verification; distinguish untested from failed.
- Keep implementation details in code, code review, or a task-specific plan—not here.
- Preserve reasons for significant technical decisions in an existing authoritative project document, adding a dedicated one only if needed.
"