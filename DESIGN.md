---
name: Nuvora
description: A self-hosted streaming service that looks and moves like a premium one.
colors:
  canvas: "#060708"
  surface: "#14161a"
  surface-raised: "#1e2126"
  hairline: "rgba(255,255,255,0.12)"
  divider: "rgba(255,255,255,0.08)"
  overlay-border: "rgba(255,255,255,0.32)"
  overlay-panel: "rgba(22,24,28,0.72)"
  text-primary: "rgba(255,255,255,0.96)"
  text-secondary: "rgba(235,235,245,0.68)"
  text-muted: "rgba(235,235,245,0.52)"
  accent: "#ffffff"
  accent-hover: "#e9e9ec"
  accent-text: "#050505"
  focus-ring: "#ffffff"
  danger: "#ff5f57"
  placeholder: "#16181c"
  video: "#000000"
  material-thin: "rgba(30,32,38,0.38)"
  material-regular: "rgba(26,28,34,0.56)"
  material-thick: "rgba(18,20,24,0.78)"
  material-solid: "#17191d"
typography:
  ui:
    fontFamily: "'Inter Variable', Inter, -apple-system, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: "22px"
  caption: { fontSize: "12px", lineHeight: "16px" }
  small: { fontSize: "13px", lineHeight: "18px" }
  body-large: { fontSize: "17px", lineHeight: "25px" }
  button: { fontSize: "16px", fontWeight: 600, lineHeight: "20px" }
  title: { fontSize: "22px", fontWeight: 600, lineHeight: "28px", letterSpacing: "-0.015em" }
  heading: { fontSize: "28px", fontWeight: 700, lineHeight: "34px" }
  headline: { fontSize: "34px", fontWeight: 700, lineHeight: "40px" }
  display: { fontSize: "44px to 88px (fluid)", fontWeight: 700, lineHeight: 0.96, letterSpacing: "-0.045em" }
  wordmark: { fontFamily: "'Archivo Variable', Archivo, sans-serif", fontSize: "20px", fontWeight: 700 }
  card-title: { fontFamily: "'Archivo Variable', Archivo, sans-serif", fontSize: "23px", fontWeight: 800, lineHeight: 0.93 }
rounded:
  sm: "8px"
  md: "10px"
  card: "12px"
  lg: "20px"
  sheet: "28px"
  pill: "999px"
spacing:
  "1": "4px"
  "2": "8px"
  "3": "12px"
  "4": "16px"
  "5": "24px"
  "6": "32px"
  "7": "48px"
  "8": "64px"
  gutter: "16px to 56px (4.4vw)"
motion:
  spring-snappy: "linear(0, 0.22 7%, 0.6 16%, 0.86 26%, 0.98 36%, 1.02 46%, 1.01 60%, 1) over 520ms"
  spring-smooth: "linear(0, 0.13 8%, 0.43 19%, 0.72 31%, 0.9 44%, 0.98 58%, 1) over 520ms"
  ambient: "1200ms standard"
  lift-scale: 1.06
---

# Design

`packages/tokens` is the code source of truth, and this file must match it. The November 2026 redesign settled the values above. The TV apps still use their own look; Vega reads only the color tokens.

## Overview

Nuvora looks and moves like a premium streaming service, in Apple TV's manner with a Netflix home: a deep, slightly cool canvas, full-bleed artwork that drifts slowly, rows that run off the window's edge, and glass on the chrome that floats over it all. People watch at home in the evening, often with the lights low, so the interface is dark and lets artwork carry the color. Delight lives in the artwork, the motion and the speed.

## Colors

### Named Rules

- **Artwork owns the color.** The chrome is neutral and white. Each screen takes its glow from its art: the ThumbHash's average color, moved in OKLCH into a lightness and chroma band that reads as light on the canvas (`lib/ambient.ts`). It costs no request and is there in the first frame.
- **White is the accent.** The primary action (Play), focus rings, progress and the current section are white, with `accent-text` on white. Nothing in the chrome is saturated, except `danger` and a person's avatar.
- **Glass with restraint.** The three materials (thin, regular, thick: a tint, a backdrop blur with saturation, one hairline stroke with a top highlight) are for chrome that floats over content: the top bar once the page scrolls, the section pill, the phone tab bar, the player's dock, sheets, menus, buttons over art. Never on content cards. Without backdrop-filter, or with reduced transparency, they become `material-solid`.

### Neutral

`canvas` is the page. `surface` sits under art before it loads and behind panels. Labels use an opacity ramp of white: `text-primary` for titles, `text-secondary` for metadata (9.6:1 on the canvas), `text-muted` as the floor for small text (6.1:1).

## Typography

Inter, self-hosted with optical sizing, carries everything; on Apple devices it reads close to SF Pro, and it works on a server with no internet. The scale is 12, 13, 15, 17, 22, 28 and 34, plus a fluid display size for hero titles (44 to 88, bold at -0.045em). Archivo appears only in the wordmark and the title card. Numbers in runtimes, sizes and bitrates use tabular figures.

- Hero and detail titles: display, balanced, with a soft shadow on the art.
- Row titles: 22/600 at -0.015em, 19 on phones, 26 on TV-size windows.
- Card captions: 13/600 title, 12 secondary for the year or episode.

## Layout

A floating top bar (72px) holds the wordmark, a glass pill of sections (Home, each library, Downloads), and round glass buttons for Search, Customize, scan, Watch Together and the account. It is clear over a hero and turns to thick glass once the page scrolls. Under 900px the pill goes away; under 600px a glass tab bar floats at the bottom with Home, the libraries, Search and Downloads. Screens own their top spacing, so heroes run under the bar. The page margin is fluid, 16 to 56px. Settings keep a section rail beside the page, a row of pills on phones.

