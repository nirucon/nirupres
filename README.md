# NIRUPRES

A minimal, keyboard-oriented desktop presentation application by
Ing Leif Nicklas Rudolfsson. Built with Python and Qt/PySide6, designed primarily
for the author's personal Linux setup (Omarchy/Arch and Debian).

## Features

- Slide layouts and themes, including Uddevalla visual profiles
- Markdown import/export and speaker notes
- Presenter and audience windows
- PDF and image-based PowerPoint export
- Local project archives and autosave

PowerPoint slides are rasterized; slide content is not natively editable.

## Install or upgrade

Extract the release ZIP, enter its directory and run:

```sh
./install.sh
```

The installer validates the release before replacing managed application
files. Existing user data under `~/.local/share/nirupres` is not part of
the release archive and must not be deleted.

Launch with `nirupres`.

## Source layout

- `nirupres/foundation.py` — shared project model, themes, assets, renderer
- `nirupres/presentation.py` — audience/presenter and transitions
- `nirupres/editor.py` — slide editing dialogs and widgets
- `nirupres/markdown_io.py` — Markdown interchange and AI brief
- `nirupres/main.py` — application shell and orchestration
- `tests/release_gate.py` — offscreen Qt regression gate

## Validation

```sh
python3 -m compileall -q nirupres tests
QT_QPA_PLATFORM=offscreen python3 tests/release_gate.py
```

Use the Python virtual environment created by the installer if the system
Python does not provide PySide6 and the other dependencies.

## Compatibility

NIRUPRES 2.0.0 retains project format 9 and Markdown format 2. Existing
presentation files should not require conversion. Keep backups before
major-version upgrades.

## License

MIT License. Copyright (c) 2026 Ing Leif Nicklas Rudolfsson.
See [LICENSE](LICENSE) for the complete terms.

## NIRUPRES 2.1

### NIRUX editor chrome

The editor, dialogs, menus and focus treatment can follow the active NIRUX
appearance. This never changes slide themes, audience output or PDF/PPTX exports:
a deck must look the same on every machine.

- **Follow NIRUX** is enabled by default when NIRUX is detected.
- The active theme comes from `niructl theme-catalog --json`; optional
  `~/.config/niruwm/appearance.toml` overrides are applied.
- Toggle **NIRUX** in the toolbar or use the command palette. The choice is
  persisted. Without NIRUX, the original monochrome Noir chrome is used.
- `nirupres --check-config` reports the effective source and data root.

### Talk mode

The **TALK** toolbar menu starts a presentation from the beginning and records
it with NIRUCLIP:

- Screen + microphone, screen + camera, or camera + microphone.
- NIRUCLIP owns capture and state; NIRUPRES never starts a second recorder.
- Each visible slide change becomes a chapter marker.
- At the end, NIRUPRES stops NIRUCLIP and writes `.chapters.json` and `.srt`
  beside the recording. The recording itself is never deleted.
- Errors from NIRUCLIP, a busy camera or a missing command are shown instead of
  claiming a successful recording.

Talk mode uses the NIRUCLIP state at
`${XDG_STATE_HOME:-~/.local/state}/niruclip/session.json` and the NIRUCLIP
profiles `screen-mic`, `screen-camera` and `camera-mic`. Set `NIRUPRES_TALK_DIR`
to choose a different recording directory.

### Command line

```sh
nirupres                         # editor
nirupres --present deck.nirupres # present from the beginning
nirupres --talk deck.nirupres screen-mic
nirupres --export-pdf deck.nirupres [out.pdf]
nirupres --check-config
nirupres --version
```

### User data and desktop integration

Projects, autosave, recent files and preferences live under the XDG data root,
normally `~/.local/share/nirupres`. Existing projects are untouched. The installer
also registers `application/x-nirupres`, a desktop entry and an icon. Release
assets are immutable and verified by the installer.
