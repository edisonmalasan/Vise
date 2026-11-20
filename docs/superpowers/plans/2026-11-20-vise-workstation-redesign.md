# Vise Workstation Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Execution in this session is **native** (the user pre-authorized autonomous work in `docs/GOAL.md`).

**Goal:** Replace Vise's settings-form UI with a cohesive professional media-workstation workspace without changing the Rust backend or losing any capability.

**Architecture:** Keep the vanilla HTML/CSS/JS Tauri frontend and every Tauri command. Restructure the single page into five landmark regions (app bar, icon activity rail, workspace, job dock, status bar); rewrite the stylesheet as a new design system; make contained `app.js` changes for native dialogs, the icon rail, the display menu, and the job dock. The Rust shell continues to own every decision and sends finished view models.

**Tech Stack:** Tauri 2, vanilla HTML/CSS/JS (no build step), Rust workspace, inline SVG sprite, locally bundled IBM Plex Sans/Mono (OFL).

**Spec:** `docs/superpowers/specs/2026-11-20-vise-workstation-redesign-design.md`

## Global Constraints

- Preserve all 199 element ids in `index.html`; `app.js` queries 87 literally and builds the rest at runtime (`stepAnswer-*`, view-model control ids).
- Preserve the surface contract: `surface-<id>`, `<id>-label`, `<id>close`, `surface-<id>-body`, `buttonId`, `openable`, `badge`, `modal`, `role`, `initialFocus`.
- Preserve all 89 Tauri command names, view-model field names, event names, and the worker contract. No Rust rewrite.
- Runtime language switching, RTL, `data-theme` resolution, visible focus and reduced motion must keep working.
- No hardcoded English interface strings in new chrome: use `data-i18n` and the existing 100-key catalogue.
- No remote asset requests at runtime; bundled fonts with OFL licence.
- Do not adopt `focusgroup`, `popover`, `light-dark()`, anchor positioning, or `closedby` as required behaviour (not Baseline Widely available; WebKitGTK risk).
- Verification commands: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- Browser support policy to record in `AGENTS.md`: assume **Baseline Widely available**; gate newer features behind feature detection with graceful degradation.

## Review Focus

- **Empty workspace with no source:** the output tab form must not read as "ready"; next action is obvious. Pin with a real-window screenshot at 1024×768 and 1440×900.
- **Long unbreakable Windows paths** in `#pathInput`/`#outPath` and in dock text must not cause horizontal overflow. Pin with `min-inline-size: 0`, `overflow-wrap`, and a screenshot with a long path.
- **Eight output tabs at 1024 px:** keep one row, no wrap; scroll with the selected tab brought into view. Pin with a screenshot and the existing `handleTabKeys` behaviour.
- **Native dialog round-trip:** opening, Esc, and closing a surface must keep Rust's `state.open` in sync (cancel intercepted). Pin with a real-window test of Queue and Preferences, plus `cargo test` for the Rust state.
- **Theme/RTL/locale:** German must translate new chrome; RTL must mirror the new regions and the dock; both themes must have 4.5:1 body text. Pin with screenshots in German + RTL + light, and the existing `the_language_answer_carries_every_word_the_page_asks_for` test.

---

### Task 1: Bundle the type system and icon sprite

**Files:**
- Create: `apps/vise-desktop/ui/fonts/plex-sans-latin.woff2`, `plex-sans-latin-ext.woff2`, `plex-mono-latin.woff2`, `plex-mono-latin-ext.woff2`, `Plex-OFL.txt`
- Modify: `AGENTS.md` (record the browser support policy)

- [ ] **Step 1: Fetch the four woff2 subsets and the OFL licence into `apps/vise-desktop/ui/fonts/`** using the Google Fonts CSS API with a desktop User-Agent; verify each file's magic bytes are `wOF2` and the licence names IBM Plex.
- [ ] **Step 2: Confirm no runtime network URLs remain** after the redesign (`grep` the UI folder for `https://`), expecting zero.
- [ ] **Step 3: Add the browser support policy line to `AGENTS.md`** verbatim: "Browser support: Baseline Widely available; newer web features are adopted only with feature detection and graceful degradation, because Tauri targets WebView2, WKWebView and WebKitGTK."
- [ ] **Step 4: Commit** `feat(desktop): bundle the workstation type system offline`.

