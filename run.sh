#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PYTHON="$ROOT/.venv/bin/python"
if [ ! -x "$PYTHON" ]; then
    printf '%s\n' 'NIRUPRES: Python environment is missing. Re-run install.sh.' >&2
    exit 1
fi
# Python -m resolves the application package from the current working directory.
# Desktop launchers/UWSM do not guarantee that cwd is the install directory, so
# enter ROOT explicitly before starting NIRUPRES.
cd "$ROOT"
exec "$PYTHON" -m nirupres.main "$@"
