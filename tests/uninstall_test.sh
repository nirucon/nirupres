#!/bin/sh
# uninstall.sh keeps preferences and autosave unless --purge (2.2.1).
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
mk() { for f in .local/bin/nirupres .local/share/applications/nirupres.desktop \
    .local/share/nirupres/nirupres/__init__.py .local/share/nirupres/.venv/bin/python \
    .local/share/nirupres/run.sh .local/share/nirupres/profile.json .local/share/nirupres/recent.json; do
    mkdir -p "$(dirname "$1/$f")"; : > "$1/$f"; done; }
mk "$T/a"; env -u XDG_DATA_HOME HOME="$T/a" sh "$ROOT/uninstall.sh" >/dev/null
[ ! -e "$T/a/.local/bin/nirupres" ] && [ ! -e "$T/a/.local/share/nirupres/.venv" ] && [ ! -e "$T/a/.local/share/nirupres/nirupres" ] || { echo "FAIL: program left"; exit 1; }
[ -f "$T/a/.local/share/nirupres/profile.json" ] && [ -f "$T/a/.local/share/nirupres/recent.json" ] || { echo "FAIL: data removed"; exit 1; }
mk "$T/b"; env -u XDG_DATA_HOME HOME="$T/b" sh "$ROOT/uninstall.sh" --purge >/dev/null
[ ! -e "$T/b/.local/share/nirupres" ] || { echo "FAIL: purge kept data"; exit 1; }
if env -u XDG_DATA_HOME HOME="$T/b" sh "$ROOT/uninstall.sh" --bogus 2>/dev/null; then echo "FAIL: unknown option accepted"; exit 1; fi
echo "PASS: uninstall keeps data, --purge removes it"
