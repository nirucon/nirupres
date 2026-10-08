#!/bin/sh
set -eu

VERSION="2.0.0"
SRC=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
DEST="$HOME/.local/share/nirupres"
BIN_DIR="$HOME/.local/bin"
BIN="$BIN_DIR/nirupres"
APP_DIR="$HOME/.local/share/applications"
DESKTOP="$APP_DIR/nirupres.desktop"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
ICON="$ICON_DIR/nirupres.svg"
VENV="$DEST/.venv"
STAGE=""
BACKUP=""
OLD_VERSION="none"

fail() { printf '\nERROR: %s\n' "$*" >&2; exit 1; }
cleanup() { [ -z "$STAGE" ] || [ ! -d "$STAGE" ] || rm -rf -- "$STAGE"; [ -z "$BACKUP" ] || [ ! -d "$BACKUP" ] || rm -rf -- "$BACKUP"; }
restore() {
    [ -n "$BACKUP" ] && [ -d "$BACKUP" ] || return 0
    rm -rf -- "$DEST/nirupres" "$DEST/tests"
    rm -f -- "$DEST/run.sh" "$DEST/requirements.txt" "$BIN" "$DESKTOP" "$ICON"
    [ ! -d "$BACKUP/nirupres" ] || mv -- "$BACKUP/nirupres" "$DEST/nirupres"
    [ ! -d "$BACKUP/tests" ] || mv -- "$BACKUP/tests" "$DEST/tests"
    [ ! -f "$BACKUP/run.sh" ] || mv -- "$BACKUP/run.sh" "$DEST/run.sh"
    [ ! -f "$BACKUP/requirements.txt" ] || mv -- "$BACKUP/requirements.txt" "$DEST/requirements.txt"
    [ ! -f "$BACKUP/nirupres-bin" ] || mv -- "$BACKUP/nirupres-bin" "$BIN"
    [ ! -f "$BACKUP/nirupres.desktop" ] || mv -- "$BACKUP/nirupres.desktop" "$DESKTOP"
    [ ! -f "$BACKUP/nirupres.svg" ] || { mkdir -p -- "$ICON_DIR"; mv -- "$BACKUP/nirupres.svg" "$ICON"; }
}
abort_install() { msg=$1; restore || true; fail "$msg"; }
trap cleanup EXIT HUP INT TERM

