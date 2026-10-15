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

**Status:** Complete (8/8 verified)

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
| 5 | Presets and configuration | Complete (7/7) |
| 6 | Video processing | Complete (15/15) |
| 7 | Audio pipeline | Complete (11/11) |
| 8 | Subtitles | Complete (7/7) |
| 9 | Chapters and metadata | Complete (6/6) |
| 10 | Encoding control and job reliability | Complete (8/8) |
| 11 | Hardware acceleration | Complete (7/7) |
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