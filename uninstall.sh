#!/bin/sh
# Remove NIRUPRES. By default your preferences, recent files and autosave in
# ~/.local/share/nirupres are kept; --purge removes them too. Your .nirupres
# decks are wherever you saved them and are never touched.
set -eu
purge=0
for arg in "$@"; do
    case "$arg" in
        --purge) purge=1 ;;
        -h|--help) echo "usage: ./uninstall.sh [--purge]"; exit 0 ;;
        *) echo "unknown option: $arg (try --help)" >&2; exit 2 ;;
    esac
done
DATA_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}"
DEST="$DATA_ROOT/nirupres"
rm -f -- "$HOME/.local/bin/nirupres" "$DATA_ROOT/applications/nirupres.desktop" \
    "$DATA_ROOT/icons/hicolor/scalable/apps/nirupres.svg" "$DATA_ROOT/mime/packages/nirupres.xml"
command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$DATA_ROOT/mime" >/dev/null 2>&1 || :
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DATA_ROOT/applications" >/dev/null 2>&1 || :
if [ "$purge" = 1 ]; then
    rm -rf -- "$DEST"
    echo "NIRUPRES removed, including preferences and autosave."
else
    rm -rf -- "$DEST/nirupres" "$DEST/tests" "$DEST/.venv"
    rm -f -- "$DEST/run.sh" "$DEST/requirements.txt"
    echo "NIRUPRES removed. Preferences and autosave were kept in $DEST"
    echo "Run ./uninstall.sh --purge to delete them as well."
fi
