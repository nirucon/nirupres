#!/bin/sh
# X11 end-to-end checks in a private Xvfb: first frame and memory, editing
# and saving a deck, PDF and Markdown export, presenting on two screens
# (window identity for NIRUWM), autosave. Usage: tests/xvfb.sh PATH/TO/nirupres
set -eu
BIN=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
SB=$(mktemp -d)
PIDS=""
cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; rm -rf "$SB"; }
trap cleanup EXIT INT TERM
export HOME=$SB/home XDG_DATA_HOME=$SB/data XDG_CONFIG_HOME=$SB/config XDG_CACHE_HOME=$SB/cache
export NIRUPRES_NIRUCTL=none
mkdir -p "$HOME/Documents" "$XDG_DATA_HOME" "$XDG_CONFIG_HOME"
D=:${NIRUPRES_TEST_DISPLAY:-$((70 + $$ % 20))}
Xvfb "$D" -screen 0 3840x1080x24 -nolisten tcp >/dev/null 2>&1 &
PIDS="$PIDS $!"
export DISPLAY=$D
sleep 1
fail() {
  echo "FAIL: $*" >&2
  [ -f "$SB/err" ] && tail -20 "$SB/err" >&2
  for w in $(xdotool search --name . 2>/dev/null); do
    printf 'window %s: %s\n' "$w" "$(xprop -id "$w" WM_CLASS WM_NAME 2>/dev/null | tr '\n' ' ')" >&2
  done
  exit 1
}
k() { xdotool key --delay 60 "$@"; sleep 0.5; }
type_() { xdotool type --delay 30 "$1"; sleep 0.4; }

"$BIN" --version | grep -q '^NIRUPRES [0-9]' || fail "--version"

# A deck in format 9 as NIRUPRES 2.x writes it, with an image asset.
python3 - "$SB/deck.nirupres" <<'PY'
import json, sys, zipfile, struct, zlib
w, h = 64, 40
raw = b"".join(b"\x00" + bytes([120, 90, 60] * w) for _ in range(h))
def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
slides = [
    {"layout": "TITLE", "title": "Garden", "body": "Seeds", "notes": "Hello"},
    {"layout": "SPLIT", "title": "Beds", "body": "Two paths", "image": "asset:pic.png"},
    {"layout": "END", "title": "Thanks", "body": ""},
]
deck = {"format": 9, "appVersion": "2.4.0", "title": "Garden", "theme": "CARL LARSSON", "slides": slides}
with zipfile.ZipFile(sys.argv[1], "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("project.json", json.dumps(deck, indent=2))
    z.writestr("assets/pic.png", png)
PY

# Headless PDF export from the command line.
"$BIN" --export-pdf "$SB/deck.nirupres" "$SB/cli.pdf" >/dev/null || fail "--export-pdf"
head -c 5 "$SB/cli.pdf" | grep -q '%PDF-' || fail "cli pdf"

# The editor: first frame and memory.
NIRUPRES_TIMING=1 NIRUPRES_MONITORS="LEFT:1920x1080+0+0,RIGHT:1920x1080+1920+0" \
    "$BIN" "$SB/deck.nirupres" 2>"$SB/err" &
A=$!
PIDS="$PIDS $A"
sleep 1.5
grep -q 'first frame after' "$SB/err" || fail "no first frame"
ms=$(sed -n 's/.*first frame after \([0-9]*\)\..*/\1/p' "$SB/err")
[ "$ms" -lt 1000 ] || fail "first frame took $ms ms"
rss=$(awk '/VmRSS/ {print $2}' /proc/$A/status)
[ "$rss" -lt 60000 ] || fail "memory $rss kB"
echo "first frame ${ms} ms, ${rss} kB"
W=$(xdotool search --sync --classname '^nirupres$' | head -1)
xdotool windowmove "$W" 0 0
sleep 0.4
xdotool mousemove 700 500

# Edit the title in the slide text editor and save.
xdotool mousemove 700 540 click --repeat 2 --delay 80 1
sleep 0.6
k ctrl+a
type_ "Spring garden"
k Return
k ctrl+s
sleep 0.8
python3 - "$SB/deck.nirupres" <<'PY' || fail "save"
import json, sys, zipfile
z = zipfile.ZipFile(sys.argv[1])
assert z.testzip() is None
d = json.loads(z.read("project.json"))
assert d["format"] == 9 and d["slides"][0]["title"] == "Spring garden", d["slides"][0]["title"]
img = d["slides"][1]["image"]
assert img.startswith("asset:") and "assets/" + img[6:] in z.namelist(), img
PY
echo "edit and save ok"

# Export PDF and Markdown through the file dialog (type a full path).
k ctrl+shift+e
k ctrl+a
type_ "$SB/out.pdf"
k Return
sleep 1
head -c 5 "$SB/out.pdf" | grep -q '%PDF-' || fail "pdf export"
k ctrl+k
type_ "export nirupres markdown"
k Return
k ctrl+a
type_ "$SB/out.md"
k Return
sleep 0.5
grep -q 'layout: SPLIT' "$SB/out.md" && grep -q '# Spring garden' "$SB/out.md" || fail "markdown export"
echo "exports ok"

# Present on two screens: audience and presenter with their identities.
# Any dialog still open after the exports is closed first.
k Escape
k F5
# A slower machine (CI) may need a few seconds for both windows.
AU= PR= n=0
while [ $n -lt 40 ] && { [ -z "$AU" ] || [ -z "$PR" ]; }; do
  sleep 0.2; n=$((n + 1))
  [ $n = 15 ] && k F5
  AU=$(xdotool search --classname '^nirupres-audience$' | head -1)
  PR=$(xdotool search --classname '^nirupres-presenter$' | head -1)
done
[ -n "$AU" ] && [ -n "$PR" ] || fail "presentation windows"
xprop -id "$AU" _NIRUWM_KEEP_MAPPED | grep -q '= 1' || fail "_NIRUWM_KEEP_MAPPED"
xprop -id "$AU" WM_NAME | grep -q 'AUDIENCE' || fail "audience title"
xprop -id "$PR" WM_NAME | grep -q 'PRIVATE presenter' || fail "presenter title"
k Right
k s
k z
k z
k p
k p
k Escape
k Escape
sleep 1
[ -z "$(xdotool search --classname '^nirupres-audience$' 2>/dev/null)" ] || fail "audience still open"
echo "presenting ok"

# Autosave: an unsaved edit is written 2.5 s later.
xdotool mousemove 700 540 click --repeat 2 --delay 80 1
sleep 0.6
type_ " again"
k Return
sleep 3.2
[ -f "$XDG_DATA_HOME/nirupres/autosave.json" ] || fail "autosave"
echo "autosave ok"
echo "PASS"
