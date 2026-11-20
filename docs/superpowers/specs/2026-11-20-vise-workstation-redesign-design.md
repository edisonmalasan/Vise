# Design: Vise as a professional media workstation

Date: 2026-11-20
Status: approved by the standing directive in `docs/GOAL.md` ("start now, do not stop to ask")

## Intent and why

Vise's transcoding engine is finished and verified; its interface is not. The
current window is a 224 px text rail beside a single-column scroll of five
equally weighted panels, with a pinned action bar at the foot. It reads as a
settings form that happens to contain a transcoder. A media professional should
instead see, at a glance: what media is loaded, what will be made from it, where
it will be written, and whether the job is running — and should reach the tools
(presets, queue, preview, logs) without them competing with that answer.

This replaces the previous "true-black flat" direction. That direction is
design history, not a requirement. `DESIGN.md` will be rewritten to describe
this system, not the old one.

## Goals

- A genuine workstation composition: a top application bar, a compact icon
  activity rail for tools, a task-focused workspace, a persistent job dock,
  and a status bar — five distinct regions, each with one job.
- One primary workflow legible without scrolling: **source → inspect → preset
  → output → destination → queue**.
- A distinctive identity: IBM Plex Sans + IBM Plex Mono (bundled, offline), a
  graphite dark theme that is not pure black, a warm paper light theme, a
  confident teal signal accent, small radii, restrained depth, and a consistent
  inline SVG icon set.
- Native web primitives where they are Baseline Widely available: `<dialog>`
  for the ten secondary surfaces, semantic landmarks, real tabs, logical
  properties, and CSS Grid with named areas.

## Non-goals / anti-goals

- Not a "SaaS dashboard", not a settings form with a sidebar of labels, not a
  single long scroll of cards, not a static mockup.
- No new framework, bundler, runtime dependency, or remote asset request.
- No hardcoded English interface strings: all fixed chrome goes through the
  window's `data-i18n` catalogue; tooltips continue to come from the Rust
  catalog descriptions.
- No adoption of `focusgroup`, `popover`, `light-dark()`, `anchor-positioning`,
  or `closedby` as required behaviour: these are not Baseline Widely available
  and Tauri's WebKitGTK build may not support them. Keep the existing manual
  roving tabindex for tabs; use `showModal()`/`show()` and graceful Esc.

## Constraints

- The Rust backend is not rewritten. All 89 Tauri commands, view-model field
  names, event names, and the worker contract stay as they are.
- All 199 ids in `index.html` are a contract: `app.js` either queries them
  literally or builds the name at runtime (e.g. `stepAnswer-*`, control ids
  from view models via `applyChoice`). Every id survives the restructure.
- Surface contract (`surface-<id>`, `<id>-label`, `<id>close`,
  `surface-<id>-body`, `buttonId`, `openable`, `badge`) stays intact.
- Theme/direction resolution stays in Rust and arrives as `data-theme` + `dir`;
  localStorage replay stays for the first paint.
- Test suite: 1041 tests; clippy `-D warnings`; fmt check.

## Approaches considered

1. **Recolour the existing true-black window.** Rejected: it keeps the exact
   structure the brief calls unsatisfactory and produces another superficial
   restyling.
2. **Three-pane creative-tool layout (chosen).** The workspace becomes a
   source/inspection region plus the output tabs; a right dock holds the
   persistent job summary, destination, primary action, and live queue; a top
   bar and a small icon rail hold global commands. It uses desktop width,
   keeps the main flow on one screen, and gives progress a permanent home.
   Trade-off: more HTML restructuring, but the rendered result is a different
   product rather than a different skin.
3. **Full step-based wizard with a contextual inspector.** Rejected: it hides
   content behind clicks and repeats the step-switcher failure the project
   already moved away from.

## The chosen design

### Regions

```
┌───────────────────────────────────────────────────────────────────────────┐
│ app bar: mark + VISE | job name & format | commands | display menu        │
├─────┬───────────────────────────────────────────────────────┬─────────────┤
│ icon│ workspace (one scroll)                                │ job dock    │
│ rail│  source bar → media inspector → presets → output tabs │ summary     │
│     │                                                       │ destination │
│     │                                                       │ queue live  │
├─────┴───────────────────────────────────────────────────────┴─────────────┤
│ status bar: ready state | media tooling | problems | live encode          │
└───────────────────────────────────────────────────────────────────────────┘
```

- **App bar (56 px)** — brand mark and wordmark; the job title and the format
  readout (`jobFormat`) are the centre of attention; quick commands (Presets,
  Preferences) and the display menu (theme/direction) on the end. The display
  menu is a plain `hidden` panel toggled by a button, because `popover` is not
  a safe baseline for every WebView.
