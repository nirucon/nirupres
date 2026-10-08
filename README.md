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
