---
name: Nuvora
description: A self-hosted media server with Plex's look and noticeably faster screens.
colors:
  canvas: "#0f1012"
  surface: "#1a1c1f"
  surface-raised: "#24272b"
  hairline: "#2c2f34"
  divider: "#1d1f22"
  overlay-border: "rgba(255,255,255,0.18)"
  overlay-panel: "rgba(26,28,31,0.82)"
  overlay-hairline: "rgba(255,255,255,0.08)"
  text-primary: "#f2f2f2"
  text-secondary: "#a6a8ab"
  text-muted: "#85888d"
  accent: "#e5a00d"
  accent-hover: "#f0b232"
  accent-text: "#141414"
  focus-ring: "#e5a00d"
  danger: "#e5534b"
  placeholder: "#1f2226"
  poster-hover: "rgba(8,9,10,0.38)"
  video: "#000000"
  title-card-ink: "#f7f2ea"
  title-card-ink-muted: "rgba(247,242,234,0.7)"
  title-card-vignette: "rgba(0,0,0,0.6)"
  title-card-edge: "rgba(255,255,255,0.06)"
typography:
  ui:
    fontFamily: "'Inter Variable', Inter, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.43
  small:
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: "18px"
  body-large:
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 400
    lineHeight: "24px"
  button:
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 600
    lineHeight: "20px"
  chip:
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 600
    lineHeight: "14px"
    letterSpacing: "0.02em"
  caption:
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: "16px"
  title:
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 600
    lineHeight: 1.4
  heading:
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif"
    fontSize: "28px"
    fontWeight: 700
    lineHeight: "34px"
  headline:
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif"
    fontSize: "40px"
    fontWeight: 700
    lineHeight: 1.1
    letterSpacing: "-0.02em"
  wordmark:
    fontFamily: "'Archivo Variable', Archivo, 'Arial Narrow', sans-serif"
    fontSize: "19px"
    fontWeight: 700
    lineHeight: "24px"
    letterSpacing: "0.01em"
  card-title:
    fontFamily: "'Archivo Variable', Archivo, 'Arial Narrow', sans-serif"
    fontSize: "23px"
    fontWeight: 800
    lineHeight: 0.93
    letterSpacing: "0.004em"
  card-title-medium:
    fontFamily: "'Archivo Variable', Archivo, 'Arial Narrow', sans-serif"
    fontSize: "26px"
    fontWeight: 800
    lineHeight: 0.93
  card-title-large:
    fontFamily: "'Archivo Variable', Archivo, 'Arial Narrow', sans-serif"
    fontSize: "36px"
    fontWeight: 800
    lineHeight: 0.93
  card-year:
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif"
    fontSize: "10.5px"
    fontWeight: 600
    lineHeight: 1
    letterSpacing: "0.08em"
rounded:
  sm: "4px"
  md: "6px"
  lg: "10px"
  pill: "999px"
spacing:
  "1": "4px"
  "2": "8px"
  "3": "12px"
  "4": "16px"
  "5": "24px"
  "6": "32px"
  "7": "48px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-text}"
    rounded: "{rounded.md}"
    padding: "0 22px"
    height: "44px"
  button-primary-hover:
    backgroundColor: "{colors.accent-hover}"
  poster:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.sm}"
    width: "150px"
  chip:
    textColor: "rgba(255,255,255,0.82)"
    rounded: "{rounded.sm}"
    padding: "0 6px"
    height: "20px"
---

# Design

`packages/tokens` is the code source of truth, and this file must match it. The Step 1 web build settled the values above.

## Overview

Nuvora follows Plex's interface language on purpose: a near-black canvas, a left navigation rail, horizontal poster rows, full-bleed detail pages with an ambient backdrop, and one warm amber accent for the primary action and focus. People use it at home in the evening, often with the lights low, so the interface is dark and lets artwork carry the color. It is an Operate surface. Familiarity is a feature, and delight lives in the artwork, the speed, and the details.

## Colors

### Primary

Amber `#e5a00d` marks the primary action (Play), the current selection, focus rings and progress. It never decorates.

### Neutral

Three stacked neutrals: `canvas` for the page and the chrome, menus included (a hairline border and a soft shadow lift them off the page), `surface` for posters and panels, `surface-raised` for hovered rows, highlighted menu items and the active nav item. Body text uses `text-primary`. Metadata uses `text-secondary`. `text-muted` is the floor for small text and still clears 4.5:1 on the canvas.

### Named Rules

- **Artwork owns the color.** The chrome stays neutral. Color on a screen comes from posters, backdrops, and the ambient wash behind a detail page.
- **One accent.** Nothing else in the chrome is saturated, except `danger` for destructive actions.

## Typography

Inter carries the whole interface on a fixed px scale (12, 13, 14, 16, 20, 28, 40), plus 15 for button labels and 11 for chips. Archivo, at 64% width on its variable `wdth` axis, is used only for the title card, the poster a movie gets when it has no artwork, where it plays the role of poster lettering. The wordmark uses Archivo at 112% width. Both faces are self-hosted from `@fontsource-variable` packages, because the server often runs on a LAN without internet access. Numerals in runtimes, sizes, and bitrates use tabular figures.

### Hierarchy

- Row titles are 20/600.
- Detail titles are display size, fluid from 40 to 76px, 800 at -0.035em, set on the hero's art. They are the one place type goes past 40.
- Poster captions are 14/500 for the title and 13/400 `text-secondary` for the year, on one line each and truncated.

## Layout

