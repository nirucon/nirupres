# Changelog

## 2.4.0: Fades you can see, aim from the presenter view

- **The opening and closing fades are visible again.** In laptop and online
  mode the fades ran only in the audience window, which the presenter view
  covers, so on your own screen nothing happened (and the end looked like a
  7 second freeze). The **CURRENT** preview in the presenter view now shows the
  same fade, and the opening waits until the audience window is really on
  screen before it starts. Fade in 0.9 s from black, fade out 0.8 s to black,
  then 1 s black and the presentation closes (it was 1.75 s + 5 s). `Esc`
  again closes at once; Esc during the opening fades out from where it is.
- **Aim from the presenter view.** Point at CURRENT with the mouse to aim
  Spotlight (`S`), Focus Reveal (`F`), Zoom (`Z`) and `[` `]` on the audience
  window; click to pin. CURRENT shows the spotlight, focus, zoom and slide
  transitions the audience sees. Works in laptop, online and docked mode.
- **Next on the last slide** fades the slides to black ("END OF SLIDES" in the
  presenter view); Next or `Esc` again ends, Previous fades back in. The NEXT
  pane says "End of presentation" on the last slide.
- The presenter view says "ENDING · press Esc again to close now" while the
  show fades out.
- Fixed: speaker notes started at 13 px instead of the remembered A−/A+ size
  until a size button was pressed.
- Fixed: every key with a modifier (Ctrl, Shift, Super) raised an error with
  PySide6 6.12, and a typed slide number + `Enter` did not jump.
- Fixed: Zoom near a slide edge showed the unzoomed slide in a strip; the
  zoomed slide now always fills the view, and the spotlight veil rests while
  zoomed instead of drawing a second, misplaced hole.
- Fixed: with a compositor that dims inactive windows (picom
  `inactive-opacity`), the slides on a projector and the presenter view became
  slightly see-through when they lost focus. Both windows now ask to stay
  fully opaque (`_NET_WM_WINDOW_OPACITY`).
- Tests: the release gate measures rendered frames of the audience window and
  the preview during both fades (intermediate brightness, about 0.9 s) and
  checks the preview to audience aim mapping for 16:9 and 4:3 decks.

## 2.3.0: Safe online presenting, presenter view, text that fits

- **Safe sharing in Teams, Meet and Zoom.** The audience window is called
  `AUDIENCE · share this · NIRUPRES` (WM_CLASS `nirupres-audience`), the
  presenter view `PRIVATE presenter · do not share` (`nirupres-presenter`) with
  a red PRIVATE bar that shows even in a share-picker thumbnail. The editor
  keeps WM_CLASS `nirupres`.
- **Laptop presenter mode.** F5 on one screen offers the laptop presenter
  (presenter view fills the screen and covers a 16:9 audience window) or slides
  only, with an optional remembered choice. Docked, the audience is fullscreen
  on the chosen screen and the presenter view on the other. The editor is
  hidden while presenting and comes back afterwards.
- **Share check** before an online presentation: tells you to pick the
  **Window** tab and `AUDIENCE · share this · NIRUPRES`, flashes a green border
  and badge on the audience window for 5 seconds and warns when the audience
  window is hidden. The timer starts when you press Start presenting.
- **Workspace hopping.** The audience window sets `_NIRUWM_KEEP_MAPPED`, so
  NIRUWM keeps it live on a hidden workspace; the presenter view warns if it is
  hidden anyway, and coming back gives the presenter view the keyboard at once.
- **Presenting mode:** `niructl present on --pid` (Do Not Disturb, private bar
  modules hidden) while presenting; with an older NIRUWM `niructl dnd on` and
  the previous state restored, also after a crash on the next start.
- **Presenter view** in the app theme, laid out for 1366x768 up to large docked
  screens: current slide, next slide (a title line on small screens), big timer
  with pace, clock, slide x / y and a notes pane of at least a third of the
  height with A− / A+ (`Ctrl+-` / `Ctrl++`, remembered).
- **Templates fit their boxes.** Pixel-sized fonts (the same on every DPI and
  export), a real fit that honours line limits and box height, then an
  ellipsis inside the box; text never draws outside it. Fixed SPLIT, TITLE
  with image, IMAGE, FULL BLEED, VIDEO, NUMBER GRID, AGENDA, HERO IMAGE,
  IMAGE + QUOTE, PHOTO, KPI, TWO COLUMN and BIG NUMBER (shrinks to width).
  Lists that are too long end with "+N more".
