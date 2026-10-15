# AGENTS.md

## Project

- **Name:** Vise Transcoder
- **Purpose:** Clean-room implementation of a media transcoder targeting a reference transcoder architecture at version 1.11.2 and its user-facing capabilities. The repository currently contains planning and reverse-engineering documentation; implementation has not started.
- **Primary users:** Not yet verified from repository evidence. Intended audiences are media-transcoding users and contributors; see `docs/ROADMAP.md` for the product/capability target.
- **Current state:** No application source, manifests, tests, or build configuration are present. `docs/ROADMAP.md` defines intended milestones. `re/docs/` contains reverse-engineering notes for a reference transcoder at version 1.11.2 that inform the design.

## Source of Truth

**Read only the documentation relevant to the task. Do not assume—verify.**

| Source | Read when |
|---|---|
| `docs/CONTEXT.md` | Locating context, constraints, and relevant documentation |
| `docs/ROADMAP.md` | Checking planned features, priorities, or verified progress |
| `docs/ARCHITECTURE.md` *(if it exists)* | Making changes to established boundaries or data flows |
| Source code and tests | Determining actual current behavior and verifying a change |
| `graphify-out/` *(if it exists)* | Exploring code relationships; verify findings against source |

- Requirements and explicitly approved decisions define **intended behavior**; source code and passing tests provide evidence of **current behavior**.
- If documentation and code conflict, **report the discrepancy** and determine whether the code or documentation needs correction. Never silently rewrite a requirement to justify current code.
- Prefer one authoritative location per fact; link to it rather than copying it into multiple files.

## Tech Stack

*No runtime, build, or test technology is implemented in this repository yet. Planned technologies are recorded separately and should not be treated as present until manifests or source exist.*

| Layer | Technology / version | Evidence |
|---|---|---|
| Frontend / UI | Not implemented; planned: Tauri + web UI | `docs/ROADMAP.md` (architecture decision snapshot) |
| Backend / runtime | Not implemented; planned: Rust core crate | `docs/ROADMAP.md` (architecture decision snapshot) |
| Database / storage | Not used currently | `docs/ROADMAP.md`; no storage code or manifests present |
| Build / tooling | Not configured. Optional derived analysis: Graphify (`graphify-out/`) | Repository file inventory; `graphify-out/` |
| Tests | None present | Repository file inventory |

## Build, Test, and Development Commands

*No runnable project commands are defined yet. Do not invent scripts.*

| Purpose | Command | Evidence / notes |
|---|---|---|
| Install dependencies | Not available | No dependency manifests in repository root |
| Run locally | Not available | No application or package configuration present |
| Build | Not available | No build configuration present |
| Test | Not available | No tests present |
| Lint / type-check | Not available | No lint or type configuration present |

Run from the repository root unless a command explicitly requires another directory.

## Architecture Non-Negotiables

- Clean-room implementation: new code, not a fork. Evidence: `docs/ROADMAP.md`, architecture decision snapshot.
- Reverse-engineering notes are behavioral/structural references only; implementation must not copy source code, strings, presets verbatim, or protected assets. Evidence: `re/docs/README.md`.
- Core must have no UI dependency; frontends should call core public APIs and poll state. Evidence: `re/docs/01-system-overview.md`; intended architecture in `docs/ROADMAP.md`.
- `re/` is ignored by `.gitignore`, so those notes are local reference material rather than tracked project source.
- `graphify-out/` is derived project knowledge, not the source of truth for code behavior.

## Working Method (Lightweight ICM)

1. **Orient:** Read `docs/CONTEXT.md`, then only the relevant roadmap section, existing source files, and needed references.
2. **Plan when warranted:** For substantial changes, summarize approach, affected files, risks, and verification criteria before coding. Small, clear fixes do not need separate plan documents.
3. **Implement within scope:** Follow existing conventions and avoid unrelated changes.
4. **Verify:** Run applicable checks; report passed, failed, skipped, and unavailable checks accurately.
5. **Preserve:** Update `docs/ROADMAP.md` only for verified outcomes, and update other authoritative docs only when facts change.

Do not create extra ICM stage folders or permanent plan files unless the task benefits from them.

## Graphify

- Graphify is **derived project knowledge**, not the source of truth for code behavior.
- When available and relevant, query the existing graph to find related symbols and dependencies; inspect source files before acting on results.
- After meaningful structural changes, refresh the graph using the **installed** Graphify skill or CLI workflow. Do not presume Graphify is installed or that a graph exists.
- If exporting to Obsidian, use only this project's generated area: `10 - Projects/Vise/_Graphify/`.
- Never export into the vault root or overwrite curated PARA notes. Refreshing the graph is **not** the same as saving session decisions.

## Obsidian / Cross-Project Knowledge

- **Central vault:** Not configured. No vault path is recorded in this repository.
- **Project notes:** `10 - Projects/Vise/`, if present.
- Read only relevant notes, when the vault is configured and accessible; repository docs remain authoritative for project-specific requirements.
- Propose meaningful updates after substantial work: project progress/decisions under `10 - Projects`, reusable lessons under `30 - Resources`, unverified ideas under `00 - Inbox`.
- **Ask before editing curated vault notes.** Avoid duplicates, stale claims, secrets, and copying entire chat transcripts.
- If the vault is inaccessible, continue using local project context and mention the limitation.

## Git Workflow

1. **Never commit directly to `main` or another protected default branch.** Use a task branch when committing.
2. Branch names: `feat/`, `fix/`, `chore/`, or `docs/` followed by a short slug.
3. Make focused commits at logical checkpoints **when commits are authorized**; do not commit automatically without permission.
4. Use a configured `git` subagent for Git operations **if this harness provides one**. Otherwise, use permitted Git tooling; never assume a subagent exists.
5. Do not merge until the relevant build/tests pass; report pre-existing warnings and introduce no new errors or warnings.
6. Do not stage transient agent-generated plans (for example, `docs/superpowers/`) unless the user explicitly wants them versioned. Respect the repository's `.gitignore`.
7. Do not push, merge, reset, or discard changes without explicit authorization.
"