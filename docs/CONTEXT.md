# Project Context

> Keep this file short: it is a **router**, not a duplicate README or architecture specification.

## At a Glance

- **Project:** Vise Transcoder
- **What it does:** Clean-room media transcoder targeting a reference transcoder architecture at version 1.11.2 and its user-facing capabilities; the repository holds Rust workspace source, tests, and planning/reverse-engineering documentation.
- **Primary users / use cases:** Not yet verified from repository evidence. Intended audiences are media-transcoding users and contributors working on the clean-room implementation.
- **Current focus:** Every group in `docs/ROADMAP.md` is implemented and verified: 187 of 187 outcomes across 21 groups. The application has the source, scan, title, range, angle, and track surfaces; the seven configuration tabs and the metadata tab; the six preferences tabs; the secondary windows and dialogs (queue, queue selection, activity, preview, problems, countdown alert, about, presets); right-to-left layout and theme selection; six shipped interface languages with runtime switching; a worker process behind a token-authenticated loopback API; a built-in preset catalogue with per-machine availability resolution; and installers for Windows, Linux, and macOS, each carrying its own pinned media tooling. Where a group's outcome could not be exercised on the machine that built it, its note in the roadmap says so rather than the item being quietly deferred.

## Where to Look

| Need | Canonical source |
|---|---|
| Goals, features, and progress | `docs/ROADMAP.md` |
| Current application behavior | `crates/vise-core` (media, pipeline), `crates/vise-worker` (worker JSON API), `apps/vise-desktop` (shell); `crates/vise-ipc` (client, supervisor, contract) |
| Product truth and the interface's design system | `PRODUCT.md` and `DESIGN.md` at the repository root |
| Technical design and key boundaries | `re` reverse-engineering notes (behavioral/structural reference only); `docs/ROADMAP.md` architecture decisions |
| Bundled media tooling | `docs/media-tooling.md`, `media-tools.json` (the pin file) |
| Release notes and the changelog | `release-notes/<version>.md` and `CHANGELOG.md` |
| Build and test commands | Root `AGENTS.md` |
| Agent skills and lock | `.agents/skills/` and `skills-lock.json` (vendored skill definitions) |
| Important existing dependencies | serde/serde_json for the worker contract and for the embedded translation resources; no HTTP framework; ffmpeg/ffprobe subprocesses; `graphify-out/` is derived analysis |
| Reusable lessons from other projects | Central Obsidian vault, when configured |

## Project-Specific Constraints

*Only record constraints that affect many tasks. Link to detailed authoritative documents instead of repeating them.*

- Clean-room implementation must not copy source code, strings, presets verbatim, or protected assets; the reverse-engineering notes are behavioral/structural reference only. Source: `re/docs/README.md`.
- Core must have no UI dependency; frontends should call core public APIs and poll state. Source: `re/docs/01-system-overview.md`.
- `re/` is gitignored, so reverse-engineering notes are local-only reference material and are outside the tracked/documented project source by default.
- `graphify-out/` is derived knowledge and is not the source of truth for code behavior.

## Context Loading Rules

1. Start with the user's task and this routing file.
2. Open only the relevant section of `docs/ROADMAP.md`, not the entire roadmap unless needed.
3. Inspect directly affected source files and tests when implementation exists.
4. Consult architecture docs, related Obsidian notes, or Graphify **only when relevant and available**.
5. If sources disagree, report the conflict; do not silently choose whichever is convenient.
6. Do not treat generated graphs or old memory notes as more authoritative than current source code and approved requirements.
"