Rows: a 22px title, then a strip that scrolls sideways to the window's right edge, snaps card by card, and pads itself so a lifted card's scale and shadow never clip. Glass paddles page it for a mouse; touch swipes. Card widths: posters 156 (116 on phones, 184 at 1600px, 232 at 2200px), wide cards 320, genre tiles 248, ranked posters 132, all scaling at the same breakpoints. The user's poster size setting picks 128, 156 or 200.

## Components

### Home

- **Hero.** Up to six featured titles from the server (new and unfinished first, then what the viewer is in the middle of, then the best rated). Each backdrop drifts in a slow Ken Burns and crossfades to the next every nine seconds; dots pick one. It pauses for the pointer, focus, a hidden tab and reduced motion. Facts (kind, genres, year, seasons or runtime, rating), the title, two or three lines of synopsis, a white Play (Resume, with the episode) and glass More Info. The active art colors the glow under the rows.
- **Continue Watching.** Wide 16:9 cards of the episode's frame or the title's backdrop (or the poster, sharp on a blur of itself), a white progress bar in a frosted track, and the episode under the title. The art resumes playback; the title opens the page.
- **Top 10.** The household's most watched titles, each poster with its rank set large and hollow behind it.
- **Trending.** The last week's most watched; hidden with fewer than three.
- **Genres.** Tiles of a title's backdrop washed from the corner in its own color, the genre's name and size; each opens a genre page.
- **Poster rows.** Recently Added and one per library, loading more as they scroll, with skeleton cards that shimmer.

### Cards

Posters are 2:3 with 12px corners, a resting shadow and a hairline edge. Hover and focus lift them to 1.06 on the snappy spring with a deeper shadow; keyboard focus adds a 3px white ring 4px out. Cards that play show a glass play badge. Watch marks: a white bar in a frosted track while in progress, a glass check once watched, a glass count of unwatched episodes on a show.

### Detail pages

The backdrop fills the window, drifting slowly, its lower edge melting into a blur of itself and then the canvas, tinted by its color. Without a backdrop, the title card's palette washes the hero and the poster stands on the right. On its lower left: the title, facts and badges, white Play (Resume shows its progress and the time left), glass round companions (Start over, Watched, which turns white when on, Delete for admins), the synopsis and Starring. Below: seasons as a glass segmented control (a menu past six) over 16:9 episode cards, More Like This from the server, the cast in round portraits, and an Information block. The card that opened the page morphs into its hero (View Transitions, `lib/sharedArt.ts`): a backdrop into the backdrop, a poster into the poster.

### Player

The picture fills the window. A glass back button and the title sit top left over a soft scrim; timeline and controls share one floating glass dock at the bottom: elapsed, the scrubber (white played, translucent buffered, a glass tip with the preview frame and time), remaining; volume, back 10, a white play disc, forward 10, next episode; quality, subtitles, picture in picture, full screen. The chrome springs in and fades out after three seconds. Skip Intro or Skip Recap (S) shows as a glass pill while the playhead is inside one the file's chapters mark. Over marked end credits, the Next episode card comes up at once and plays when its countdown ends; otherwise it shows over the last ten seconds.

### Search

A large title and a 56px glass field that stays put while results change under it. Before a query, the genres to browse; then movies and shows as posters rising in a beat apart, episodes, and people on quiet cards. `/` or Ctrl/⌘+K opens it from anywhere.

### Title card (signature component)

A movie without poster artwork gets a title card, never a cropped video frame: a deep two-stop gradient picked from twelve palettes by a hash of the title (`titleCardLook` in `@nuvora/core`, so every client picks the same one), a soft bloom, a vignette, fine grain, and the title in condensed Archivo 800. The same palette washes a detail page without a backdrop.

### Icons

Phosphor everywhere, through `apps/web/src/components/Icon.tsx`: bold under 20px, regular from 20px up, fill for solid controls and "on" states. The TV apps use the bold weight from `@phosphor-icons/core`. The wordmark's play triangle is the brand mark, not an icon.

## Motion

Two springs as CSS `linear()` curves: snappy (a touch of overshoot) for lifts, presses and indicators; smooth for heroes, sheets and the dock. Everything animates transform and opacity only. Pages crossfade through the View Transitions API, and shared art morphs between card and hero. Loading shows skeletons that shimmer, never spinners where a shape can stand in. Arrow keys move focus like a TV remote (`lib/spatialNav.ts`): Left and Right along a row, Up and Down to the nearest card of the next row, which scrolls to the middle. With reduced motion, transitions and animations go instant, the hero holds still, and the drift stops.

## Do's and Don'ts

### Do

- Paint every image slot in the first frame: ThumbHash placeholders, title cards, skeletons. Nothing shifts as images arrive.
- Keep text on art legible with scrims and the art's own blur, not with panels.
- Theme the browser parts too: selection, scrollbars, focus rings and caret.

### Don't

- Put glass on content cards, or more than one layer of it on top of another.
- Use a saturated chrome color; the art brings the color.
- Add an eyebrow label above a heading, or gradient text on headings or metrics. (The hollow Top 10 numerals are the one gradient fill, as rank art.)

## Personal home screens

- **Rows.** Continue Watching, Top 10, Recently Added, Trending, Browse by Genre and one row per library by default; each can be hidden, renamed and reordered, and library rows sorted and filtered. Rows added in an update join a saved layout once, where the default puts them.
- **Backgrounds.** With featured titles, Home's glow comes from the hero. Without, the chosen wash (Dusk, Ocean, Forest, Ember, Aurora, Graphite or Posters) sits behind the rows and fades into the canvas.
- **Poster sizes.** Small, medium and large; everything else in a row scales from the card width.
