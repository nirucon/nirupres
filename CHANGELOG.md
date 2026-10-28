# Changelog

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
