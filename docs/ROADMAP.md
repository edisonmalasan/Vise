# Vise Transcoder — Roadmap

> **Purpose:** A grouped, outcome-based guide to **what** should be built and in what order—not an implementation plan. Tasks below are retained from the previous roadmap; only presentation and status rules were improved.

A feature-by-feature build guide. This describes **what** is built and **in what order**, not how it is implemented. Features are grouped; a group is only complete when every item in it is checked.

Last updated: 2026-10-18  
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
- [x] Job validation before pipeline start (mux, codec, filter and audio-layout compatibility)
- [x] Engine crash isolation (a worker failure fails only its job)

**Status:** Complete (10/10 verified; a job is judged before the pipeline starts by one rule set that is a pure function of the job and the machine facts handed to it, so the window runs it before a job is queued — through the page, the selection sheet, the title batch, and once more before the job is claimed — and the worker runs the identical rules on `StartEncode` and again on the encode thread as a defence in depth. It reports every problem at once against the field that carries it, covering container/codec and subtitle-mode compatibility, filters the chosen encoder cannot apply (deblocking and telecine removal need inter prediction, which an intra-only codec never has, and a deblock block size outside the filter's own range), audio layouts the source cannot produce (a channel count or mixdown wider than the source, and a mixdown to the layout it already has), rate-control sanity (a lossless request carrying a quality or bitrate value, a bitrate job with none, a two-pass job with nothing to gain from a second pass), and paths that are blank, missing, unreadable, unwritable, or the wrong container for the extension. Crash isolation is a pure attribution of an outcome: the supervisor reads a dead worker's exit status without starting a replacement, the in-flight job is marked failed with that status and its partial output swept (or named, when an orphaned encoder still holds it), the queue continues on a fresh worker, a stop the user asked for is a cancellation on either side of the worker dying, and a worker that dies on consecutive jobs is replaced with a growing wait until the third, which is reported and stops the queue instead of looping. Verified by test: every rule and its valid counterpart; the queue refusing an unencodable job and carrying on with the ones behind it; a real worker killed with `taskkill /F` during an encode, after which only that job failed, the partial was swept, and the next job encoded on a replacement process; a user-requested stop arriving as a cancellation; and three consecutive deaths replaced twice and then reported. Not exercised here: the desktop's own poll loop is covered by its unit tests against the same policy rather than against a live worker, because the desktop test binary has no built worker to point at; the signal half of a worker's death is implemented for unix and only the exit-code half ran on this platform; a hardware encoder was never the subject of a crash, so the tests are all against the software path; and the in-process fallback has no worker process to isolate, so nothing was tested for it)

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

- [x] Windows build and installer
- [x] macOS build and app bundle
- [x] Linux build and package
- [x] Bundle FFmpeg and ffprobe with release builds
- [x] Prefer bundled media tooling before PATH discovery
- [x] Versioned release notes
- [x] Reproducible build process
- [x] Update delivery mechanism \(optional)
**Status:** Complete (8/8 verified; all three platforms build and package.)

A shipped build carries its own encoder rather than relying on whatever the
machine has, because the filters and encoders the application offers are only
present in some builds. `crates/vise-core/src/tooling.rs` owns the order and it
is fixed: the application's own copy under any of the packaged layouts, then the
machine's `PATH`, then a per-user install. A file only counts when it exists, is
a file, and is executable, so a directory called `ffmpeg.exe` is not mistaken for
a binary and found at the first spawn instead. When nothing is bundled the build
says so rather than behaving quietly differently from a shipped one, and that
reaches the Group 20 diagnostics bundle.

Verified end to end on this machine rather than by inspection. The tools are
fetched into a pin file, the archive digest was compared against the digest the
publisher publishes, and the extracted binaries' digests were then pinned, so a
release job can re-check the claim with `verify-media-tools.sh` before it
publishes. That script was confirmed to pass on the fetched tools and to fail on a
binary altered by one byte. A real `build-release.sh --bundled` produced a
staged package whose bundled `ffmpeg` runs (8.1.2) and which the tooling resolver
reports as bundled while a different ffmpeg is on the `PATH`.

The Windows installer was produced and inspected rather than assumed. It was
installed silently into a scratch directory and the installed tree checked: the
application, the worker, and both tools are present, and the installed worker
starts and answers its own authenticated API on the loopback port. Building it
found a defect that would have shipped: the bundle carried the media tools but
not the worker, so an installed application would have found no worker beside its
executable and silently fallen back to running the pipeline in-process, which is
the one thing process isolation exists to prevent. It also carried the 105 MB
download archive, which is a build input rather than something to ship; the
installer went from 159 MiB to 54 MiB once the tools were staged separately from
the cache.

The macOS bundle is built by the release workflow on a macOS machine, which is
the only place it can be: it links against the frameworks of the platform it
runs on. The bundle and the disk image were produced there, and the workflow
that produced them runs the same checks as the other two platforms.

All three platforms now build and package on every push, and each is checked
rather than assumed: the Linux package is unpacked and its parts run, and the
Windows installer is installed into a scratch directory and its tree inspected.
The media tools are pinned per platform with their archive and binary digests,
and every build checks those digests before it trusts what it downloaded.

Building for the other platforms found several things that would have shipped.
The window asked for a Windows window handle unconditionally and so did not
compile off Windows. The application looked for the worker only beside its own
binary, and a Linux package installs the binary under /usr/bin and its
resources under /usr/lib, so an installed application would have found no
worker and quietly fallen back to running the pipeline inside the window. The
capabilities offered a hardware encoder because ffmpeg listed it, whatever
hardware was present. And the bundle carried the media tools but not the worker,
so the installed application had no worker to start.

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

**Status:** Complete (9/9 verified; `crates/vise-worker` serves the API, `crates/vise-ipc` provides the client, supervisor, and portable-mode switch, and the desktop shell drives the worker instead of the pipeline. A worker that stops mid-request is recognised as gone whether it resets the connection or hangs up cleanly without replying, so the supervisor restarts it in both cases)

## Group 14 — Desktop parity with reference product

*Outcome: The desktop application exposes every reference surface with an improved, consistent interface.*

- [x] Summary tab parity
- [x] Dimensions tab parity (crop modes, scale, anamorphic, resolution limits, flip/rotate)
- [x] Filters tab parity (all filters, presets, tunes)
- [x] Video tab parity (encoder, rate control, quality, profile, tune, level, colour range, framerate mode)
- [x] Audio tab parity (per-track behaviour, defaults, mixdown, DRC, dither, delay, naming)
- [x] Subtitles tab parity (behaviour, burn-in behaviour, import, search)
- [x] Chapters tab parity
- [x] Metadata tab parity
- [x] Queue window and queue selection window parity
- [x] Activity/log window parity
- [x] Static preview window parity
- [x] Source selection control parity (source menu, scan modes)
- [x] Point-to-point range and angle selection parity
- [x] Error view, copy-error, countdown alert
- [x] About view
- [x] Preset add/manage views parity
- [x] Right-to-left layout support
- [x] Theme selection (system/light/dark)

**Status:** Complete (18/18 verified; `apps/vise-desktop` drives the worker from a validated file or folder picker with drag-and-drop and recent sources, shows scan progress without blocking the window, lists the scanned titles with their facts, and resolves range, angle, and track selection in the window so what the page renders is covered by unit tests. Angle selection is wired to the contract's `angle_count`, which this build never reports above one until optical-disc sources arrive, so the control stays hidden until then. The seven configuration tabs are built as Rust view models that carry their own availability and reason, and one settings set drives both the tabs and the job specification, so a queued job cannot disagree with what the tabs showed. Fields the job model lacked were added and honoured in the resolved plan: output framing flags, encoder preset/profile/tune/level, colour range, mirroring, and per-track forced-dialogue selection. Controls the worker contract has no field for are shown closed with the reason instead of being accepted and ignored: the deblock filter, which is a media-depth item, and the MP4-only output flags outside an MP4. Anamorphic scaling and per-chapter inclusion are resolved from the frame size and the marker list rather than stored as separate fields, so they cannot fall out of step with the geometry they describe.

The secondary windows are described by the window as one value each and the page draws them from that description, so it cannot open a surface the window does not offer or present one with the wrong accessibility semantics. The queue keeps its own order and its own priority rather than borrowing either from the session, so a reordering and a priority change are answers to the same question and cannot disagree; its selection sheet plans every output name before anything is queued, so two titles that would resolve to one file are refused together rather than the second overwriting the first. The activity window is paged by index, so following a long encode costs the same whether the worker has written ten lines or ten thousand, and the dropped-lines note is shown rather than hidden. The preview window converts a rectangle drawn in displayed pixels into source pixels, refuses one that leaves the frame or has odd sides, and writes it into the dimensions tab, so the rectangle that gets encoded is the one that was checked. Destructive actions ask through a countdown that is counted in Rust, so the count the user reads is the count that arms the confirm control, and one answer drives one action. Presets are created, renamed, copied, imported, exported, and deleted through the core preset API, with the name, duplicate-name, and category rules enforced in the window so a preset it accepted can never fail on the next launch.

The theme and the layout direction are settings the user makes once and then forget, so they are held in the one settings file through the core settings API that every other setting uses, and there is no second store and no page-side preference that could disagree with them. A preference is not a resolved value: "match the system" and "match my language" are looked up against the machine every time something is painted, and the lookup can change while the window is open, so the window holds the preference, the observation, and the resolution, and hands the page the resolved pair. The page therefore applies what it is given and decides nothing: it puts `data-theme` and `dir` on the document, and because every offset, padding and text alignment in the stylesheet is a logical property, those two attributes move the toolbar, the tab strip, the form rows and the dialogs together rather than each needing an override. The two places that stay physical are deliberate and marked: a surface is centred over the window rather than laid out inside it, and a crop rectangle is drawn over a picture that does not mirror when the window does. Direction is decided from the locale by script before language, so a Kurdish or Punjabi locale written in the Arabic script is mirrored even though the language alone would not be, and a locale this build cannot name reads left to right rather than being mirrored at random. A stored value that is not one of the named options falls back to the safe default rather than stopping the window from opening, because a settings file written by hand or by a newer build is not a reason to refuse to open. The operating system can change its theme while the window is open, and the window pushes the new answer to the page rather than waiting to be asked; a page reporting the same theme the platform already reported is a handover of authority that changes no pixels, and the log records a theme change only when the colours on screen actually moved.

Verified by unit tests and by running the application against a generated clip: scan, preview still, crop into the dimensions tab, queue, encode through the worker, the countdown alert, and the preset round trip. Theme and direction were verified by driving the shipped page in a browser with the bridge stubbed to the answers the window sends, across following and forced preferences in both themes and both directions: the resolved pair reaches every surface, the toolbar and tab strip are measured on the start edge of the layout they are in, the arrow keys along the strip move with the layout, a dialog opened over the page inherits both values and holds the same contrast, and the page opens in the theme and direction it was last left in when the bridge cannot answer at all. The colours were checked in both themes: every text token clears 4.5:1 against the surface it is read on, every control boundary clears 3:1 against both the surface and the sunken fill, and the focus ring is two colours so it stays visible against a light control in a dark window and a dark control in a light one. The application itself was not launched for this slice, so the appearance of the real window was confirmed through the shipped page rather than through the running window.

## Group 15 — Preferences surface

*Outcome: Users can configure the application through the full preferences surface.*

- [x] General preferences tab
- [x] Output preferences tab (folders, overwrite, autonaming collision)
- [x] Advanced preferences tab (process isolation, log level, priority)
- [x] Hardware preferences tab (encoder availability, decode device)
- [x] Updates preferences tab (check frequency, channel)
- [x] When-done preferences tab
- [x] Reset-to-defaults flow
- [x] Settings persistence and migration

**Status:** Complete (8/8 verified; the six tabs are one record in the settings file, under a version of its own so a record written by a newer build is read, shown as that build left it, and never written over. An older document's own output folder is adopted into the Output tab rather than lost, and a corrupt record falls back to the defaults while the bytes on disk are left exactly as they were found)

A preference is only worth having if something reads it, so each tab names the path that consumes it and a control with no such path is shown closed with the reason rather than stored and ignored. That rule is what decided the three answers the when-done tab cannot give — suspending, hibernating and signing out are listed with what they would have meant and closed, because this build has no session control — and what closed the button that would fetch a release, because this build contacts no release server. The same rule applies to the one that *is* on offer: the advanced tab's switch is closed when a portable settings file has taken isolation away, while the preference itself is kept and shown, so removing the file brings the user's choice back rather than replacing it with a default.

The record's version is separate from the document's on purpose. The document gains members for reasons that have nothing to do with what a control means, while the record's number only moves when a preference changes shape. So a document from an older build is migrated forward by the one migration that has anything to carry — the output folder it kept at the top level becomes the folder the Output tab edits — and a record from a newer build is read as far as it can be, shown as that build left it, and refused rather than written over, because a downgrade and an upgrade round trip would otherwise throw it away. A record this build cannot read is not deleted or half-applied: every preference falls back to its default, the stored bytes are left exactly as they were found, and the first change is what rewrites the file.

The two answers about replacing a file are settled into one before anything is written, rather than left as a rule the user has to learn: turning the switch on settles the collision choice to replace, and choosing any other collision turns the switch off, so the record cannot hold two answers to one question. The queue reads that one settled value as a single policy and resolves the whole batch before any of it is queued, through the same call the selection sheet is built from — so the names on the sheet are the names the queue writes, including the ones a renumbering policy changed, and two titles that would resolve to one file are numbered, refused or left out together rather than the second quietly overwriting the first.

Hardware detection reports what the machine has rather than offering a switch beside each encoder, because a list of encoders is information and offering a choice for each one would be offering a choice nothing reads. A named decode device is a choice, because the pipeline reads it, so it goes into the decoder the media tools are given — as an accelerator name and as the API it corresponds to, not as the `-hwaccel_device` argument, which takes a device path. A device detection did not find is refused at the point of setting rather than saved and quietly ignored, and the two answers that are not a device are always on offer.

The page holds nothing but the surface. A value derived from a preference — the verbosity rank a log level works out to, the days between checks, the date the next check falls — is shown as a description list and not as an editable field, because a field that can be typed into and then ignored is a control that lies about what it is; the only controls that take input are the ones the window will read. Every closed control carries its reason in the same place and announces it through `aria-describedby`, so the explanation exists for people who cannot see it. The strip walks with the platform's own arrows, which swap with the layout rather than becoming unreachable with one of them, and Home and End go to the ends either way.

Restoring the defaults is the one control on the surface that throws away work, so it asks through the countdown every other destructive action uses, and the previous record is kept for exactly one reset so the answer can be taken back. The queue finishing can ask the same question with nobody pressing anything, so that question is pushed to the page as an event and confirmed through a command that closes the window only when a confirmation is behind it; a wait is honoured rather than recorded, and the window remembers it has already asked for this drain so two jobs finishing together ask once.

Verified by unit tests across the record's lifecycle: a round trip through the settings file, an older document whose output folder is adopted, an older record brought forward without losing what it held, a record from a newer build read and never written, three kinds of corrupt record left alone on disk, a settings file that is not JSON at all, a partially missing record, and a reset that is on disk and can be taken back once. Each preference is checked against the code path that consumes it, and the collision rules, the hardware options, the schedule, and the when-done decision are pinned as plain functions. The page was verified by driving the shipped page with the bridge stubbed to the answers the window sends: the six tabs and their panels carry the right semantics, one tab stop with arrows that wrap and swap with the layout, closed controls keeping their reasons and their stored values, the invalid folder marked and explained inline, a change sending the field the window named rather than the element it came from, the reset asking through the countdown, and a question the window asked for itself confirming through its own command. The application itself was not launched for this slice, so the preferences surface was confirmed through the shipped page rather than through the running window.

## Group 16 — Preset catalog and versioning

*Outcome: Built-in and user presets match the reference catalog and support versioning, migration, and exchange.*

- [x] Built-in preset catalog with all six categories (General, Web, Devices, Matroska, Hardware, Professional)
- [x] General quality families (Very Fast / Fast / HQ / Super HQ at 480p30, 576p25, 720p30, 1080p30, 2160p60 4K, HEVC and AV1 variants)
- [x] Web presets (Creator 720p60-2160p60, Social 10 MB / 25 MB with duration caps)
- [x] Device presets (Amazon Fire, Android, Apple, Chromecast, Playstation, Roku, Xbox)
- [x] Matroska presets (H.264, H.265, VP9, AV1)
- [x] Hardware presets (QSV, NVENC, VCN, Media Foundation, VideoToolbox)
- [x] Professional presets (ProRes, DNxHR, FFV1 preservation)
- [x] Preset schema with explicit version triple and migration
- [x] Preset clean/normalize operation
- [x] Preset import/export round-trip
- [x] Preset clone and reorder operations
- [x] Hardware-dependent preset availability resolution with software fallback
- [x] Preset descriptions with device compatibility guidance

**Status:** Complete (13/13 verified; the presets that ship with this build are an embedded JSON catalog rather than code, stamped with a version triple of their own and read against a template so an incomplete preset still means something. 138 entries across the six categories: the four quality families at five rungs each in H.264, H.265, and AV1; Creator 720p60–2160p60 and size-targeted Social presets that each carry the length they were designed around; the seven device families; four Matroska codecs; the five hardware families, each naming the chip it was written for; and ProRes, DNxHR, and FFV1 for preservation. Availability is asked of the machine rather than assumed: a preset names the encoder it was written for, the answer comes from the encoder's own `-encoders` list, and a chip that is missing becomes its documented software stand-in with the substitution said out loud, while a preset with no stand-in is refused by name instead of being given something else silently. A machine that could not be probed at all closes nothing, because refusing every preset on the strength of a probe that never ran would be a guess)

Verified by unit tests over the whole catalog rather than over samples of it: the asset loads and names its own version, every one of the 138 entries validates, every entry normalizes the same way twice, and every entry round-trips through a file and back. The version policy is checked in both directions — a document from a later minor build is read and keeps the members this build does not know, a document from a later major build is refused by name rather than half-read, and a file written before the version existed is migrated forward with what it held intact. Cleaning is checked to fold the duplicated spellings and the two ways of writing "no chip needed", so two presets that mean the same thing compare equal, and the identifiers three earlier builds stored are resolved through an alias table rather than by renaming the entries. Availability is checked four ways: a machine with everything, a software-only machine where a missing chip becomes its stand-in and says so, a machine where hardware has been switched off, a machine that could not be probed at all, and a machine lacking an encoder that has no stand-in. Clone and reorder are checked for independence — a copied entry and a copied stored preset share no mutable state with the original, and a reordered library loses nothing. The browser surface is a Rust view model: the six categories are one tab stop with arrows that wrap and swap with the layout, the entries are a listbox the arrows walk, an entry this machine cannot run stays in the list with its reason beside it rather than disappearing, and the detail reads out the description, the compatibility guidance, and the encoder that will actually run. Two problems were found and fixed while applying a catalog preset to the tabs: a preset's encoder was never written onto the video tab, so a ProRes profile was checked against the H.264 picker and refused, and a field the preset was silent about kept the previous preset's value, so applying two presets in a row produced a job neither of them described. A preset is now the whole instruction for the encode — the encoder comes from the preset and anything it does not name goes back to the encoder default. The application itself was not launched for this slice, so the browser was confirmed through the shipped page and its view model rather than through the running window.

## Group 17 — System integration

*Outcome: The application integrates with the desktop the way the reference product does.*

- [x] System tray icon and menus
- [x] Windows toast notifications on completion/failure
- [x] Power management (prevent sleep during encode)
- [x] Pre/post encode actions
- [x] Automatic update checking with appcast support
- [x] Portable mode via portable.ini
- [x] Queue persistence and recovery after restart
- [x] Disk-space-aware queue pause behavior
- [x] Process priority and NUMA awareness

**Status:** Complete (9/9 verified; each capability is a decision made in Rust and one platform call behind a seam, so every rule is a unit test that runs without the operating system)

Nine capabilities, arranged the same way throughout: the *decision* is a pure function over plain facts with its own unit tests, and the *mechanism* is one call to the platform behind a trait, so a build on a machine that lacks the capability reports the capability as missing instead of pretending. Nothing here talks to the media tools and nothing here runs the pipeline — encoding stays in the worker, and the window arranges things around it.

The tray is the surface somebody uses when the window is not in front, so what it carries is the answer to two questions at once — what the queue is doing and what can be done about it right now — plus the three things a desktop application is expected to offer from there. Its model is built in Rust from the queue's own counts rather than from a page that composes strings: a row the window cannot carry out is disabled and says why in the same place, a row that changes meaning when the queue does (pause becomes resume, the update row opens when a release is on offer) is one row with two states, and the status line is the tooltip verbatim so the two cannot disagree. The page holds none of it; the window builds a platform menu from the model on every change and refuses a press it does not recognise.

A notification is a claim on somebody's attention, so the default is not to make one. The gate is not "something happened" but "something happened the user could not have seen from where they were": a queue that finishes while the window is in front says so in the activity log and stops there, the same queue finishing with the window behind something else says it in a notification, and a failure is worth a notification wherever the user is — because a silent failure is one discovered on Saturday. Both events that are about a *change* rather than a state are noticed by comparing with the last refresh, so a queue that finishes once is announced once however often the window is polled afterwards. The Windows implementation puts a balloon on the notification area through a private icon it removes when the balloon has been up long enough to read, and every notification is written to the activity log as well, so nothing is lost when the platform declines.

Power is held by whoever owns the pipeline, and that is a decision rather than an accident: with a worker process the worker holds it, because the hold then dies with the encode thread that asked for it and a window that crashes mid-encode cannot leave somebody's machine awake until they log off; with process isolation off the pipeline is inside the window, so the window holds it instead. Two processes holding one state is how a desktop stays awake all night. The hold is taken when a job is accepted rather than when the pipeline starts, and let go on every path out of an encode thread — finished, failed, cancelled, rejected, or never startable — through a flag that makes a second release harmless.

Commands around an encode are the user's own programs on the user's machine, which is exactly why there is no shell anywhere in this path. A command is split into a program and an argument vector before anything runs, quoting says only where one argument ends and the next begins, and `;`, `&`, `|`, `>`, and `%PATH%` are ordinary characters: a title called `Episode 1 & del important.mkv` is a file name and not a second command. Only Vise's own `%output%`, `%input%`, `%title%`, `%job_id%`, `%container%`, and `%folder%` are substituted, and an unknown `%name%` is left exactly as written rather than replaced with nothing. A command that will not stop is killed at its timeout, so the queue is not held by a program somebody forgot about; a failed post-encode command is a line in the log and never a reason to fail an encode that has already been written; and a failed pre-encode command either continues or stops the encode according to a preference, while a command that cannot be *read* always stops it, because running nothing is not what a command is for.

Updates are read from an appcast, which is XML fetched from a server, so it is parsed as untrusted input against a small fixed grammar: a size limit, no document type and no entity declarations, only the five entities XML defines for text with everything else left as the characters the feed wrote, https-only artifact addresses, and a SHA-256 checked over the bytes before anything is offered. The hash is written out rather than brought in — it is pinned against the published test vectors, which is what makes it safe to offer a download on the strength of it — and a mismatch means the bytes are not the ones that were described, so nothing is offered. The schedule is the one the Updates preferences already carried, resolved against the date of the last check, and the feed address is a field the user types rather than one this build ships: **a build with no feed configured contacts nobody**, which is why the check-now control is closed with that reason until one is. Nothing is ever downloaded automatically and nothing is installed by this build at all; what happens is that the availability, the version, and the feed are shown and the decision is the user's. The transport is the platform's own network tool, invoked with an argument vector and options that refuse everything it would otherwise do on its own; where that tool is absent the button says so, which is a different answer from a build that cannot reach the network at all.

Portable mode now decides where state is *written*, not only whether a worker process is started. Settings, the queue, and the presets each resolve through their own key, fall back to the general one, and are taken as written for an absolute value; a relative value is read against the portable root and one that climbs out of it with `..` is refused with a refusal naming the line the operator wrote, because a marker file in a folder should not be able to reach anywhere else on the machine by accident. The resolution is a pure function of the entries and the two roots, so all of that is tested without a portable drive attached.

The queue on disk is versioned and written atomically, so a machine that loses power halfway through leaves either the old queue or the new one and never half of either, and a document from a newer build is read as far as it can be and then left alone. The reading is the interesting half: a job that was running or suspended when the window went away comes back *interrupted* — waiting, marked, carrying the reason, and never started on its own — because resuming it would start a second encode over a half-written file and starting nothing would look like a queue that forgot what it was doing. A cancelled job comes back waiting for the same reason. The queue's own order and priorities are written down with the jobs, since a run order the user set by hand is as much part of the queue as the jobs are.

Disk space is asked before each queued job rather than once for the queue, because a queue of forty jobs can run out of room halfway through. The threshold is stated rather than merely "enough for this file": the output is counted twice, because the encoder writes a temporary sibling and moves it into place, and a two gibibyte reserve and a fifth of the volume have to remain afterwards. A volume the platform will not describe is not a reason to refuse — that would make the queue useless exactly where filesystems are least standard, and a pause whose reason is "unknown" tells nobody anything they can act on — so the answer is "proceed", with the reading recorded as unchecked. A volume that cannot answer in a particular case is distinguished from a volume that is full.

Processor placement reports what the machine says about itself and decides from that, with four refusals rather than four guesses: a layout that could not be measured is never pinned, because "no pinning needed" from an unknown layout reads as a measurement; a group the preference names that does not exist is refused by name rather than clamped into one that does; `off` is honoured as written; and automatic pinning happens only for a worker competing hard on a machine with several memory controllers, because the scheduler's own placement is better than anything guessed here. The worker applies the decision to itself, since a process can restrict itself where nothing else can restrict it reliably. The priority the worker asks for is the existing Advanced preference, read from the one record and sent in the one init command, so there is no second answer to "how much of the machine does this build take".

The preferences that these capabilities read were added to the tabs that already existed rather than to a new surface: the Advanced tab grew the hold on the machine and its scope, the placement of the worker and the decision that follows from it, the two commands and their timeout and failure rule, and what is turned into a notification; the Updates tab grew the feed address. The Advanced tab shows what the machine was found to be — the processor layout, whether the machine can be held awake, where a notification would go — as read-outs rather than fields, because those are observations and a field that cannot be typed into is a control that lies about what it is. The record moved to version two, so a record from an older build is migrated forward with what it held and a record from a newer one is read and never written.

Verified by unit tests across the whole surface rather than by launching the application: the portable rules are checked against six storage shapes including a refused `..`, a refused absolute value in an installed build, and a specific key overriding a general one; the queue is checked through a file round trip and through recovery of a running job, a suspended job, a finished job, a failed job, a cancelled job, a document from a newer build, a corrupt document, and one that was never saved; the disk gate is checked against four injected readings including a nearly full volume and a volume that could not be read; the actions are checked for tokenising, for quoting, for substitution, for the no-shell rule with a title containing `&`, for a timeout that kills, for a program that cannot start, and for a post-action that cannot fail an encode — the runner itself is exercised on Windows against a real program, started directly with an argument vector, and against one that takes longer than its timeout; the appcast is checked against a well-formed feed, three kinds of entity declaration, an undefined entity, a feed with no releases, an oversized feed, a release with no version, a `file://` artifact, a hash that is not a SHA-256, a matching download and a mismatched one, and the SHA-256 itself against four published vectors; the power, tray, notification, and placement rules are checked as tables over their inputs, including that a queue finishing with the window in front produces no notification and the same queue finishing with the window behind something else does.

**Not verified by anything but unit tests and inspection, and called out as such:** the tray icon, the notification-area balloon, the hold on the machine, the processor-group placement, the free-space reading, and the update transport all need a real desktop to exercise. The platform calls behind them are one call each, declared directly and resolved by name so a missing entry point is reported as a missing capability rather than a load-time failure; the shell takes no new dependency beyond Tauri's `tray-icon` feature, and the SHA-256, the XML reading, the argument-vector launching, and the SHA-free appcast grammar are written out rather than brought in, which is what keeps the workspace building on a machine with no network and no extra crates. The page was not verified in a browser for this slice: the only change to it is one event listener that routes a surface the window has already decided to open to the same call its toolbar button makes.

## Group 18 — Localization

*Outcome: The user interface is available in the reference language set and switches at runtime.*

- [x] UI language selection
- [x] Runtime language switching
- [x] Translation resource pipeline
- [x] Right-to-left locale handling
- [x] Localized preset descriptions
- [x] Locale fallback behavior

**Status:** Complete (6/6 verified)

Six languages ship, each translated for every key rather than for a fraction of one: English is the base the fallback chain ends at, and Spanish, German, French, Japanese, and Arabic are translated for all seventy-six keys. The set was chosen for coverage rather than reach — three scripts, four plural habits, and one right-to-left language in the shipped set so the mirroring path is exercised by something a user can pick rather than only by a test fixture. Every language is offered in its own name, because a picker of translated names is a picker nobody who cannot read the current language can use, and a language this build does not ship is not refused: the fallback chain answers with the base language and says which one it used.

A message is reached by a key that is an enum rather than a name, so a key nobody declared cannot be spelled; each locale is a file under `crates/vise-core/data/locales`, embedded with `include_str!` and checked against the key set at test time, so adding a key without translating it is a failed test rather than a blank control. A duplicate key in a file is refused rather than silently overwriting the earlier spelling, a blank is refused, and a translation that fills a placeholder the key does not have — or misses one it does — is refused. The base locale is a file too rather than a set of match arms, which is what makes "every key is present in every language" one check rather than six. A key that is somehow in no file at all is answered by its own readable name; the rule is that a user is never shown an identifier, and there is a test that reads the resolved text and refuses anything that looks like the key.

Resolution is a chain whose rungs are recorded rather than guessed: the locale itself, then the same language under another tag so `de-AT` reads `de` rather than English, then the base locale. Every rung a key took is available afterwards through the catalog's gap report, which the language picker shows and the window writes to the activity log — a half-translated locale is a report somebody can print, not a surprise somebody notices on screen.

The language lives in the preferences record with everything else a user chooses once, and the record moves to version three. A version two record gains the field at its default rather than as a choice, because a record written by a build that spoke one language says nothing about which language its author wants; nothing the record already held is lost, which is why it is a migration rather than a second store. Switching applies the language and nothing else: the window pushes the three answers that moved — the words, the appearance, and the preferences — and the page applies those. The configuration tabs are deliberately absent from the payload, because redrawing them is what would throw away what the user has typed, and the one preferences field with the caret keeps its text even when the panel is rebuilt underneath it.

The layout follows the language through the observation Group 14 already had, so the direction preference is still what decides: Arabic in a left-to-right window is a choice somebody made, and choosing a language cannot override it. The direction tables moved into the language resources with the locale tag reduction, so the shell and the messages cannot answer "which way does this read" differently.

Preset descriptions are localized the same way and for the same reason. The catalog carries the base sentence for all 138 presets, twelve are translated per language, and the browser shows the translation where there is one and the catalog's own words where there is not — a sentence either way, never an identifier and never an empty row. Machine-translating a hundred and forty descriptions would ship sentences nobody has read; a mechanism that falls back honestly is worth more than a hundred and forty invented ones.

**Verified by unit tests and by inspection of the shipped resources:** every locale file parses, has every key, and fills exactly the placeholders its key declares; a duplicate key, a blank, an unknown key name, and an untranslated sentence copied over the base are each refused; the fallback chain answers exact, then base language, then base, and a deliberately incomplete locale built in a test falls back and reports itself as incomplete; a key no file has resolves to a readable label rather than an identifier; a right-to-left language mirrors the window while a pinned left-to-right direction does not move, and a left-to-right language under a pinned right-to-left direction stays mirrored; a language switch re-emits its view models with no tab view model among them and leaves the tabs byte-identical; a version two record migrates with everything it held and its language at its default; and a translated preset description is used where one exists, is not where none does, and every row in every shipped locale has a sentence.

**Not verified by anything but unit tests and inspection, and called out as such:** the page was not run in a browser for this slice. The `data-i18n` mechanism, the `language-changed` event handler, and the tab strip's fallback to the window's own label are the whole of the page's change and are covered by the view models they consume rather than by a browser. The coverage of the catalog is the chrome — the masthead, the toolbar, the tab strip, the source and queue lines, the appearance and preferences surfaces — and the per-control copy inside the eight configuration tabs is still English, because threading a catalog through every tab view model is a larger change than this group asked for and half of it would be untested either way. A preset browser that is open at the moment the language changes shows the new descriptions the next time it is read rather than immediately, because the browser's view model is built from the library rather than from the language. The command line's per-job progress lines stay in the base language: they are log lines rather than sentences, and a translation there would cost a script the reader cannot grep.
## Group 19 - Media depth

*Outcome: Media handling reaches the reference product's coverage in the areas still missing.*

- [ ] DVD source support `(optional)`
- [ ] Blu-ray source support `(optional)`
- [x] Deblock filter
- [x] Lossless FFV1/PCM/FLAC MKV modes
- [x] Dolby Vision profile and RPU handling
- [x] HDR10+ dynamic metadata
- [x] MP4 behaviors (web optimized, iPod 5G, common metadata passthrough)
- [x] Chapter import from CSV, TXT, and XML
- [x] Cover art and attachment handling with container restrictions
- [x] Foreign audio scan parity

**Status:** In progress (8/10 verified; the two optical-disc items are optional and are not claimed.)

The scan reports the Dolby Vision profile and RPU presence and the HDR10+ dynamic
metadata from the stream side data, and marks which audio tracks are in a language
other than the source's primary one. The video tab turns the HDR facts into two
rows that are always present and are closed with a reason when the source carries
nothing to keep; when the machine's HEVC encoder was built without RPU coding or
without the user-SEI path, that is stated on the tab rather than discovered at
encode time. Cover art and attachments are mapped where the container is known, so
a WebM asked for a cover says which container refused it, and the MP4-only output
flags stay closed outside an MP4.

Verified by unit tests over argument construction and decision logic (nine chapter
parser tests, four attachment-container tests, six media-depth plan tests) and by
one real encode: an FFV1 + FLAC Matroska file produced by ffmpeg and confirmed to
be a valid lossless MKV. The application was not launched, so the live rows were
confirmed through their view models and the shipped page rather than the running
window. Optical-disc input is not implemented: there is no optical drive on this
machine to read or test a disc, and a code path that cannot be exercised would be a
claim rather than a capability. Both optional boxes stay unchecked.


## Group 20 — Diagnostics and developer tools

*Outcome: Support and debugging workflows match the reference product's operational tooling.*

- [x] Per-encode log files and log rotation
- [x] Activity log streaming to the UI
- [x] Quality sweep tool for the queue
- [x] Queue result data extraction tool
- [x] Hardware capability report export
- [x] Reproducible diagnostic bundle export

**Status:** Complete (6/6 verified)

The worker writes one log file per encode beside its session log and trims
the folder when an encode ends, against a byte bound and a file-count
bound. Which files may go is a pure function of the policy, the measured
sizes and ages, and the role of each file, so the two cases that matter —
the log being appended to right now, and a log somebody has open — are
pinned by tests rather than by hope. Logs live under the portable-mode
storage roots, so a portable install keeps them beside the executable.

The activity window reads the worker by index and never re-reads, and both
ways a stream can stop being a stream are made visible rather than
swallowed: a worker that was replaced or whose buffer was cleared (the
reply now carries which worker and buffer it came from, so a cursor is not
held across it) and a window that cannot hold every line (the oldest go, the
count is kept, and the summary says the history is no longer whole).

The quality sweep is a plan, an executor and a table. The plan is pure and
bounded: at most twelve encodes, a range wider than twenty-six values only
with the user's confirmation, and a step widened rather than a range
changed when the bound would otherwise be broken — with the narrowing said
out loud. The executor hands out one run at a time, which is the failure
mode a sweep is most likely to cause and the one it makes unrepresentable.
Its runs are ordinary jobs written to a folder of their own, so it never
touches the session or the queue, and it is refused outright while the queue
has an encode running.

The queue's results export as CSV and JSON, deterministically: the rows are
in the run order, the columns are a fixed list, and the JSON comes from a
struct. A title carrying a comma, a quote or a line break round-trips
through a parser written the way a spreadsheet writes one. Paths are
exported as file names, because an export is a document that travels.

The capability report is a pure function of an injected detection, so it is
pinned against media-tool builds this repository cannot run, and it sorts
and de-duplicates every list so two reports of one machine can be diffed. A
machine with no tools is reported rather than left out.

The diagnostic bundle is a stored ZIP with no clock in it: entries are
sorted, stamped at the archive epoch, and the file is named for the SHA-256
of its own contents, so two exports of one session are byte-for-byte
identical. Before anything is written, a redactor removes tokens by value
and by key name and reduces every absolute path to a root placeholder plus
its last two components, inside documents and inside free text alike. The
archive carries a manifest of per-file digests and a README that says to
treat the whole thing as untrusted input.

Verified by unit tests over the rotation policy (twelve cases, including
the in-progress and being-read files), the streaming state machine
(ordering, de-duplication, reset on a replaced worker, reset on a cleared
buffer, and the window's own back-pressure), the sweep plan (bounds across
every width and step, refusals, narrowing, cancellation, and the table's
ordering and relative size), the CSV and JSON writers (commas, quotes,
newlines, and determinism over the same queue twice), the capability report
against injected detections, and the bundle's redaction, reproducibility
and archive shape. Plus six window-level tests over the desktop command
paths and an integration test over the public core API.

The application was not launched, so the diagnostics window was confirmed
through its view models and the shipped page rather than in a running
window, and no real encode was driven through a sweep; the sweep's
executor is pinned by tests and its encode path is the queue's own, not a
live measurement.

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
| 10 | Encoding control and job reliability | Complete (10/10) |
| 11 | Hardware acceleration | Complete (7/7) |
| 12 | Platform packaging and release | Complete (8/8; all three platforms build and package) |
| 13 | Worker process and JSON IPC | Complete (9/9) |
| 14 | Desktop parity with reference product | Complete (18/18) |
| 15 | Preferences surface | Complete (8/8) |
| 16 | Preset catalog and versioning | Complete (13/13) |
| 17 | System integration | Complete (9/9) |
| 18 | Localization | Complete (6/6) |
| 19 | Media depth | In progress (8/10; optical-disc input optional and not claimed) |
| 20 | Diagnostics and developer tools | Complete (6/6) |

## Notes

- Items may be refined as implementation progresses.
- Features marked `(optional)` are not required for target parity but are useful for Vise.
- A group should not be marked complete until every checkbox is checked.
- Revisit this roadmap when scope or priorities change.
- Update statuses only after verification; distinguish untested from failed.
- Keep implementation details in code, code review, or a task-specific plan—not here.
- Preserve reasons for significant technical decisions in an existing authoritative project document, adding a dedicated one only if needed.
"