### Task 2: Restructure the page into five landmarks

**Files:**
- Modify: `apps/vise-desktop/ui/index.html`
- Test: `cargo test -p vise-desktop --bin vise-desktop the_language_answer_carries_every_word_the_page_asks_for`

- [ ] **Step 1: Add a contract test** (or extend the existing one) asserting that every id queried by `app.js` exists in `index.html`, excluding ids created at runtime (`backdrop`, `surface-*`, `*-body`). Run it and confirm it fails before the restructure if a contract id is dropped.
- [ ] **Step 2: Rewrite the body** with `<header class="appbar">`, `<nav id="toolbar" class="activity">`, `<main id="work" class="workspace">`, `<aside class="dock">`, `<footer class="statusbar">`, and `<div id="surfaces">`. Keep `#workScroll` as the workspace's scroll container.
- [ ] **Step 3: Preserve the source contract** (`#dropzone`, `#dropTitle`, `#dropHint`, `#pathInput`, `#openBtn`, `#scanBtn`, `#sourceCurrent`, `#sourceKind`, `#sourceName`, `#sourceLocation`, `#clearSourceBtn`, scan status/progress/notice, `#recentList`, `#clearRecentsBtn`, `pathHint`) and keep a `.step[data-step="source"]` wrapper so `data-loaded` still applies.
- [ ] **Step 4: Preserve the inspector contract** (`#titleList`, `#titleEmpty`, `#rangeSummary`, `#rangeModeList`, `#pointRow`, `#startPoint`, `#endPoint`, `#angleRow`, `#angleSelect`, `#rangeNotice`, `#chapterList`, `#chapterEmpty`, `#audioList`, `#audioEmpty`, `#subtitleList`, `#subtitleEmpty`, and all `#stepAnswer-*`).
- [ ] **Step 5: Preserve the output contract verbatim**: `#tabSource`, `#tabSourceNote`, `#tabList`, `#tabPanels`, all eight `#panel-*`, and every field/hint id inside them. Move only the surrounding containers.
- [ ] **Step 6: Preserve the dock contract** (`#outPath`, `#queueBtn`, `#openSelectionBtn`, `#queueHint`) and the display contract (`#appearanceHeading`, `#appearanceThemeList`, `#appearanceDirectionList`, `#appearanceSummary`, `#appearanceFlow`, `#appearanceSummaryText`, `#appearanceObservation`).
- [ ] **Step 7: Run the id-contract test and the i18n test; both must pass. Commit** `feat(desktop): compose the window as a workstation workspace`.

### Task 3: Write the new stylesheet

**Files:**
- Rewrite: `apps/vise-desktop/ui/styles.css`
- Add: inline SVG sprite markup lives in `index.html` (Task 2), styled here.

- [ ] **Step 1: Define the token system** — the two palettes from the spec, `@font-face` rules for the four bundled files, spacing/radius/type scales, and `color-scheme: light dark` on `:root` with `<meta name="color-scheme">` already in the head.
- [ ] **Step 2: Lay out the shell** with named `grid-template-areas`, `56px 1fr 320px` columns, `56px 1fr 28px` rows, logical properties, and `min-inline-size: 0` where paths live.
- [ ] **Step 3: Style each region** to the spec: app bar, icon rail, source bar, media inspector, preset launcher, output tabs, job dock, status bar. Selected tabs use a non-colour underline from `aria-selected`; no `focusgroup`.
- [ ] **Step 4: Add responsive rules** — dock collapses below 1200 px behind an app-bar toggle; below 900 px rail becomes a top icon strip and the inspector stacks. Use container queries for the inspector rather than more viewport breakpoints.
- [ ] **Step 5: Gate motion** behind `@media (prefers-reduced-motion: no-preference)` and verify focus-visible rings exist on every interactive element.
- [ ] **Step 6: Verify** with `npx.cmd -y modern-web-guidance@latest retrieve "css-layout"` cross-check (logical properties, `min-inline-size`, named areas) and a browser screenshot at 1440×900 in both themes. Commit `feat(desktop): apply the workstation visual system`.

