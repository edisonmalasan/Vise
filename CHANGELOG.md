# Changelog

All notable changes to Vise Transcoder are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [semantic versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Nothing yet.

## [0.1.0] - 2026-10-19

The first release. A working transcoder rather than a finished product; the
release notes carry the honest account of what it does and does not do, and
the downloads are on the [releases page](https://github.com/edisonmalasan/Vise/releases/tag/v0.1.0).

### Added

- Media ingestion from files and folders, from optical discs, and from a
  disc image, with the titles, chapters, tracks, range, and angle a source
  carries.
- Transcoding across container, video codec, audio codec, filters,
  dimensions, subtitles, chapters, and metadata, all configurable on the
  output tabs.
- A catalogue of 138 built-in presets, each resolved against the hardware the
  machine actually has.
- A queue that runs jobs in order, with an activity log, a preview, a
  problems view, and a diagnostics bundle.
- Six shipped interface languages, a light and a dark theme, and a layout
  direction that follows the language and can be forced either way.
- The `vise` command line: `transcode`, `batch`, and `presets`.
- Platform builds: a Windows installer, a Debian package, an AppImage, and a
  macOS disk image.

### Changed

- The desktop window was rebuilt as a shell. The name and the controls that
  open the other windows are a bar at the top, the appearance settings are
  one line of switches rather than a panel above the work, the work is three
  bands in the order the task happens, and the output path and the button
  that uses it are pinned to the foot of the window.
- The spacing was rebuilt on a 4-unit scale, so the space between bands is
  twice the space inside a card and the page reads as steps rather than as
  one long column of equal boxes.

### Fixed

- The media tool scripts asked the pin file for a build under the name of
  the machine rather than the name of the tool, so an Apple silicon Mac
  looked for tools nobody publishes and the fetch failed.
- A selected catalogue entry was marked with a thick coloured edge on one
  side, which reads as a tag rather than as a selection.

[Unreleased]: https://github.com/edisonmalasan/Vise/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/edisonmalasan/Vise/releases/tag/v0.1.0