- **Activity rail (56 px)** — icon-only buttons built from `window_catalog`,
  one per openable surface, with `aria-label`/`title` from the Rust hint and a
  badge. The wordmark stays in the app bar, so the rail is purely tools. No
  `.step` navigation: steps are sections in the workspace.
- **Workspace** — a compact source bar (drop zone, path, Open, Scan, status),
  a media inspector with a technical stat strip and the title/range/track
  selectors in two columns, a preset launcher, and the eight-tab output
  configuration. `data-i18n` on section headings; the outputs tabs keep every
  field, id, hint and validation state.
- **Job dock (320 px)** — the persistent job summary, destination path
  (`outPath`), the primary `queueBtn`, the secondary `openSelectionBtn`,
  `queueHint`, and a live queue readout that reports state without raw logs.
  Collapses below 1200 px into a toggle from the app bar; stacks below 900 px.
- **Status bar (28 px)** — a `role="status"` strip with ready/scanning/encoding
  state, encoder facts, problem count, and live progress.

### Visual system

- **Colour** — dark: ground `#0B0E11`, panel `#131820`, raised `#1C232C`,
  line `#27303A`, text `#E9EEF3`, muted `#93A0AC`, signal `#37D6C6`.
  light: ground `#EDEEEA`, panel `#FFFFFF`, raised `#F3F4F1`, line `#D7DBD4`,
  text `#182027`, muted `#5D6A73`, signal `#0B8579`. Status colours are tints
  on their own backgrounds and always carry a word or icon as well as a hue.
- **Type** — IBM Plex Sans (400/500/600/700) for words, IBM Plex Mono for
  technical readings. Scale 11/12/13/14/16/20/26 px; tabular numerals on all
  readings. Sentence case, no decorative capitals, tracking near zero.
- **Depth** — small radii (6/10/14 px), 1 px lines that are one step above the
  surface, and a single soft shadow only for floating panels/dialogs. No
  gradient decoration, no glass, no pill buttons everywhere.
- **Motion** — 120–180 ms transitions on interactive state only, gated behind
  `@media (prefers-reduced-motion: no-preference)`.
- **Icons** — one inline SVG sprite with `<use href="#i-…">`, stroke width
  1.7, 18 px in the rail, 16 px in buttons, `currentColor`. No emoji.

### Modern web guidance applied

- `css-layout`: named `grid-template-areas` for the page shell, `minmax()` and
  `fr` tracks, logical properties, `min-inline-size: 0` on flex items that hold
  paths, `aspect-ratio` where media reserves space.
- `dialog`: native `<dialog>` for all ten surfaces, `showModal()` for modal and
  `show()` for non-modal, `::backdrop`, no manual focus trap or `aria-modal`,
  `cancel` intercepted so Rust state stays in sync — purpose-built for
  WebKitGTK where `closedby` may not exist.
- `tabs`: keep the single-row scrolling tablist and the manual roving tabindex
  (do not adopt `focusgroup`, which is unsupported in Safari/Firefox), style
  selection from `aria-selected` with a non-colour underline.
- `forms`: labels before controls, `aria-describedby` for hints, `aria-invalid`
  for failed fields, fieldsets with legends, `type="button"` on helpers.
- `accessibility`: five landmarks, one `h1`, sequential headings, skip link,
  live status regions, no tables for layout, focus order matches visual order.
- `dark-mode`: `<meta name="color-scheme" content="light dark">` and
  `color-scheme` on `:root`; theme still resolved by the window.
- `icons`: one consistent inline SVG set, masked/stroked through
  `currentColor`, 3:1 contrast in both themes.
- `motion`: reduced-motion by default, opt-in transitions.

## Implementation order

1. Bundle IBM Plex Sans/Mono + OFL; add the icon sprite.
2. Restructure `index.html` into the five regions, preserving every id.
3. Rewrite `styles.css` as the new design system, with container/media
   adaptation and both themes.
4. Make focused `app.js` changes: native-dialog surfaces, icon rail buttons,
   display-menu toggle, dock queue rendering, status bar updates.
5. Retrieve further guidance as each region is implemented (per GOAL Phase 5).
6. Verify in the real window at four desktop sizes, both themes, RTL, German,
   empty/loaded/error/queue states; fix what the screenshots reveal.
7. Run cargo test/clippy/fmt; update `DESIGN.md`, `README.md`, screenshots,
   `docs/ROADMAP.md`; commit on a task branch and merge after verification.

## What is left to the builder

All of it. This document fixes the direction and the contracts, not the exact
markup of every field. Fields, view-model bindings, and i18n keys follow the
existing catalogue and are not redesign decisions.
