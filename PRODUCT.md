# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The application is a desktop program whose interface is a web document rendered in
a native shell (Tauri 2). The design language is therefore a web document's, and
the native shell contributes only the window, the tray, and the file dialogs.

## Stack

Rust workspace (Cargo) with five crates — `vise-core`, `vise-cli`, `vise-ipc`,
`vise-worker`, and the `vise-desktop` Tauri application — plus a dependency-free
HTML/CSS/JS interface with no build step. The encoder is reached as a subprocess
rather than a binding, and a release carries its own pinned copy of that tool.

## Users

The primary user is someone with a video or audio file they need to convert:
they have a source, they have a target format or a device in mind, and they want
the result without learning a command line. A secondary audience is a
contributor working on the clean-room implementation, who reads the roadmap and
the core API rather than the interface.

## Product Purpose

Vise Transcoder converts video and audio between containers and codecs, and
gives the user direct control over the result: which title, which range, which
audio and subtitle tracks, and every encoder setting.

Success means the user picks a source, sees what is in it, makes a few choices,
and gets a file that plays where they intended it to.

## Positioning

The reference product this work is measured against is a mature, dense
transcoder. The claim here is different in two ways.

First, the work is clean-room: no reference source, strings, presets, or assets
were copied. The catalogue, the interface, and the pipeline are written from
scratch against observed behaviour.

Second, the interface is deliberately better than the reference's. The reference
is a dense form; this is a windowed, staged workspace where a person can see
the source, the titles, the tracks, and the output settings at once, and where
every control explains itself rather than relying on a tooltip.

## Operating Context

The user works in a desktop application beside a file browser. A typical session
is: open a file or drop a folder on the window, scan it, pick the title and
range, choose the tracks, set the output up on the tabs, and queue the result.

The media tooling is a separate program. A shipped build carries its own pinned
copy of it, because the filters and encoders the application offers are only
present in some builds of that tool; a development build may fall back to
whatever the machine has.

A long encode runs in a separate worker process, so the window stays responsive
and an encoder crash costs one job rather than the session.

## Capabilities and Constraints

- Scan a file or a folder; list the titles, chapters, audio tracks, subtitle
  tracks, and detected range and angle.
- Configure the output across eight tabs: summary, dimensions, filters, video,
  audio, subtitles, chapters, metadata.
- Presets: a built-in catalogue of 138 entries in six categories, with
  per-machine availability resolution that substitutes a software encoder for a
  missing hardware one and says so.
- Queue, a queue selection window, an activity log, a preview, a problems view,
  an about view, a preset manager, preferences, and a diagnostics bundle.
- Six preferences tabs and six shipped interface languages, switchable at run
  time, with a fallback chain that never shows a raw key.
- Right-to-left layout and a light/dark/system theme, both chosen in the app.
- Optical-disc input: a DVD, a Blu-ray, or an optical image.
- Hardware acceleration where the machine has the device for it.
- The encoder runs as a subprocess; there is no native binding.

Explicitly undecided: whether the reference's remaining optional behaviours
should be added, and how the product should be distributed beyond GitHub
releases.

## Brand Commitments

The name is **Vise Transcoder**. The mark is a pair of right-pointing chevrons,
which read as a transcode. The voice is plain and technical: it says what a
control does, names the thing it acts on, and states a limitation rather than
hiding it. Where a capability is unavailable, the interface says which
capability is missing.

## Evidence on Hand

- `docs/ROADMAP.md` — 187 of 187 outcomes verified across 21 groups, each group
  recording what was checked and what could not be exercised.
- `docs/screenshot-desktop.png` and `docs/screenshot-tabs.png` — the application
  running, with a scanned source.
- `crates/vise-core/data/builtin-presets.json` — the 138-entry catalogue.
- `.github/workflows/release.yml` — installers built for Windows, Linux, and
  macOS.
- `crates/vise-core/tests/roadmap.rs` — a test that fails when the roadmap's own
  counts disagree.

Absences future work must not fabricate: no user testimonials, no customer
logos, no benchmark numbers, no download counts, and no claim that the
application has been used on physical optical media.

## Product Principles

1. **The window never lies about the work.** A job that is running says so, a
   job that failed says why, and a control that cannot be used says what is
   missing rather than quietly doing nothing.
2. **The pipeline never runs inside the window.** Isolation is the default
   because a long encode must not freeze the interface and a crash must not
   cost the session.
3. **The shipped build is the tested build.** The application carries the
   encoder it was tested against, pinned by digest, and checks that digest
   before it trusts what it downloaded.
4. **Every output is explainable.** The settings on the tabs are the settings
   the encoder will be given, so what the user sees is what will happen.
5. **The interface earns its place.** Every control does something a
   transcoder can actually do, and nothing is shown that the machine cannot do.

## Accessibility & Inclusion

Keyboard-operable throughout, with visible focus and no keyboard trap. Text
scales with the user's settings. Motion is reduced on request.
Right-to-left layout is a supported layout direction, not a mirror of a
left-to-right design, and follows the language when the language is chosen for
the reader. Status changes are announced rather than only shown. The interface
ships in six languages and never shows a translation key to a reader.
