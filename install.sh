#!/bin/sh
# NIRUPRES installer.
#
#   ./install.sh            install the prebuilt static binary (downloaded from
#                           the GitHub release and checked against SHA256SUMS,
#                           or taken from beside this script / target/release)
#   ./install.sh --build    build from this checkout with cargo instead
#
# Installs ~/.local/bin/nirupres, the desktop entry, icon and the
# application/x-nirupres MIME type. A NIRUPRES 2.x (Python) installation is
# replaced cleanly: its code and virtual environment are removed, your
# preferences, recent files and autosave in ~/.local/share/nirupres stay.
set -eu

VERSION_DEFAULT="3.1.0"
MODE=auto
for arg in "$@"; do
    case "$arg" in
        --build) MODE=build ;;
        -h|--help) printf '%s\n' "Usage: ./install.sh [--build]"; exit 0 ;;
        *) printf 'nirupres install: unknown option %s\n' "$arg" >&2; exit 2 ;;
    esac
done

say() { printf '%s\n' "$*"; }
die() { say "nirupres install: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1; }

[ "$(id -u)" != 0 ] || die 'run ./install.sh as your desktop user'
SRC=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$SRC"
DATA_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}"
DEST="$DATA_ROOT/nirupres"
BIN_DIR="$HOME/.local/bin"
BIN="$BIN_DIR/nirupres"
APP_DIR="$DATA_ROOT/applications"
ICON_DIR="$DATA_ROOT/icons/hicolor/scalable/apps"
MIME_DIR="$DATA_ROOT/mime/packages"

VERSION="${NIRUPRES_VERSION:-}"
if [ -z "$VERSION" ] && [ -f Cargo.toml ]; then
    VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
fi
[ -n "$VERSION" ] || VERSION=$VERSION_DEFAULT

asset() {
    # A data file: beside this script (release) or in the checkout.
    for p in "$SRC/$1" "$SRC/assets/$1" "$SRC/mime/$1"; do
        [ -f "$p" ] && { printf '%s\n' "$p"; return 0; }
    done
    return 1
}

TMP=""
trap '[ -z "$TMP" ] || rm -rf "$TMP"' EXIT
if [ "$MODE" = build ]; then
    need cargo || die 'Rust/cargo is required for --build'
    say 'Building NIRUPRES…'
    cargo build --release --locked
    BIN_SRC="$SRC/target/release/nirupres"
elif [ -x "$SRC/nirupres" ] && [ ! -d "$SRC/nirupres" ]; then
    BIN_SRC="$SRC/nirupres"
elif [ -x "$SRC/target/release/nirupres" ]; then
    BIN_SRC="$SRC/target/release/nirupres"
else
    [ "$(uname -m)" = x86_64 ] || die "no prebuilt binary for $(uname -m); use ./install.sh --build"
    need curl || need wget || die 'curl or wget is required to download the release binary'
    need sha256sum || die 'sha256sum is required to verify the release binary'
    TMP=$(mktemp -d)
    BASE="https://github.com/nirucon/nirupres/releases/download/v$VERSION"
    say "Downloading NIRUPRES $VERSION…"
    for f in nirupres SHA256SUMS; do
        if need curl; then
            curl -fsSL --retry 3 -o "$TMP/$f" "$BASE/$f" || die "could not download $f"
        else
            wget -qO "$TMP/$f" "$BASE/$f" || die "could not download $f"
        fi
    done
    expected=$(awk '$2 == "nirupres" || $2 == "*nirupres" { print $1; exit }' "$TMP/SHA256SUMS")
    actual=$(sha256sum "$TMP/nirupres" | awk '{print $1}')
    [ -n "$expected" ] && [ "$expected" = "$actual" ] || die 'the downloaded binary failed SHA-256 verification'
    say 'SHA-256 verification passed.'
    BIN_SRC="$TMP/nirupres"
fi
chmod +x "$BIN_SRC"
if [ -f "$SRC/SHA256SUMS" ] && [ "$BIN_SRC" = "$SRC/nirupres" ]; then
    expected=$(awk '$2 == "nirupres" || $2 == "*nirupres" { print $1; exit }' "$SRC/SHA256SUMS")
    actual=$(sha256sum "$BIN_SRC" | awk '{print $1}')
    [ "$expected" = "$actual" ] || die 'the local binary failed SHA-256 verification'
fi
got=$("$BIN_SRC" --version 2>/dev/null | awk '{print $NF}')
[ "$got" = "$VERSION" ] || die "the binary reports version '$got', expected $VERSION"

# The version installed before (a 2.x Python release keeps its version in
# its package), for the organization profile of upgraded installations.
OLD_VERSION=none
if [ -f "$DEST/nirupres/__init__.py" ]; then
    OLD_VERSION=$(sed -n 's/^__version__ = "\(.*\)"/\1/p' "$DEST/nirupres/__init__.py" | head -n 1)
    [ -n "$OLD_VERSION" ] || OLD_VERSION=unknown
elif [ -x "$BIN" ]; then
    OLD_VERSION=$("$BIN" --version 2>/dev/null | awk '{print $NF}') || OLD_VERSION=unknown
fi

mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR" "$MIME_DIR" "$DEST"
install -m 0755 "$BIN_SRC" "$BIN.tmp.$$"
mv -f "$BIN.tmp.$$" "$BIN"
install -m 0644 "$(asset nirupres.svg)" "$ICON_DIR/nirupres.svg"
install -m 0644 "$(asset nirupres.xml)" "$MIME_DIR/nirupres.xml"
sed "s|^Exec=nirupres|Exec=$BIN|; s|^TryExec=nirupres|TryExec=$BIN|; s|^Icon=nirupres|Icon=$ICON_DIR/nirupres.svg|" \
    "$(asset nirupres.desktop)" > "$APP_DIR/nirupres.desktop"
chmod 0644 "$APP_DIR/nirupres.desktop"
need update-mime-database && update-mime-database "$DATA_ROOT/mime" >/dev/null 2>&1 || :
need update-desktop-database && update-desktop-database "$APP_DIR" >/dev/null 2>&1 || :

# Replace the Python release: its code and virtual environment go, user data stays.
for old in "$DEST/nirupres" "$DEST/tests" "$DEST/.venv"; do
    [ ! -d "$old" ] || rm -rf -- "$old"
done
rm -f -- "$DEST/run.sh" "$DEST/requirements.txt"

# 2.2 organization profiles: an installation upgraded from 2.1 or earlier
# keeps the Uddevalla defaults it always had. Written once, never overwritten.
if [ ! -e "$DEST/profile.json" ]; then
    case "$OLD_VERSION" in
        1.*|2.0.*|2.1.*|unknown)
            printf '{\n  "profile": "uddevalla",\n  "reason": "upgraded from %s"\n}\n' "$OLD_VERSION" > "$DEST/profile.json" || : ;;
    esac
fi

"$BIN" --version >/dev/null || die 'the installed binary does not run'
say "NIRUPRES $VERSION installed at $BIN"
[ "$OLD_VERSION" = none ] || say "Upgrade: $OLD_VERSION -> $VERSION (preferences, recent files and autosave were kept)"
say 'Run: nirupres'
