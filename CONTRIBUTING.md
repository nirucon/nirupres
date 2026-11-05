# Contributing

NIRUPRES is maintained as a personal application. Issues and constructive
feedback are welcome. Please open an issue before proposing substantial
architectural changes. Do not submit unrelated formatting changes.

Never commit user presentations, local preferences, secrets, caches or
generated build artifacts.

## Running the tests

The release gate (`tests/release_gate.py`) is an offscreen Qt regression test.
It needs the runtime dependencies from `requirements.txt`, which are the same
ones the installer puts in its virtual environment
(`~/.local/share/nirupres/.venv`).

```sh
scripts/test.sh            # creates or reuses ./.venv from requirements.txt
scripts/test.sh --system   # uses the distribution's python3 + PySide6
```

Distribution PySide6 packages (for example Debian's `python3-pyside6`) do not
ship `PySide6.QtTest`; the gate then falls back to an equivalent event-based
helper in `tests/gate_compat.py`. `python-pptx` is only needed for PPTX export,
not for the gate.

The gate redirects `XDG_DATA_HOME`, `XDG_CONFIG_HOME` and `XDG_STATE_HOME` to a
temporary directory before importing NIRUPRES, so running it (also from
`install.sh`) never reads or deletes your autosave data, recents or settings.

Formatting is mechanical: `black --line-length 100 nirupres tests` (a dev-only
tool; it is not a runtime dependency).