The top bar is 56px. It holds the wordmark on the left and the scan status pill and scan button on the right. Search joins it, centered, in Step 2. The left navigation rail is 232px and collapses to icons under 1024px. Content has 40px side padding. Poster rows scroll horizontally, with a 150px card and a 16px gap. Each row shows its first page from the server and loads more as it scrolls.

## Elevation & Depth

The interface is flat by default. A hovered or focused poster lifts with a soft shadow (`0 12px 28px rgba(0,0,0,.45)`) and a 2px amber ring offset by 2px. A detail page's poster, shown only when there is no backdrop, rests on `0 18px 40px rgba(0,0,0,.5)`, and the Play button and the poster play badge on `0 6px 16px rgba(0,0,0,.4)`. A detail hero layers the backdrop under three scrims into `canvas`: up from the bottom, in from the left, and a light one down from the top bar.

## Shapes

Posters use 4px corners, and buttons and fields use 6px. The scan pill and avatar are fully rounded.

## Components

### Buttons

- **Primary.** Amber with dark text, 44px tall, a play icon plus "Play". On a detail hero it is 52px with 10px corners, and Resume carries a small dark progress bar and the time left.
- **Secondary.** Uses `surface-raised` with primary text.
- **Round.** A detail hero's companions to Play (Start over, Watched) are 52px circles of 14% white over the art, icon only, with a tooltip. Watched turns amber when pressed.
- **Disabled.** The primary button drops to `surface-raised` with `text-muted`, and an inline notice explains why.

### Chips

Technical badges (1080p, HEVC, HDR10, 5.1) are 11px bold text, 20px tall, in a 1px 40% white outline, because they sit on the hero's art. They are the server's version `summary` segments plus the container.

### Navigation

Nav items are 40px rows with an icon and a label. The active item uses `surface-raised` with an amber icon. There is no colored side border.

### Detail pages

A movie or show opens the way Apple TV's app does. A hero fills the window, less 72px so the next row peeks in. It shows the backdrop, or the title card's wash with the poster on the right when there is none. The title, a facts line (genres, year, length, rating, badges), Play with its round companions, a three-line synopsis and "Starring" sit on its lower left. The art's lower edge dissolves into a 36px blur of itself before the scrims take it to `canvas`. That is a treatment of the artwork, not a glass panel. Below the hero come shelves: a show's episodes as 16:9 cards under season pills, then the cast in round portraits. Each shelf has back and forward buttons and Left/Right keys. An "Information" block closes the page with titled columns of label-over-value facts (About, Languages, File) and where the details came from.

### Watch marks

An in-progress poster carries a 3px amber bar along its bottom edge over a dark track, with a 1px dark seam so it reads over warm artwork. A watched poster carries a 22px canvas disc in the top-right corner with an amber check. Unwatched posters carry nothing. The detail poster uses the same marks at 4px and 28px. An episode card's watched check is a 30px button on the still's corner. It is always visible once the episode is watched, and otherwise appears when the pointer is over the card.

### Player

The player fills the window with `video` black. A top bar holds the back arrow, title and year, and a chip saying how the file reaches the screen: Direct play, Direct stream, Converting audio or Converting video, with the server's reason on hover. A bottom bar holds the timeline (elapsed, scrubber, remaining) and three clusters: volume on the left, back 10 s, play and forward 10 s in the center, and picture-in-picture and full screen on the right. Each bar sits on its own black gradient scrim, and the bars fade together: 160 ms in, 400 ms out with a 6px drift. Buttons are white glyphs with a soft white disc on hover and a tooltip naming the action and its key. The scrubber is 4px, 6px on hover, with amber for played, translucent white for buffered, and an amber thumb on hover. Volume fills in white, because amber is reserved for progress and focus.

### Title card (signature component)

A movie without poster artwork gets a title card, never a cropped video frame. It is a deep two-stop gradient picked from twelve cinematic palettes by a hash of the title (`titleCardLook` in `@nuvora/core`, so every client picks the same one), with a soft light bloom, a vignette, and fine film grain. The title is set bottom-left in uppercase condensed Archivo 800 in `title-card-ink`, with the year below it in tabular Inter. The same palette tints the ambient wash on the detail page, so the card and the page belong together.

## Do's and Don'ts

### Do:

- Paint every poster in the first frame, using the title card or the ThumbHash placeholder.
- Keep motion to state changes (150 to 240 ms, standard easing), and respect reduced motion.
- Theme the browser parts too: selection, scrollbars, focus rings and caret.

### Don't:

- Use spinners where a placeholder can stand in.
- Put the accent on inactive or decorative elements.
- Add an eyebrow label above a heading, gradient text, or a glass effect.

## Personal home screens

- **Library icons.** Each library shows one of 22 preset line icons (24-unit grid, 1.75 stroke, same as the rest of the set) in the rail, on its home row and as the library page's heading, where it is amber. Movies default to the film icon and TV to the TV icon.
- **Backgrounds.** Home can sit on a wash in one of six palettes (Dusk, Ocean, Forest, Ember, Aurora, Graphite), each three soft radial gradients at 14% to 42% opacity, or "Posters", the first poster on screen blurred 80px at 50%. Every background fades into the canvas within the first screen and a half, so rows stay on the canvas color for reading.
- **Poster sizes.** Small 124px, medium 150px, large 196px. Everything else in a row scales from the card width.
- **Customize panel.** A 440px panel from the right on `canvas`, like the menus, with swatches, a segmented size control, and the row list on `surface`. A row's move arrows appear on hover or focus (always on touch screens). Changes apply as they happen and save to the account, so the panel only speaks up when a save fails.
