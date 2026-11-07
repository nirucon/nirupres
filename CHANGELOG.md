# Changelog

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