### Task 4: Modernise surfaces to native dialogs and wire the new chrome

**Files:**
- Modify: `apps/vise-desktop/ui/app.js`
- Test: `cargo test -p vise-desktop --bin vise-desktop window_catalog` (or the relevant existing surface tests)

- [ ] **Step 1: TDD** — write a focused JS check (Node + the design harness) that opening `#surface-queue` results in `dialog.open === true`, and that a `cancel` event calls `close_window`. Run it to confirm it fails.
- [ ] **Step 2: Change `buildSurface()`** to create `document.createElement("dialog")` with `id="surface-<id>"`, an `<h2 id="<id>-label">` and the existing `<div id="surface-<id>-body">`. Do not set `role="dialog"` or `aria-modal`; do not add a manual focus trap.
- [ ] **Step 3: Change `openSurface()`/`showSurface()`** to call `showModal()` when `spec.modal` and `show()` otherwise; keep `initialFocus`. Remove the custom `#backdrop` element and its `syncBackdrop()` calls.
- [ ] **Step 4: Add a `cancel` listener** to each dialog that calls `event.preventDefault()` and `closeSurface(id)`, so Esc keeps Rust's open-state in sync where `closedby` is missing.
- [ ] **Step 5: Change `hideSurface()`** to call `dialog.close()` before hiding; keep the Rust-driven `restoreFocus`.
- [ ] **Step 6: Add the icon rail** rendering in `loadCatalog()`: each openable surface gets an icon-only button with `aria-label` and `title` from the Rust hint, `id = spec.buttonId`, and the existing badge.
- [ ] **Step 7: Add the display-menu toggle** and keep `applyAppearance()`/`renderAppearance()` wired to the preserved ids. Add the dock queue renderer fed from `queue_window` polled state and status-bar updates from scan/queue state — no new Tauri commands.
- [ ] **Step 8: Run the JS check and `cargo test`; commit** `feat(desktop): use native dialogs and wire the workstation chrome`.

### Task 5: Real visual verification and iteration

**Files:** Modify `index.html`, `styles.css`, `app.js` as defects are found.

- [ ] **Step 1: Launch the real app** and capture screenshots at 1024×768, 1280×800, 1440×900, 1920×1080, dark and light.
- [ ] **Step 2: Inspect the six required states** — empty workspace, scanned source, preset catalogue, advanced output tabs, queue with active progress, preferences/secondary surfaces — and record defects.
- [ ] **Step 3: Fix and re-capture** each defect; keep before/after images under `docs/superpowers/evidence/`.
- [ ] **Step 4: Verify keyboard focus, RTL, and German** in the real window; fix anything broken.
- [ ] **Step 5: Commit** `fix(desktop): resolve defects found in visual verification`.

### Task 6: Technical verification, documentation, and integration

**Files:** Modify `DESIGN.md`, `README.md`, `docs/ROADMAP.md`, `docs/screenshot-*.png`, `docs/GOAL.md`.

- [ ] **Step 1: Run** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`; record exact summaries.
- [ ] **Step 2: Rewrite `DESIGN.md`** to document the new system (regions, tokens, type, icons, motion, states, accessibility) without the old true-black rules.
- [ ] **Step 3: Update** `README.md` screenshots/caption, `docs/ROADMAP.md` Group 14 note, and the Obsidian project note if accessible.
- [ ] **Step 4: Refresh Graphify** if the tool is installed, on a separate chore commit.
- [ ] **Step 5: Push the task branch, merge to `main` after verification, and report** using GOAL.md's completion format, including the `### Modern Web Guidance` section.
