# Design System

<!-- impeccable:design-schema 1 -->

The system the Vise Transcoder interface is built from. It is an **Operate**
surface: a working tool where an operator watches media move through a pipeline.
Predictability, density, reachability and readable technical data come before
expression. The reference is a professional media workstation, not a website
or a settings form.

## Character

Deliberate, calm and capable. The window looks like an instrument: surfaces are
graphite in the dark theme and warm paper in the light theme, one hue marks
action and live state, and every number is a reading off a scale. Nothing
shouts, nothing is ornament, and nothing needs a paragraph of copy to explain
itself.

The product's identity is carried by four things: the double-chevron mark, the
format readout (`MP4 · H264 video · AAC audio`), IBM Plex as the voice, and the
persistent job dock.

## Layout

The window is five regions with one job each, laid out with named grid areas.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ app bar  mark · job name & format                 presets · dock · display │
├──────┬───────────────────────────────────────────────────────┬─────────────┤
│      │ SOURCE   drop / path / open / scan / status           │ JOB         │
│      │                                                       │ summary     │
│ acts │ INSPECTOR  titles + range        tracks               │ destination │
│      │                                                       │ commit      │
│      │ PRESETS    catalogue launcher                         │ live queue  │
│      │                                                       │             │
│      │ OUTPUT     summary · dimensions · … · metadata        │             │
├──────┴───────────────────────────────────────────────────────┴─────────────┤
│ status  ready state · media facts · problems · live encode                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

- **App bar (56 px)** — the brand and the object being worked on. The job name
  and its format are the centre of attention; global commands sit at the end.
- **Activity rail (56 px)** — icon-only buttons for the windows the build
  offers, built from `window_catalog`, each with an accessible name and a hint
  from the window. Badges carry counts. It is tools, not a table of contents:
  the work area already names every step of the job.
- **Workspace** — one scroll containing the source, the media inspector, the
  preset launcher and the eight-tab output configuration. Every control is on
  screen in one scroll; nothing is hidden behind a step switcher.
  - The **source** is a compact band: a drop zone that becomes a single line
    once a source is chosen, the path field, and the live scan state.
  - The **inspector** puts titles and range on the left and tracks on the right,
    so what is in the source and what was chosen read together.
  - The **output** keeps the full field set, hints and validation of the eight
    tabs, in one non-wrapping tab strip.
- **Job dock (320 px)** — the permanent answer to "what will be made and
  where": source, format, destination, path field, primary action and the live
  queue. It never scrolls out of reach.
- **Status bar (28 px)** — ready/scanning/encoding state, media facts, problem
  count and an inline progress bar, announced through a live region.

**Progressive disclosure** is by region, not by hiding: common settings are on
the Summary tab, advanced settings are one tab away, and the dock holds only
the facts needed to commit. The primary workflow — choose media → inspect →
select preset → configure → set destination → queue — is legible from top to
bottom and left to right.

Below 1180 px the dock becomes a compact strip under the workspace so the
destination and commit action stay reachable. Below 900 px the rail becomes a
top strip and the inspector stacks. Either region can be toggled from the app
bar.

### Spacing

A four-unit base, used for the relationship it expresses.

| Token | Value | Used for |
|---|---|---|
| `--s1` | 4px | inside a control, between a label and its hint |
| `--s2` | 8px | between controls that act together |
| `--s3` | 12px | between the parts of one panel |
| `--s4` | 16px | a panel's padding |
| `--s5` | 20px | a workspace's top padding |
| `--s6` | 24px | between regions of the workspace |
| `--s7` | 32px | workspace bottom padding |

### Grid

`.appbody` is `56px | minmax(0,1fr) | 320px` with areas `rail work dock`; the
page itself is `56px | minmax(0,1fr) | 28px`. The workspace holds one column
with a gutter that grows with the window and is never less than 16 px. The
inspector is two columns that stack when the container cannot fit them. Paths
live in flex items with `min-inline-size: 0` so a long Windows path never
forces horizontal overflow.

## Colour

Two token sets, one per theme, decided by the window and reported as
`data-theme`. The stylesheet holds colours and no preference logic. The media
query is only the state before the first answer arrives.

