# AGENTS.md

## Project

- **Name:** Vise Transcoder
- **Purpose:** Clean-room implementation of a media transcoder targeting a reference transcoder architecture at version 1.11.2 and its user-facing capabilities. The repository now contains Rust application source, tests, build manifests, and planning/reverse-engineering documentation.
- **Primary users:** Not yet verified from repository evidence. Intended audiences are media-transcoding users and contributors; see `docs/ROADMAP.md` for the product/capability target.
- **Current state:** Rust workspace (`Cargo.toml`, `crates/vise-core`, `crates/vise-cli`, `crates/vise-ipc`, `crates/vise-worker`, `apps/vise-desktop`) with Group 0–13 required outcomes implemented where feasible. `docs/ROADMAP.md` tracks verified status. `re/docs/` and `re/README.md` contain reverse-engineering notes.
- **Current progress:** `main` contains the merged integration branches and a reconciled roadmap. Platform installers and optional DVD/Blu-ray support remain pending.
- **Worker architecture:** the desktop shell never runs the pipeline in-process. `crates/vise-worker` serves a token-authenticated loopback JSON API; `crates/vise-ipc` holds the shared contract, the client, the process supervisor (start, handshake, crash restart), and the portable-mode switch that can disable process isolation.

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

| Layer | Technology / version | Evidence |
|---|---|---|
| Frontend / UI | Tauri 2 desktop scaffold with placeholder web UI; planned full Tauri + web UI | `apps/vise-desktop`, `docs/ROADMAP.md` |
| Backend / runtime | Rust stable workspace; verified with cargo 1.99.0 and rustc 1.99.0 | `Cargo.toml`, `crates/vise-core`, `crates/vise-cli`, `crates/vise-ipc`, `crates/vise-worker` |
| Media tooling | ffmpeg 9.0.2 / ffprobe via subprocess bridge | Group 1/2 implementations and tests |
| Worker IPC | Minimal HTTP/1.1 over `std::net` on 127.0.0.1 with a `token` header; serde/serde_json payloads | `crates/vise-ipc/src/http.rs`, `crates/vise-ipc/src/contract.rs` |
| Database / storage | Not used currently; JSON presets/settings via files | `docs/ROADMAP.md`; storage code |
| Build / tooling | Cargo workspace + optional Graphify analysis | `Cargo.toml`, `graphify-out/` |
| Tests | Rust unit/integration tests | `crates/*/tests`, `cargo test --workspace` |

## Build, Test, and Development Commands

| Purpose | Command | Evidence / notes |
|---|---|---|
| Install dependencies | `cargo build --workspace` | Workspace manifests in `Cargo.toml` |
| Run locally | `cargo run -p vise-cli -- --help` | CLI crate entry point |
| Build | `cargo build --workspace` | Rust workspace |
| Test | `cargo test --workspace` | Rust tests |
| Lint / type-check | `cargo clippy --workspace --all-targets -- -D warnings` | Rust workspace |

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

- **Central vault:** `C:\Users\Edison\Documents\Obsidian`
- **Project notes:** `10 - Projects/Vise/`, if present.
- Read only relevant notes, when the vault is configured and accessible; repository docs remain authoritative for project-specific requirements.
- Propose meaningful updates after substantial work: project progress/decisions under `10 - Projects`, reusable lessons under `30 - Resources`, unverified ideas under `00 - Inbox`.
- **Ask before editing curated vault notes.** Avoid duplicates, stale claims, secrets, and copying entire chat transcripts.
- If the vault is inaccessible, continue using local project context and mention the limitation.

## Git Workflow

1. **Never commit directly to `main`.** Use task branches prefixed with `feat/`, `fix/`, `chore/`, `docs/`, or `refactor/`.
2. Before modifying files, check `git status`, switch to `main`, pull the latest `origin/main`, create a task branch, and push it with upstream tracking.
3. Commit at logical checkpoints and push changes to the remote.
4. Use the `git` subagent when available; otherwise, use permitted Git tools.
5. After implementation, run relevant tests, resolve issues, and **merge verified work into `main`**. Follow required CI checks and branch protections.
6. Never discard existing changes, force-push, or commit temporary agent-generated plans unless explicitly authorized.
7. **Do not consider a task complete until it is verified, merged, and reflected in `docs/ROADMAP.md`.**

### Branch Lifecycle

```bash
git status
git switch main
git pull --ff-only origin main
git switch -c <branch-name>
git push -u origin <branch-name>
```

If the working tree is dirty or Git operations are blocked, resolve safely without overwriting existing work.

---

## Autonomous Orchestration Mode

_Activate only when the user explicitly requests autonomous roadmap execution._

1. **Orchestrator:** Read `docs/CONTEXT.md` and `docs/ROADMAP.md`, select the next eligible task, manage Git operations, and verify completion.
2. **Subagent:** Spawn **one implementation-and-testing subagent** per task when supported. Otherwise, perform the work directly.
3. **Execution:** Create branch → Implement → Test → Commit → Push → Merge → Update roadmap. (REPEAT with next task)
4. **Continuity:** Automatically proceed to the next eligible roadmap task. **Do not stop after one feature or merge.**
5. **Blockers:** Attempt safe recovery. Pause only for unresolved errors, missing requirements, permissions, required approvals, or unsafe operations. Continue independent tasks when possible.
6. **Completion:** Stop when all authorized tasks are finished, remaining tasks are deferred or blocked, or the user stops execution.
7. **Knowledge:** Update relevant documentation, preserve reusable lessons in Obsidian, and refresh Graphify after significant changes.