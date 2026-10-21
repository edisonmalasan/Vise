# Design System

<!-- impeccable:design-schema 1 -->

The system the Vise Transcoder interface is built from. It is an *Operate*
surface: a working tool, where predictability, density, and reaching the
controls quickly matter more than expression.

## Character

Quiet, technical, and plain. The interface is a workspace, not a poster: it
gets out of the way, uses the platform's own conventions, and spends its
attention on the thing being configured. Nothing announces itself; a control
that is not in use stays quiet.

## Layout

The window is a rail and a work area, not a document.

```
┌────────────┬──────────────────────────────────────────────────────────┐
│ » Vise     │  Big Buck Bunny (2008).mkv                               │
│   Transcoder│  Big Buck Bunny · 1920×1080 · 23.976 fps · 9:57          │
│            │  · MP4 · H264 video · AAC audio                          │
│ Queue      │  ──────────────────────────────────────────────────────── │
│ Queue sel. │  SOURCE              │  ┌────────────────────────────┐  │
│ Activity   │  Big Buck Bunny      │  │ drop, path, status         │  │
│ Preview    │  (2008).mkv          │  │ recent sources             │  │
│ Problems   │  ────────────────────┤  └────────────────────────────┘  │
│ About      │                      │                                  │
│ Presets    │  TITLES              │  ┌────────────────────────────┐  │
│ Preferences│  Big Buck Bunny      │  │ the list                   │  │
│ Diagnostics│  ────────────────────┤  └────────────────────────────┘  │
│            │                      │  ┌────────────────────────────┐  │
│ ────────── │  RANGE AND ANGLE     │  │ the modes                  │  │
│ Appearance │  00:00 – 09:56       │  └────────────────────────────┘  │
│ Theme      │  ────────────────────┴  ┌────────────────────────────┐  │
│   ◉ system │                          │ OUTPUT SETTINGS              │  │
│   ○ light  │                          │ Summary Dimensions Filters …  │  │
│   ○ dark   │                          └────────────────────────────┘  │
│ Direction  │                                                          │
├────────────┴──────────────────────────────────────────────────────────┤
│  Output path [...............]  [Add to queue] [Choose titles to queue…] │
└────────────────────────────────────────────────────────────────────────┘
```

- **Nothing is hidden behind a click.** Every control is on screen. The work
  is one scroll with the five steps in the order the work is done in, so the
  order the panels are read in is the order the work happens in, and the reader
  never switches anything to reach a control.
- **The rail holds the application's own destinations, and only those.** The
  name and mark, the windows the application can open, and the settings that
  change how it looks. A navigation list is a list of *other* screens: a
  sidebar that scroll-spies one long page asks the reader to hold a second map
  of the page in their head, and the work area already says where it is.
- **The work area is a label column beside the panels.** Each step names
  itself and states what has been decided for it, in a 240px column, beside
  the panel that decides it. The job's summary is here rather than in a rail,
  and it is at 13px in a reading face rather than squeezed into 12px at the
  edge of the window.
- **The title row answers the screen's question.** The name of the object the
  window is working on, and one grey line under it saying what is being made:
  the title, the frame, the rate, the length, and the format. The format is
  the element only this product has, and it is set in the trade's own words.
- **One column of steps.** No two panels share a row, so no row can end
  ragged, and each panel is as tall as its content.
- **The source panel is quiet once a source is chosen.** The drop target
  becomes a line that still takes a drop, because it is how the source is
  replaced, but it no longer takes a third of the first screen and pushes the
  work below the fold.
- **The windows are in the rail** because they are chrome rather than work,
  and the appearance controls are at its foot because they are settings a
  reader changes once.
- **The action bar is pinned to the foot** of the work area. Where the file is
  written and the button that writes it are always together and always
  reachable, whatever the length of the work above them.
- **Below the width where a rail and a panel both fit**, the rail becomes a
  strip along the top with the windows and the settings in rows of their own,
  and each step stacks its label above its panel.

### Spacing

A 4-unit base, used for the relationship it expresses rather than for a round
number.

| Token | Value | Used for |
|---|---|---|
| `--space-1` | 4px | inside one control |
| `--space-2` | 8px | between two controls that act together |
| `--space-3` | 12px | between the parts of one panel |
| `--space-4` | 16px | a panel's own gap, and one step's padding |
| `--space-5` | 24px | between two groups |
| `--space-6` | 32px | between two bands of the page |

One value repeated everywhere gives every element the same weight, and a page
where nothing is louder than anything else has no hierarchy at all. One gap
value is used horizontally and vertically, so the rhythm is the same in both
directions.

### Grid

`.shell` is two columns: the rail at 224px and the work area taking the rest.
The rail is as wide as a list of destination names needs and no wider, because
the work area is where the reading happens. The rail and the work area are
each their own scroll, so a long list of windows never pushes the action bar
off the foot of the window.

The work area holds one column of at most 1152px, centred, with a gutter of at
least 20px on each side. A step is a two-column row: a 240px label column and
a panel that takes the rest. The column and the gutter are the same on the
title row and on the action bar, so the header, the panels and the foot share
one left edge.

## Colour

Two token sets, one per theme, decided by the window rather than by the page:
the operating system's preference is resolved in Rust and sent down as
`data-theme`, so the stylesheet holds colours and no preference logic.

