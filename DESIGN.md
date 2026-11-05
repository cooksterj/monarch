---
name: Monarch
description: A terminal multiplexer that shows which AI coding agent needs you.
colors:
  wing: "#e8761b"
  wing-edge: "#1e1612"
  wing-spot: "#f5f0e6"
  claude: "#d97757"
  dark-accent: "#e8a33d"
  dark-accent-text: "#1a1a1a"
  dark-bar: "#2b2d3a"
  dark-bar-text: "#d0d3e0"
  dark-working: "#5fafff"
  dark-blocked-from: "#c8a000"
  dark-blocked-to: "#7b4f1d"
  dark-done-from: "#a8e6a3"
  dark-done-to: "#2ee65b"
  dark-error: "#ff5f5f"
  dark-added: "#7ed37e"
  dark-removed: "#f07178"
  dark-hunk: "#6fb3d2"
  light-accent: "#c25e00"
  light-accent-text: "#ffffff"
  light-bar: "#e4e2dc"
  light-bar-text: "#2b2b2b"
  light-working: "#1f6fd1"
  light-blocked-from: "#a67c00"
  light-blocked-to: "#5c3a12"
  light-done-from: "#6cbf6a"
  light-done-to: "#0a8a2a"
  light-error: "#c62828"
  light-added: "#1a7f37"
  light-removed: "#b31d28"
  light-hunk: "#0550ae"
  capture-ground: "#14151c"
  capture-text: "#d5d8e4"
  capture-strip: "#0c0d12"
  capture-border: "#3a3d4e"
typography:
  capture-cell:
    fontFamily: "'JetBrainsMono Nerd Font Mono', 'JetBrains Mono', Menlo, 'DejaVu Sans Mono', monospace"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: "20px"
  capture-key:
    fontFamily: "'JetBrainsMono Nerd Font Mono', 'JetBrains Mono', Menlo, 'DejaVu Sans Mono', monospace"
    fontSize: "15px"
    fontWeight: 600
  capture-caption:
    fontFamily: "-apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
    fontSize: "17px"
    fontWeight: 400
    lineHeight: 1.3
  diagram-heading:
    fontFamily: "-apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
    fontSize: "24px"
    fontWeight: 600
  diagram-state:
    fontFamily: "-apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
    fontSize: "22px"
    fontWeight: 600
  diagram-label:
    fontFamily: "-apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
    fontSize: "19px"
    fontWeight: 600
  diagram-body:
    fontFamily: "-apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
    fontSize: "19px"
    fontWeight: 400
rounded:
  key-chip: "5px"
  ring: "6px"
  capture: "10px"
  diagram: "14px"
spacing:
  cell-width: "9px"
  cell-height: "20px"
  capture-pad: "10px"
  caption-strip: "44px"
  caption-strip-phone: "96px"
components:
  status-bar:
    backgroundColor: "{colors.dark-bar}"
    textColor: "{colors.dark-bar-text}"
  shown-tab:
    backgroundColor: "{colors.dark-accent}"
    textColor: "{colors.dark-accent-text}"
  capture-frame:
    backgroundColor: "{colors.capture-ground}"
    textColor: "{colors.capture-text}"
    typography: "{typography.capture-cell}"
    rounded: "{rounded.capture}"
    padding: "{spacing.capture-pad}"
  capture-caption:
    backgroundColor: "{colors.capture-strip}"
    textColor: "{colors.capture-text}"
    typography: "{typography.capture-caption}"
    height: "{spacing.caption-strip}"
  capture-key-chip:
    backgroundColor: "{colors.dark-accent}"
    textColor: "{colors.dark-accent-text}"
    typography: "{typography.capture-key}"
    rounded: "{rounded.key-chip}"
    padding: "3px 9px"
  diagram-panel:
    backgroundColor: "{colors.capture-ground}"
    textColor: "{colors.capture-text}"
    rounded: "{rounded.diagram}"
---

# Design System: Monarch

## Overview

**Creative North Star: "The Field Mark"**

A field mark is the feature that identifies a species at a glance. Monarch has two: the butterfly identifies the project, and a coloured glyph identifies each agent's state. Everything else belongs to someone else. The terminal's font and ground belong to the host, pane contents belong to the program, and the README's type and page belong to GitHub.

The source of truth is code. Tokens live in `crates/monarch-core/src/theme/builtin.rs`, and the `dark-` and `light-` colours above are its `monarch-dark` and `monarch-light` themes. Catppuccin, Tokyo Night and Gruvbox are defined there too, and only their chrome colours differ. They are not repeated here.

