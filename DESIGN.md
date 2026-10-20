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
┌──────────────┬──────────────────────────────────────────────┐
│ Vise         │  ┌────────────────────────────────────────┐  │
│ Transcoder   │  │  Source                                 │  │
│              │  │  ...the card for the selected step...   │  │
│  1 Source    │  └────────────────────────────────────────┘  │
│    film.mkv  │                                              │
│  2 Titles    │                                              │
│    Title 1   │                                              │
│  3 Range     │                                              │
│    0:00–0:12 │                                              │
│  4 Tracks    │                                              │
│  5 Output    │                                              │
│    MP4 · H264 │                                              │
│              │                                              │
│  Queue       │                                              │
│  Activity    │                                              │
│  …           │                                              │
│  Theme       │                                              │
│  Direction   │                                              │
├──────────────┴──────────────────────────────────────────────┤
│  Output path [...............]  [Add to queue] [Choose…]    │
└─────────────────────────────────────────────────────────────┘
```

- **The rail is the pipeline.** The steps are in the order the work is done in,
  so the shape of the job is legible rather than inferred from where the cards
  happen to sit. A step that is waiting on an earlier one is dimmed rather
  than hidden, so the reader always knows what comes next.
- **Each step carries its answer.** The source's name, the title that is
  chosen, the range that is set, the tracks that are on, the container and the
  codecs the output will be. The rail is therefore a summary of the whole job,
  not only a way to move around it.
- **One step at a time.** The work area shows the step that is chosen. This is
  what gives the eight output tabs the room they need; they were the last card
  in a scroll, below four cards of setup.
- **The windows are in the rail** because they are chrome rather than work,
  and the appearance controls are at its foot because they are settings a
  reader changes once.
- **The action bar is pinned to the foot** of the work area. Where the file is
  written and the button that writes it are always together and always
  reachable, whatever the step on screen.
- **Below the width where a rail and a card both fit**, the rail becomes a
  strip along the top with the steps in a row. The strip is as many bands
  tall as it needs and no more, so the work area keeps the rest of the height.

### Spacing

A 4-unit base, used for the relationship it expresses rather than for a round
number.

| Token | Value | Used for |
|---|---|---|
| `--space-1` | 4px | inside one control |
| `--space-2` | 8px | between two controls that act together |
| `--space-3` | 12px | between the parts of one card |
| `--space-4` | 16px | a card's own padding |
| `--space-5` | 24px | between two groups |
| `--space-6` | 32px | between two bands of the page |
| `--space-7` | 48px | the page's own margin |

One value repeated everywhere gives every element the same weight, and a page
where nothing is louder than anything else has no hierarchy at all.

### Grid

`.shell` is two columns: the rail at 280px and the work area taking the rest.
The rail is as wide as the step names and their answers need and no wider,
because the work area is where the reading happens. The rail and the work
area are each their own scroll, so a long list of windows never pushes the
action bar off the foot of the window.

## Colour

Two token sets, one per theme, decided by the window rather than by the page:
the operating system's preference is resolved in Rust and sent down as
`data-theme`, so the stylesheet holds colours and no preference logic.

| Role | Light | Dark |
|---|---|---|
| `--bg` | `#f4f5f7` | `#14161a` |
| `--surface` | `#ffffff` | `#1d2026` |
| `--surface-sunken` | `#eceef2` | `#262a31` |
| `--text` | `#16181d` | `#e9ebef` |
| `--muted` | `#4d525c` | `#a7aeb9` |
| `--accent` | `#1a56c4` | `#8ab4ff` |
| `--accent-soft` | `#e6eefb` | `#1e2b42` |
| `--border` | `#d5d9e1` | `#333844` |
| `--border-strong` | `#7d8494` | `#707886` |
| `--danger` | `#a8201a` | `#ff9d95` |
| `--success` | `#14663a` | `#86ddaa` |
| `--warning` | `#7a4f00` | `#e8bd63` |

The accent is reserved for the one thing that is selected or active. A row that
is chosen carries the accent wash and a weight, and — in a list — a dot beside
its name. It is never a thick coloured edge on one side, which reads as a tag
rather than as a selection and is the mark of a generated interface.

The focus ring is two colours: a mark against the control and a halo against
whatever is behind it. One colour alone disappears on one of the two themes.

## Type

The platform's own sans stack, at three sizes and three weights.

- **14px** is the body. Nothing in a working tool is set larger than it needs
  to be.
- **12px** is the supporting voice: hints, counts, and the summary lines.
- **17px** is the application name and nothing else.
- Weight 600 marks the chosen item and a heading. Nothing is bolder.

A line of supporting text is capped at about 62 characters, which is the
measure the eye reads comfortably.

## Components

| Component | What it is |
|---|---|
| **Rail** | The window's chrome: the name, the steps, the windows, and the settings. It scrolls on its own and never moves the work area. |
| **Step** | A row in the rail: the number of the step, its name, and the answer it has reached. The step on screen is marked with the accent and a leading edge; a step waiting on an earlier one is dimmed. |
| **Card** | A panel with a heading, a hint on the right, and its content below. `--surface`, a 1px border, a 10px radius, 16px of padding. |
| **Choice** | A radio in a label: the control, the name, and the reason beneath it. The chosen one carries the accent wash. |
| **Switch** | A checkbox with a name and its reason beneath, in a row that wraps. |
| **Field** | A label, a control, and a hint beneath it, in a column. |
| **Tab strip** | The same strip the preset catalogue and the preferences use, so the arrow keys, the one tab stop, and the wrapping all behave identically. A tab that needs something fixed says so with a dot rather than a colour alone. |
| **List row** | A title, a chip, and a line of facts. The chosen row carries the wash, the weight, and a dot. |
| **Chip** | A short label: a file kind, an encoder, a hardware marker. |
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
- Focus is visible on everything, keyboard order follows the visual order, and
  motion is reduced when the reader asks for less of it.
- A control that cannot be used says which capability is missing, rather than
  being hidden or quietly doing nothing.

## Rules that hold

- No gradient text. Emphasis comes from weight or size.
- No glass or blur as decoration.
- No thick coloured edge on one side of a card, row, or callout.
- No hard offset shadows outside a world that has chosen them.
- No monospace as a costume; monospace is for code, paths, and measurements.
- No emoji or unicode glyph standing in for an icon.
- No section numbers unless the sequence itself carries information.
- A card is not a container for everything: grouping is done with proximity
  first, and a nested card is always wrong.