| Role | Light | Dark |
|---|---|---|
| `--ground` | `#edefe9` | `#0a0d10` |
| `--panel` | `#ffffff` | `#131920` |
| `--raised` | `#f5f6f2` | `#1b232c` |
| `--sunken` | `#f9faf7` | `#0e1318` |
| `--line` | `#d7dbd2` | `#25303a` |
| `--line-strong` | `#b7c0b4` | `#35434f` |
| `--text` | `#172026` | `#e7edf3` |
| `--muted` | `#5b6a73` | `#94a3b0` |
| `--faint` | `#79858d` | `#6f7f8c` |
| `--signal` | `#0b8579` | `#35d6c3` |
| `--good` | `#1e8a57` | `#52d18c` |
| `--bad` | `#c13742` | `#ff6b77` |
| `--warn` | `#9a6510` | `#e8b55a` |

The dark theme is graphite, not pure black: a surface step is visible without
a border, and a border is one step above the surface. The light theme is warm
paper rather than a blue-grey SaaS white.

**The signal colour is spent on action and live state** — the primary button,
the selected tab, the chosen option, the running job. It is never used as
decoration and never on a whole card. Status always carries a word or a shape
as well as a hue.

Depth is small radii (6/10/14 px), 1 px lines, and one soft shadow reserved for
floating surfaces: the display menu and native dialogs. No gradient
decoration, no glass on cards, no pill on every button.

## Type

Two bundled faces, offline by construction:

- **IBM Plex Sans** carries the words. Variable weight, 400–700 in use.
- **IBM Plex Mono** carries the readings: a time code, a resolution, a frame
  rate, a bit rate, a container, a codec, a path. It is not a costume applied
  to prose.

The scale is 11/12/13/14/16/20/26 px. Body text is 14 px, labels and hints
12 px, readings 12–13 px, section names 13–14 px, the job name 16 px. Every
reading uses tabular figures. Sentence case throughout, tracking near zero,
no decorative capitals, weight 600 for emphasis and nothing bolder.

A line of supporting text is capped at roughly 62 characters.

## Icons

One inline SVG sprite, `<use href="#i-…">`, stroke-based at 1.5–1.9 px, 18 px
in the rail and 16 px in buttons, drawn in `currentColor` so they follow the
theme. No emoji, no image files, no remote requests. Every icon has a text
alternative: the rail buttons carry `aria-label` and `title`.

## Components

| Component | What it is |
|---|---|
| **App bar** | Brand, the job's name and format, and global commands. |
| **Activity rail** | The build's secondary surfaces as icon buttons with badges. |
| **Source band** | Drop zone, path, Open, Scan, live status, recents. |
| **Inspector** | Titles and range beside tracks, with technical readings in mono. |
| **Preset launcher** | One visible route into the 138-preset catalogue. |
| **Output tabs** | A single scrolling row; selection marked with a shape from `aria-selected`. |
| **Job dock** | Summary, destination, primary action and the live queue. |
| **Status bar** | One `role="status"` line with state, facts and progress. |
| **Native dialog** | Each secondary surface is a `<dialog>`, modal or modeless as the window says. The platform owns the backdrop, focus and Esc. |
| **Chip** | A short technical label: file kind, encoder, hardware state. |
| **Choice** | A native radio in a label; the chosen name takes the signal colour and weight. |

## Behaviour

- The theme and direction are resolved by the window. The page applies what it
  is given and decides nothing; the previous answer is replayed from local
  storage before first paint so the window does not flash the other theme.
- Right-to-left is a real layout: every offset, padding and alignment is a
  logical property. English fallback sentences in a mirrored window use
  `unicode-bidi: plaintext`, so their punctuation stays at the end until the
  window's translation arrives.
- The words are the window's. An element names the message it wants with
  `data-i18n` and the window's language answers; a missing message costs a
  sentence in the base language, never a blank control.
- Keyboard order follows visual order, focus is visible on everything, and
  motion is an opt-in behind `prefers-reduced-motion: no-preference`.
- A disabled control says why in the nearby hint, not only in a tooltip.

## Rules that hold

- The dock and the commit action are never scrolled out of reach.
- No step switcher: every step of the job stays in the one workspace scroll.
- No focus trap, backdrop element or `aria-modal` on a native dialog.
- No `focusgroup`, `popover`, `light-dark()`, anchor positioning or `closedby`
  as a requirement — they are not Baseline Widely available for WebKitGTK.
- No hardcoded English in new chrome; new messages go through the catalogue.
- No emoji as icons, no decorative gradients, no tables used for layout.
