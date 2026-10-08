# NIRUPRES Markdown Specification v1

NIRUPRES Markdown v2 is the stable, AI-friendly interchange format for NIRUPRES 1.x. The application version and Markdown spec version are intentionally independent.

## Deck block

A document starts with an HTML comment metadata block:

```text
<!-- deck
nirupres: 2
title: Example
theme: UDDEVALLA BLUE
font: Noto Sans
ratio: 16:9
numbers: false
logo: true
footer: Socialtjänsten · Uddevalla kommun
footer-align: LEFT
footer-size: MEDIUM
transition: MORPH
-->
```

Supported ratios: `16:9`, `16:10`, `4:3`, `A4`. Supported transitions: `NONE`, `FADE`, `MORPH`, `REVEAL`.

## Slides

Slides are separated by a line containing exactly `---`. Each slide begins with a `<!-- slide` block. Supported layouts:

`TITLE`, `STATEMENT`, `TEXT`, `BULLETS`, `AGENDA`, `IMAGE`, `PHOTO`, `HERO IMAGE`, `SPLIT`, `TWO COLUMN`, `COMPARE`, `IMAGE + QUOTE`, `QUOTE`, `BIG NUMBER`, `DATA / KPI`, `NUMBER GRID`, `PROCESS`, `TIMELINE`, `MATRIX`, `TABLE`, `SECTION`, `FULL BLEED`, `END`.

Common slide fields are `theme`, `font`, `hidden`, `image-mode`, `mono`, `focal-x`, `focal-y`, `align`, `weight`, `font-scale`, `brightness`, `contrast`, `overlay`, `blur`, `logo`, `accent`, `transition`, `image`, `image-query`, and `caption`.

Per-slide `logo`: `AUTO`, `ON`, `OFF`. Per-slide `transition`: `DECK`, `NONE`, `FADE`, `MORPH`, `REVEAL`. Accent: `AUTO`, `BLUE`, `GREEN`, `YELLOW`, `RED`, `PINK`, `PURPLE`.

The first Markdown heading (`#`) is the slide title. Remaining plain text is the body. Speaker notes use a `<!-- notes ... -->` block.

## Safety

NIRUPRES does not execute Markdown, HTML, scripts, commands, or URLs. `image-query` is descriptive metadata only. Unknown/invalid supported fields are normalized to safe defaults on AI-oriented import where possible.


### Presentation motion
Deck metadata supports `transition: NONE|FADE|MORPH|REVEAL` and optional `reduce-motion: true|false`. Slide metadata supports `transition: DECK|NONE|FADE|MORPH|REVEAL`.


## Layout catalogue (1.2)

NIRUPRES 1.2 ships 23 curated layouts while keeping Markdown Spec v1. `layout:` accepts: TITLE, STATEMENT, TEXT, BULLETS, AGENDA, IMAGE, PHOTO, HERO IMAGE, SPLIT, TWO COLUMN, COMPARE, IMAGE + QUOTE, QUOTE, BIG NUMBER, DATA / KPI, NUMBER GRID, PROCESS, TIMELINE, MATRIX, TABLE, SECTION, FULL BLEED, END.

Structured body conventions are intentionally simple for AI interoperability: `value | label` for KPI/number layouts, `step | description` for PROCESS/TIMELINE/MATRIX, `|||` on its own line separates TWO COLUMN columns; content on each side is literal text and blank lines are preserved as spacing. COMPARE remains blank-line separated, and TABLE uses pipe-separated cells. Unknown layouts still normalize safely to TEXT on import.


## Footer styling
`footer-align` accepts `LEFT`, `CENTER`, or `RIGHT`. `footer-size` accepts `SMALL`, `MEDIUM`, or `LARGE`. Older decks without these fields render as `LEFT` + `MEDIUM`. All sizes remain deliberately subtle and the renderer keeps footer text inside the slide safe area.


## Image treatment (1.3.0)

Optional slide metadata: `image-treatment: NATURAL|MONO|DIM|CONTRAST`. Existing `mono: true` remains supported and maps to `MONO`. Treatments are non-destructive and are applied by the renderer.

## 1.4 composition metadata

List layouts (`BULLETS`, `AGENDA`) may use `list-style: NUMBERS|DOTS|DASHES|NONE`.
Slides may use a full-canvas background image independently of their normal content image:
`background-image`, `background-dim` (0–90, default 75), `background-blur` (0–20), `background-mono` (true/false), `background-focal-x` and `background-focal-y` (0.0–1.0).
Background imagery is always rendered below theme decoration and slide content.


## TWO COLUMN structure (1.4.8)

Use `|||` on its own line as the explicit column boundary. `---` cannot be used because Markdown v2 reserves it for slide boundaries. Text on each side is rendered literally with one shared fitted font size: no first-line heading, no automatic semantic grouping, and blank lines remain paragraph spacing. The GUI stores LEFT SIDE and RIGHT SIDE independently and generates `|||` only for the portable Markdown/body mirror. Legacy decks that used the first blank line as the column boundary remain readable and are normalized when edited and saved.


## 1.5 media, emphasis and motion

Inline `**bold**` and `*italic*` are preserved as literal Markdown and rendered by NIRUPRES. Optional slide metadata includes `background-motion: NONE|KEN BURNS`, `video: <URL-or-path>`, `video-poster: <path>`, `video-autoplay: true|false`, and `video-muted: true|false`. Deck metadata supports `cinematic-open` and `cinematic-close`. VIDEO and VIDEO + TEXT are valid layouts.