- **Deck health** renders every slide and reports real overflow ("Text does not
  fit (title)", "3 bullets hidden"); the slide list shows a FIT badge.
- **New layouts:** CODE (monospace, shrinks to fit, Markdown code fence),
  IMAGE + CAPTION, THREE COLUMN, and TABLE is now a comparison table with
  wrapping cells, content-based column widths and up to 6 columns.
- Tests: a template stress render (every layout, four text lengths, three
  sizes and DPIs) that fails on any overflow, plus presenter, share check and
  presenting-mode checks in the release gate.

## 2.2.1 — Installer and uninstaller

- **Uninstaller.** New `uninstall.sh` (and `nirux extras uninstall nirupres`):
  removes the program and keeps preferences and autosave; `--purge` removes
  those too. Decks are never touched.
- **Fresh Debian.** The installer installs `python3-venv` when venv support
  is missing (with a prompt in a terminal, or with `NIRUPRES_ASSUME_YES=1` /
  `NIRUX_ASSUME_YES=1` unattended) instead of failing with "Could not create
  Python virtual environment".
- **Half-created venv.** A virtual environment left without pip by a failed
  first attempt is rebuilt instead of failing on every later install.

## 2.2.0 — UX and maintainability

Project format 9 and Markdown format 2 are unchanged; existing decks open and
save byte-identically (apart from `appVersion`), also back in 2.1.4.

- **Organization profiles.** The Uddevalla identity (logo on for new decks,
  starters, title preset, placeholders) is an opt-in profile; fresh installs
  start neutral. Upgraded installations and installations with existing
  settings keep Uddevalla automatically. Decks keep their own logo setting.
- **No clipped labels.** Controls size from font metrics (also with the NIRUX
  chrome font); the editor fits 1366x768, the inspector scrolls.
- **Inspector:** sections other than Content start collapsed, show a one-line
  summary and remember their state.
- **Toolbar:** grouped File | Slide | View | Present with separators and a ⋯
  overflow menu at narrow widths; all actions and shortcuts kept.
- **Presenting idles:** the audience window no longer repaints at 25 fps on
  still slides (19% -> 0% CPU when presenting, 11% -> 0.2% online); KEN BURNS
  animates only while moving and pauses under black/white/curtain/PAUS.
- **Autosave status** in the status bar and a Restore/Discard recovery prompt;
  the autosave model is documented in the README.
- **Slide list:** Alt+Up/Down reorders slides; slides have accessible names.
- Closing the editor during Talk mode stops the NIRUCLIP recording.
- The release gate isolates XDG data (the installer's gate run could clear a
  real autosave), runs on distro PySide6 without QtTest, and has new checks.
  `scripts/test.sh`, CI on push/PR, and a module split of `main.py`.

## 2.1.4 — release packaging fix

- Keeps the 2.1 feature set; the release archive includes install.sh before checksumming.

## 2.1.3 — release test path fix

- Keeps the 2.1 feature set; the release gate imports the checkout explicitly.

## 2.1.2 — release packaging fix

- Keeps the 2.1 feature set; release CI installs the EGL/XKB runtime needed by Qt.

## 2.1.1 — release identity fix

- Keeps the 2.1 feature set and fixes the tagged release identity check.

## 2.1.0 — NIRUX-native presentation workflow

- Added XDG data paths, noninteractive CLI (`--present`, `--talk`,
  `--export-pdf`, `--check-config`) and MIME/desktop integration.
- Added **Follow NIRUX** for editor chrome only, with semantic palette
  mapping, live config refresh and a Noir fallback.
- Added **Talk mode**: NIRUCLIP recording with the screen-mic, screen-camera and
  camera-mic profiles, slide chapter markers and SRT sidecars.
- Preserved project format 9, Markdown format 2, slide themes and exports.
- Added release-gate checks for chrome mapping, Talk SRT markers and the new
  command/version contracts.


## 2.0.0 — modularization candidate

- Extracted the 1.7.3 monolithic application into dedicated modules for
  shared model/rendering, presentation windows, editor widgets and Markdown.
- Preserved project format 9, Markdown format 2, themes and existing layouts.
- Added repository documentation, ignore rules and security guidance.
- Updated package, installer and regression-test version contracts.

**Validation status:** Python compilation is checked. The full Qt release
gate and interactive Wayland testing are required before publishing as stable.

## 1.7.3

Previous production baseline. See the tagged 1.7.3 archive for historical
release notes.