| Role | Light | Dark |
|---|---|---|
| `--ground` | `#f9fafb` | `#000000` |
| `--surface` | `#ffffff` | `#111111` |
| `--raised` | `#f0f1f3` | `#2a2a2a` |
| `--active` | `#eeeeee` | `#1c1c1c` |
| `--border` | `#e8e8e8` | `#1a1a1a` |
| `--rule` | `#f0f0f0` | `#161616` |
| `--control` | `#d1d5db` | `#444444` |
| `--text` | `#111111` | `#ffffff` |
| `--muted` | `#5f6368` | `#b4b4b4` |
| `--faint` | `#6b6b6b` | `#949494` |
| `--accent` | `#0a6ba8` | `#4cb3f0` |
| `--primary` | `#1d1f20` | `#ffffff` |
| `--positive` | `#247b50` | `#3fc07a` |
| `--negative` | `#c4242a` | `#e5566f` |
| `--warning` | `#92400e` | `#e5a93b` |

**Elevation is a lightness step alone.** A panel is a step above the ground and
a nested panel a step above that; nothing at rest carries a border or a
shadow, because a line reads as an edge to be crossed. A border or a shadow,
never both, and only on what floats: a surface, a menu, a tooltip.

**Colour is spent on the data, and on one selected state.** The accent marks
the things that have been chosen — a chosen title, a chosen track, the work in
progress — and nothing else. It never touches a button, a tab or a navigation
row. The primary control is made primary by inversion instead: a filled pill
with the label turned the other way out.

The focus ring is two colours: a mark against the control and a halo against
whatever is behind it. One colour alone disappears on one of the two themes.

## Type

One grotesk and one monospace, at five sizes.

- **Inter** is the text face, bundled with the application rather than
  fetched from a website: a desktop program that asks a website for its
  typeface has a typeface on the days the website is unreachable, and a
  program that is installed should not need a network to look right. The
  languages this build ships that Inter has no glyphs for fall back to the
  system's own face for those letters, which is what a system face is for.
- **A monospace** is the second face, and it has one job: the figures that
  describe media — a time code, a resolution, a frame rate, a bit rate, a
  container, a codec. Those are readings off a scale rather than prose, and a
  time code is not proportional text.
- **20px** is the title. **16px** is a step's name. **14px** is the body.
  **13px** is a label, a note, and an answer. **12px** is for the marks that
  are almost not text: a count, a table heading, a level. Nothing is smaller
  than 12, and most of the page is at 13 or above.
- Sentence case everywhere, tracking zero, and no capitals used as decoration.
- Weight 600 marks the chosen item and a heading. Nothing is bolder.

A line of supporting text is capped at about 62 characters, which is the
measure the eye reads comfortably.

## Components

| Component | What it is |
|---|---|
| **Rail** | The window's chrome: the name and mark, the windows, and the settings. It scrolls on its own and never moves the work area. |
| **Title row** | The name of the object the window is working on, and the grey line under it saying what is being made. |
| **Step** | A row of the work: a label column carrying the step's name and the answer it has reached, beside the panel that decides it. |
| **Panel** | A card with no border and no shadow: `--surface`, a 20px radius, 20px of padding. |
| **Choice** | A radio in a label: the control, the name, and the reason beneath it. The chosen one carries the weight; the control itself is the mark. |
| **Switch** | A checkbox with a name and its reason beneath, in a row that wraps. |
| **Field** | A label, a control, and a hint beneath it, in a column. |
| **Tab strip** | The same strip the preset catalogue and the preferences use, so the arrow keys, the one tab stop, and the wrapping all behave identically. The tab on screen is marked by a neutral fill and by weight, never by the accent. A tab that needs something fixed says so with a dot rather than a colour alone. |
| **List row** | A title, a chip, and a line of facts. The chosen row carries the wash, the weight, and a dot. |
| **Chip** | A short label: a file kind, an encoder, a hardware marker. A pale tint with the ink of the same hue. |
| **Action bar** | The pinned foot: the path field and the button that uses it. |
| **Surface** | A secondary window, centred over the application, dimming what is behind it. |

## Behaviour

- The theme and the layout direction are resolved by the window and reported
  back, so the interface never disagrees with itself about which system it
  followed.
- Right-to-left is a supported layout direction, not a mirror of a
  left-to-right design: every offset, padding, and text alignment is a logical
  property, and the two places that are physical are marked in the stylesheet
  and are deliberate.
- The words are the window's. An element says which message it wants and the
  window's language answers; the page holds no text of its own, so a language
  the build does not ship costs a sentence in the base language rather than a
  blank control.
- Focus is visible on everything, keyboard order follows the visual order, and
  motion is reduced when the reader asks for less of it.
- A control that cannot be used says which capability is missing, rather than
  being hidden or quietly doing nothing.

## Rules that hold

- No gradient text. Emphasis comes from weight or size.
- No glass or blur as decoration.
- No thick coloured edge on one side of a card, row, or callout.
- No hard offset shadows outside a world that has chosen them.
- No monospace as a costume; monospace is for the readings that describe
  media, and for paths and code.
- No emoji or unicode glyph standing in for an icon.
- No section numbers unless the sequence itself carries information.
- A panel is not a container for everything: grouping is done with proximity
  first, and a nested panel is always wrong.
- The accent is on the data, never on the chrome.