**Key Characteristics:**
- Dense, flat, cell-based chrome with no shadows and square box-drawing borders.
- State shown by glyph and colour together, and still legible with no colour at all.
- One fixed identity palette across every theme.
- Real captures of the real binary as the only product imagery.

## Colors

A warm butterfly over cool terminal chrome, where saturated colour is reserved for state, diffs and identity.

### Primary
- **Monarch Wing** (`wing`): the butterfly's wing cells, in the mark and the welcome sprite. Identity only.
- **Wing Edge** (`wing-edge`): veins, wing borders, body and antennae.
- **Wing Spot** (`wing-spot`): margin spots and body dots, and the logo's keyline.

### Secondary
- **Chrome Accent** (`dark-accent`, `light-accent`): the shown tab, the waiting prefix, the focused pane's border, sidebar headings and commits to push. Text on it takes `dark-accent-text` or `light-accent-text`. Captures also use it for annotations.

### Tertiary
- **Working Blue** (`dark-working`, `light-working`): the pulsing dot.
- **Blocked Amber** (`dark-blocked-from` to `dark-blocked-to`, and the light pair): the fading diamond. The `from` colour also marks commits to pull.
- **Done Green** (`dark-done-from` to `dark-done-to`, and the light pair): the check that brightens on finishing, then rests in `from` once seen.
- **Error Red** (`dark-error`, `light-error`): a failed agent's cross.
- **Diff Colours** (`dark-added`, `dark-removed`, `dark-hunk`, and the light set): diff lines and checkout badges. The hunk colour is for diff hunk headers only.
- **Claude Terracotta** (`claude`): Claude Code's icon, shared by every theme. Codex's icon takes the theme's bar text colour.

### Neutral
- **Bar** (`dark-bar`, `light-bar`, with `dark-bar-text`, `light-bar-text`): the status bar. Apart from the bar, the shown tab, reverse video and the welcome sprite's half blocks, the app paints no ground, so the host's background shows through.
- **Capture Ground and Text** (`capture-ground`, `capture-text`): the stand-in host terminal in `monarch-dark` captures, and the diagram's panel. Theme stills use each theme's own ground and text instead.
- **Capture Strip** (`capture-strip`): the caption band under a capture.
- **Capture Border** (`capture-border`): capture and diagram outlines, and the diagram's rules.

### Named Rules
**The Shared Wing Rule.** `wing`, `wing-edge` and `wing-spot` are the same in every theme. A theme recolours the chrome, never the butterfly.

**The Wing Stays Off Chrome Rule.** In the app, wing tokens draw only butterflies, and the accent is a separate token. Outside the app, the signals diagram uses `wing` for its trust ladder and connector.

**The Own Chrome Rule.** Monarch colours only its own chrome. Programs in panes keep their colours, untouched.

**The Token Rule.** Widgets take colours from theme tokens, never literals. A new chrome colour is a new `Theme` field set in all eight themes.

## Typography

**App:** the host terminal's font, at the host's size. Monarch ships no font.
**README:** GitHub's own type. There is no custom CSS.
**Captures:** JetBrains Mono, as the first available face in the `capture-cell` stack, set one glyph per cell. The caption strip uses the system sans stack.
**Diagram:** the same system sans stack, so it sits with GitHub's type.

**Character:** a monospace grid that belongs to the user, with hierarchy made from terminal attributes rather than sizes.

### Hierarchy
- **App emphasis:** bold for sidebar headings, the shown tab and settled state glyphs; dim for most popup borders, hints and untracked counts; reverse video for selection. There are no sizes to choose from.
- **Capture:** `capture-cell` for the grid, `capture-key` for pressed keys and `capture-caption` for the words beside them.
- **Diagram:** `diagram-heading` for sources, `diagram-state` for state names, `diagram-label` for group labels and `diagram-body` for descriptions. The state descriptions step down to 18px.

### Named Rules
**The Borrowed Type Rule.** Monarch has no type scale of its own. The terminal and GitHub set the type, and captures and the diagram carry the only specified sizes.

## Layout

The app is a character grid: a sidebar, panes, and a status bar, all whole cells. Density is the terminal's.

The README is a single GitHub column. The mark, name and promise are centred above the install command and a full-width hero. Images size by attribute: `100%` for full screens, `38%` right-floated for the sidebar, `32%` each for the row of three themes, and fixed widths of 440, 560 and 640 for popups and the diagram. `<picture>` swaps in phone crops at `max-width: 600px`.

Timestamps as `###` headings structure the README's main story, with a line of jump links above them. This device belongs to the README and is not a rule for other surfaces.

Captures render a 108 × 26 grid, or a crop of it, on `cell-width` by `cell-height` cells inside `capture-pad`. Stills render at 2x, and the hero at 1.5x wide and 2x for phones. The phone hero uses `caption-strip-phone`.

