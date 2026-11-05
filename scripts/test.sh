#!/bin/sh
# Run the NIRUPRES release gate in a virtual environment built from
# requirements.txt. Usage: scripts/test.sh [--system]
#   default   create or reuse $NIRUPRES_TEST_VENV (default: ./.venv)
#   --system  use the system python3 and its PySide6 (QtTest is optional)
# The gate always runs offscreen with private XDG directories, so it never
# touches your presentations, autosave data or preferences.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
if [ "${1:-}" = "--system" ]; then
    PYTHON=$(command -v python3) || { echo "python3 not found" >&2; exit 1; }
else
    VENV=${NIRUPRES_TEST_VENV:-$ROOT/.venv}
    if [ ! -x "$VENV/bin/python" ]; then
        echo "Creating test environment in $VENV"
        python3 -m venv "$VENV"
    fi
    PYTHON=$VENV/bin/python
    if ! "$PYTHON" -c 'import PySide6.QtWidgets, pptx, packaging' 2>/dev/null; then
        "$PYTHON" -m pip install -q -r requirements.txt
    fi
fi
"$PYTHON" -m compileall -q nirupres tests
QT_QPA_PLATFORM=offscreen PYTHONPATH="$ROOT" "$PYTHON" tests/release_gate.py
