# NIRUPRES AI PRESENTATION BRIEF
nirupres-spec: 2
target-format: NIRUPRES Markdown

## INSTRUCTIONS FOR AI
Return ONLY valid NIRUPRES Markdown. Do not wrap the result in a Markdown code fence. Preserve supported metadata names exactly. Do not invent layouts, themes, local file paths, URLs, scripts, HTML, or executable content. Prefer concise presentation language, one main message per slide, and speaker notes for supporting detail. Use `image-query:` when an image would materially improve a slide.

## NIRUPRES MARKDOWN v2
Deck metadata is a `<!-- deck` block. Every slide starts with a `<!-- slide` block. Slides are separated by a line containing exactly `---`.

Allowed layouts: TITLE, STATEMENT, TEXT, BULLETS, AGENDA, IMAGE, PHOTO, HERO IMAGE, SPLIT, TWO COLUMN, COMPARE, IMAGE + QUOTE, QUOTE, BIG NUMBER, DATA / KPI, NUMBER GRID, PROCESS, TIMELINE, MATRIX, TABLE, SECTION, VIDEO, VIDEO + TEXT, FULL BLEED, END.
Allowed themes: B&W, W&B, GRAY, REDWINE, SATIE, CARL LARSSON, BEKSINSKI, MARTIN, SFUMATO, UDDEVALLA BLUE, UDDEVALLA DARK, UDDEVALLA BLACK, UDDEVALLA GREEN, UDDEVALLA YELLOW, UDDEVALLA RED, UDDEVALLA PINK, UDDEVALLA PURPLE.
Allowed footer alignment: LEFT, CENTER, RIGHT. Allowed footer size: SMALL, MEDIUM, LARGE.
Allowed deck transitions: NONE, FADE, MORPH, REVEAL. Per-slide transition: DECK, NONE, FADE, MORPH, REVEAL. Optional deck `reduce-motion: true` suppresses motion at runtime without deleting transition choices. `cinematic-open` and `cinematic-close` control whole-show black fades.
Allowed logo values per slide: AUTO, ON, OFF. Allowed accents: AUTO, BLUE, GREEN, YELLOW, RED, PINK, PURPLE.
Inline emphasis: `**bold**` and `*italic*` are preserved and rendered. Backgrounds may use `background-motion: KEN BURNS`. VIDEO/VIDEO + TEXT use `video:` plus optional `video-poster:`. BULLETS: one point per body line. TWO COLUMN: use `|||` on its own line between literal left/right columns; no line is a heading and blank lines are preserved as spacing. NUMBER GRID: `value | label` per line, max 4. TIMELINE: `milestone | description` per line, max 5. Notes go in `<!-- notes` blocks and are private.

<!-- deck
nirupres: 1
title: Presentation title
theme: B&W
font: Noto Sans
ratio: 16:9
numbers: false
logo: true
footer:
footer-align: LEFT
footer-size: MEDIUM
transition: NONE
-->

<!-- slide
layout: TITLE
theme: DECK
font: DECK
logo: AUTO
accent: AUTO
transition: DECK
-->

# Presentation title

Short subtitle

---

<!-- slide
layout: STATEMENT
theme: DECK
image-query: editorial photo describing the subject
transition: DECK
-->

# One strong idea

A short supporting sentence.

<!-- notes
Private speaker notes.
-->

---

<!-- slide
layout: END
theme: DECK
transition: DECK
-->

# Thank you