os_name="Linux"
if [ -r /etc/os-release ]; then os_name=$(sed -n 's/^PRETTY_NAME=//p' /etc/os-release | head -n 1 | sed 's/^"//;s/"$//'); [ -n "$os_name" ] || os_name="Linux"; fi
shell_name=${SHELL##*/}; [ -n "$shell_name" ] || shell_name="unknown"

printf 'NIRUPRES %s Release Gate installer\n\n' "$VERSION"
printf 'System\n  OS ............. %s\n  Shell .......... %s\n' "$os_name" "$shell_name"
command -v python3 >/dev/null 2>&1 || fail "python3 was not found in PATH."
mkdir -p -- "$HOME/.local/share" "$BIN_DIR" "$APP_DIR" "$ICON_DIR" "$DEST" || fail "Could not create install directories."
if [ -x "$VENV/bin/python" ] && [ -f "$DEST/nirupres/__init__.py" ]; then OLD_VERSION=$( (cd / && PYTHONPATH="$DEST" "$VENV/bin/python" -c 'import nirupres; print(nirupres.__version__)') 2>/dev/null || printf unknown); fi
printf '  Installed ...... %s\n  Target ......... %s\n\n' "$OLD_VERSION" "$VERSION"

printf '[1/6] Checking Python environment...\n'
if [ ! -x "$VENV/bin/python" ]; then
    printf '      Creating virtual environment...\n'
    python3 -m venv "$VENV" || fail "Could not create Python virtual environment. Ensure venv support is installed."
fi
"$VENV/bin/python" -c 'import sys; print("      Python",sys.version.split()[0])' || fail "Virtual environment is not usable."

printf '[2/6] Checking dependencies...\n'
if ! "$VENV/bin/python" -c 'from importlib.metadata import version; from packaging.version import Version; from PySide6.QtMultimedia import QMediaPlayer; from PySide6.QtMultimediaWidgets import QVideoWidget; from PySide6.QtWebEngineWidgets import QWebEngineView; q=Version(version("PySide6")); p=Version(version("python-pptx")); assert Version("6.7")<=q<Version("7") and Version("1.0")<=p<Version("2"); print(f"      OK — PySide6 {q} + Multimedia/WebEngine, python-pptx {p}")' 2>/dev/null; then
    printf '      Installing/repairing release dependencies...\n'
    "$VENV/bin/python" -m pip install -r "$SRC/requirements.txt" || fail "Dependency installation failed; installed application code was not changed."
fi
"$VENV/bin/python" -m pip check || fail "Python dependencies are inconsistent."

printf '[3/6] Staging and validating release...\n'
STAGE=$(mktemp -d "$HOME/.local/share/.nirupres-stage.XXXXXX") || fail "Could not create staging directory."
cp -R -- "$SRC/nirupres" "$SRC/tests" "$SRC/run.sh" "$SRC/requirements.txt" "$STAGE/" || fail "Could not stage release."
"$VENV/bin/python" -m compileall -q "$STAGE/nirupres" "$STAGE/tests" || fail "Staged release failed Python compilation."
staged_version=$(cd / && PYTHONPATH="$STAGE" "$VENV/bin/python" -c 'import nirupres; print(nirupres.__version__)') || fail "Could not read staged version."
[ "$staged_version" = "$VERSION" ] || fail "Staged version is $staged_version, expected $VERSION."
QT_QPA_PLATFORM=offscreen PYTHONPATH="$STAGE" "$VENV/bin/python" "$STAGE/tests/release_gate.py" || fail "Staged release gate failed. Existing application was not changed."

printf '[4/6] Creating rollback snapshot...\n'
BACKUP=$(mktemp -d "$HOME/.local/share/.nirupres-backup.XXXXXX") || fail "Could not create rollback directory."
[ ! -d "$DEST/nirupres" ] || mv -- "$DEST/nirupres" "$BACKUP/nirupres"
[ ! -d "$DEST/tests" ] || mv -- "$DEST/tests" "$BACKUP/tests"
[ ! -f "$DEST/run.sh" ] || mv -- "$DEST/run.sh" "$BACKUP/run.sh"
[ ! -f "$DEST/requirements.txt" ] || mv -- "$DEST/requirements.txt" "$BACKUP/requirements.txt"
[ ! -f "$BIN" ] || cp -p -- "$BIN" "$BACKUP/nirupres-bin"
[ ! -f "$DESKTOP" ] || cp -p -- "$DESKTOP" "$BACKUP/nirupres.desktop"
[ ! -f "$ICON" ] || cp -p -- "$ICON" "$BACKUP/nirupres.svg"

printf '[5/6] Installing transactionally...\n'
if ! mv -- "$STAGE/nirupres" "$DEST/nirupres" || ! mv -- "$STAGE/tests" "$DEST/tests" || ! cp -- "$STAGE/run.sh" "$STAGE/requirements.txt" "$DEST/"; then abort_install "Could not install release; previous managed files were restored."; fi
chmod +x -- "$DEST/run.sh"
cat > "$BIN" <<EOF
#!/bin/sh
exec "$DEST/run.sh" "\$@"
EOF
chmod +x -- "$BIN"
cp -- "$DEST/nirupres/assets/nirupres.svg" "$ICON" || abort_install "Could not install application icon; previous managed files were restored."
cat > "$DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Name=NIRUPRES
Comment=Minimal, suckless presentation app by Nicklas Rudolfsson
Exec=$BIN
Terminal=false
Icon=$ICON
Categories=Office;Presentation;
EOF

printf '[6/6] Verifying installed release...\n'
if ! "$VENV/bin/python" -m compileall -q "$DEST/nirupres" "$DEST/tests"; then abort_install "Installed code failed compilation; previous managed files were restored."; fi
installed_version=$( (cd / && PYTHONPATH="$DEST" "$VENV/bin/python" -c 'import nirupres; print(nirupres.__version__)') 2>/dev/null || true)
[ "$installed_version" = "$VERSION" ] || abort_install "Installed version is $installed_version, expected $VERSION; previous managed files were restored."
if ! QT_QPA_PLATFORM=offscreen PYTHONPATH="$DEST" "$VENV/bin/python" "$DEST/tests/release_gate.py"; then abort_install "Post-install release gate failed; previous managed files were restored."; fi
# Validate the launcher from an unrelated cwd: this protects the UWSM/desktop regression.
( cd /tmp && test -x "$BIN" && test -s "$ICON" && grep -F 'Icon=' "$DESKTOP" >/dev/null && grep -F 'cd "$ROOT"' "$DEST/run.sh" >/dev/null ) || abort_install "Launcher validation failed; previous managed files were restored."

rm -rf -- "$BACKUP"; BACKUP=""; rm -rf -- "$STAGE"; STAGE=""; trap - EXIT HUP INT TERM
printf '\nNIRUPRES %s installed successfully.\n' "$VERSION"
if [ "$OLD_VERSION" != "none" ]; then printf 'Upgrade: %s -> %s\n' "$OLD_VERSION" "$VERSION"; fi
printf 'User data, recent files, display preferences and the virtual environment were preserved.\nRun: nirupres\n'