## Elevation & Depth

Flat everywhere. Nothing in the app, captures, diagram or README has a shadow. A popup is separated by clearing the cells beneath it and drawing a border, and the diagram separates groups with a rule.

### Named Rules
**The No-Shadow Rule.** Separation comes from borders, cleared cells and reverse video, because a cell grid can't draw anything else.

## Shapes

App boxes use square box-drawing corners (`┌ ┐ └ ┘`), including panes and popups. Rounded corners exist only in framing outside the app: `capture`, `diagram`, `ring` and `key-chip`.

The butterfly is bilaterally symmetric in both forms. The logo mirrors one half with `<use>`, and the sprite stores the left half and mirrors it in code.

## Components

### Status Bar and Shown Tab
The bar takes `status-bar`, and the shown tab and waiting prefix take `shown-tab` in bold. Under `NO_COLOR` the bar is reverse video and the accent falls back to bold, and the chrome still reads in the host's colours.

### State Glyphs
Each state has its own shape, so colour is never the only signal. Frames step every 100 ms. With `animations` off, each glyph holds the look given in the last column.

| State | Glyph | Colour and motion | Animations off |
|---|---|---|---|
| Working | `●`, cycling `· • ● •` | `working`, pulsing to half brightness over 1600 ms | Full colour, `●` |
| Blocked | `◆` bold | `blocked-from` to `blocked-to` and back over 2000 ms | `blocked-from` |
| Idle | `○` | Text colour | Same |
| Done | `✓` | `done-from` to `done-to` over 1200 ms, bold once settled | `done-to`, bold |
| Finished (seen) | `✓` | `done-from` | Same |
| Error | `✗` bold | `error` | Same |
| Exited | `■` | Text colour | Same |

Agent icons sit beside the state glyph: `✻` for Claude Code and `⌬` for Codex. They are plain Unicode, so every font draws them.

### Popups
Inbox, pickers, the worktree form and review share one shape: a square dim border, a title set into the top edge, and key hints set into the bottom edge. Help is the same with a border at normal weight. The selected row is reverse video.

### Butterfly Mark
`docs/assets/logo.svg` draws the welcome sprite's anatomy as a vector with open wings: wing cells between veins, spots along the margins, and a `wing-spot` keyline around the whole silhouette. The keyline lets one file work on light and dark grounds. The README shows it at 136px.

### Welcome Sprite
Half-block pixel art at three depths: 19 × 7, 11 × 3 and 3 × 1 cells. Wings cycle open, half, closed, half. It is built only from the three wing tokens.

### Capture Frame
`scripts/readme_shots.py` draws the client's real output cell by cell from a real PTY. Box-drawing and block characters are drawn as vector shapes so they join with no gaps. The `capture-frame` has a 1px `capture-border` outline. The capture path records colour, bold and reverse only, so dim text appears at full brightness in captures.

Three annotations sit outside what the app draws: the `capture-caption` strip, the `capture-key-chip` showing the key pressed, and a 2px `dark-accent` ring with `ring` corners around a sidebar row. The README's caption says that captions and the ring were added for the recording.

### Signals Diagram
`docs/assets/signals.svg` is a self-contained dark panel with the `diagram-panel` ground and a 1px `capture-border` outline. Three `wing` dots shrink and fade down the trust ladder (radius 9, 7, 5; opacity 1, 0.7, 0.45). States below use `monarch-dark` colours and their glyph shapes drawn as vectors. Its text colours (#e6e8f0, #a4a8ba) and exited square (#7f8396) are local to this file. Its Blocked row uses the `blocked_from` colour, #c8a000.

## Do's and Don'ts

### Do:
- **Do** take every chrome colour from a theme token, and define new tokens in all eight themes.
- **Do** pair each state's colour with its own glyph, and check that it reads under `NO_COLOR`.
- **Do** make product imagery with `scripts/readme_shots.py` from the real binary, with scripted agents and added annotations stated in the caption.
- **Do** use `docs/assets/logo.svg` as it is on any ground; its keyline handles both.
- **Do** keep the README to GitHub-flavoured Markdown, with `<picture>` and image attributes for layout.

### Don't:
- **Don't** use wing tokens in app chrome, or vary them by theme.
- **Don't** recolour or restyle what a program draws in its pane.
- **Don't** round the corners of app boxes; the rounded frame belongs to captures.
- **Don't** add shadows.
- **Don't** add a type scale or font to the app or the README.
- **Don't** treat `dark-blocked-to` or `light-done-from` as legible text colours; on the captures' grounds they measure 2.58:1 and 2.16:1.
