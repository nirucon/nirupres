"""NIRUX appearance bridge for NIRUPRES editor chrome.

Only application chrome follows NIRUX. Slide themes remain part of the deck and
must look identical on every machine. No daemon or network access is used.
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import time
import tomllib
from dataclasses import dataclass
from pathlib import Path

NOIR = {
    "background": "0f0f10",
    "foreground": "e5e5e5",
    "muted": "a8a8a8",
    "border": "2a2a2d",
    "focus": "5a5a60",
}


def command_path(name: str) -> str | None:
    found = shutil.which(name)
    if found:
        return found
    for candidate in (Path.home() / ".local/bin" / name, Path("/usr/local/bin") / name, Path("/usr/bin") / name):
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    return None


def xdg_config_home() -> Path:
    value = os.environ.get("XDG_CONFIG_HOME", "").strip()
    return Path(value).expanduser() if value else Path.home() / ".config"


def xdg_data_home() -> Path:
    value = os.environ.get("XDG_DATA_HOME", "").strip()
    return Path(value).expanduser() if value else Path.home() / ".local/share"


def nirux_config_paths() -> tuple[Path, Path]:
    root = xdg_config_home() / "niruwm"
    return root / "config.toml", root / "appearance.toml"


@dataclass(frozen=True)
class ChromePalette:
    background: str
    toolbar: str
    input: str
    button: str
    button_hover: str
    hover: str
    foreground: str
    muted: str
    border: str
    border_strong: str
    focus: str
    accent: str
    on_accent: str
    disabled: str
    font_size: int
    reduced_motion: bool
    active_theme: str
    source: str


def _hex(value: object, fallback: str) -> str:
    text = str(value or "").strip().lstrip("#").lower()
    if len(text) == 3:
        text = "".join(ch * 2 for ch in text)
    if len(text) == 6 and all(ch in "0123456789abcdef" for ch in text):
        return text
    return fallback


def _rgb(value: str) -> tuple[int, int, int]:
    return tuple(int(value[i:i + 2], 16) for i in (0, 2, 4))


def _hexrgb(rgb: tuple[int, int, int]) -> str:
    return "".join(f"{max(0, min(255, int(round(ch)))):02x}" for ch in rgb)


def blend(a: str, b: str, amount: float) -> str:
    amount = max(0.0, min(1.0, amount))
    ar, ag, ab = _rgb(a)
    br, bg, bb = _rgb(b)
    return _hexrgb((ar + (br - ar) * amount, ag + (bg - ag) * amount, ab + (bb - ab) * amount))


def luminance(value: str) -> float:
    channels = []
    for channel in _rgb(value):
        c = channel / 255.0
        channels.append(c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4)
    return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]


def contrast_text(background: str) -> str:
    return "090909" if luminance(background) > 0.42 else "f6f3ed"


def _read_toml(path: Path) -> dict:
    try:
        with path.open("rb") as stream:
            value = tomllib.load(stream)
        return value if isinstance(value, dict) else {}
    except (OSError, tomllib.TOMLDecodeError):
        return {}


def _catalog(config_home: Path | None = None, catalog_json: str | None = None) -> dict:
    if catalog_json is not None:
        try:
            value = json.loads(catalog_json)
            return value if isinstance(value, dict) else {}
        except json.JSONDecodeError:
            return {}
    command = command_path("niructl")
    if not command:
        return {}
    env = dict(os.environ)
    if config_home is not None:
        env["XDG_CONFIG_HOME"] = str(config_home)
    try:
        result = subprocess.run(
            [command, "theme-catalog", "--json"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=2.0,
            check=False,
            env=env,
        )
    except (OSError, subprocess.TimeoutExpired):
        return {}
    if result.returncode != 0:
        return {}
    try:
        value = json.loads(result.stdout.decode("utf-8", "replace"))
        return value if isinstance(value, dict) else {}
    except json.JSONDecodeError:
        return {}


def detect_nirux(config_home: Path | None = None) -> bool:
    config_home = config_home or xdg_config_home()
    config, appearance = (config_home / "niruwm" / "config.toml", config_home / "niruwm" / "appearance.toml")
    if os.environ.get("NIRUPRES_CHROME", "").strip().lower() == "noir":
        return False
    return config.is_file() or appearance.is_file()


def load_nirux_chrome(
    config_home: Path | None = None,
    catalog_json: str | None = None,
) -> ChromePalette | None:
    config_home = config_home or xdg_config_home()
    config_path, appearance_path = nirux_config_paths() if config_home is None else (
        config_home / "niruwm" / "config.toml",
        config_home / "niruwm" / "appearance.toml",
    )
    if not detect_nirux(config_home):
        return None
    config = _read_toml(config_path)
    appearance = _read_toml(appearance_path)
    catalog = _catalog(config_home, catalog_json)
    active = str(catalog.get("active") or config.get("theme") or "niru-noir").strip().lower()
    colors = dict(NOIR)
    active_name = active
    for theme in catalog.get("themes", []) if isinstance(catalog.get("themes"), list) else []:
        if not isinstance(theme, dict) or str(theme.get("id", "")).lower() != active:
            continue
        active_name = str(theme.get("name") or active)
        found = theme.get("colors") if isinstance(theme.get("colors"), dict) else {}
        for key in NOIR:
            colors[key] = _hex(found.get(key), colors[key])
        break
    overrides = appearance.get("colors") if isinstance(appearance.get("colors"), dict) else {}
    for key in NOIR:
        if key in overrides:
            colors[key] = _hex(overrides[key], colors[key])
    font = appearance.get("font") if isinstance(appearance.get("font"), dict) else {}
    try:
        font_size = int(round(float(font.get("size", 14))))
    except (TypeError, ValueError):
        font_size = 14
    font_size = max(12, min(17, font_size))
    motion = appearance.get("motion") if isinstance(appearance.get("motion"), dict) else {}
    reduced = bool(motion.get("reduced", False))

    background = colors["background"]
    foreground = colors["foreground"]
    muted = colors["muted"]
    border = colors["border"]
    focus = colors["focus"]
    accent = _hex(overrides.get("accent", focus), focus)
    return ChromePalette(
        background=background,
        toolbar=blend(background, foreground, 0.045),
        input=blend(background, foreground, 0.075),
        button=blend(background, foreground, 0.105),
        button_hover=blend(background, foreground, 0.16),
        hover=blend(background, foreground, 0.20),
        foreground=foreground,
        muted=muted,
        border=border,
        border_strong=blend(border, foreground, 0.20),
        focus=focus,
        accent=accent,
        on_accent=contrast_text(accent),
        disabled=blend(foreground, background, 0.58),
        font_size=font_size,
        reduced_motion=reduced,
        active_theme=active_name,
        source=str(catalog.get("active") or config.get("theme") or active),
    )


def chrome_style(base: str, palette: ChromePalette) -> str:
    """Apply semantic NIRUX colours to the existing editor QSS."""
    replacements = {
        "#080808": palette.background,
        "#0b0b0b": palette.toolbar,
        "#0d0d0d": palette.toolbar,
        "#0e0e0e": palette.input,
        "#121212": palette.button,
        "#151515": palette.button_hover,
        "#191919": palette.hover,
        "#202020": palette.hover,
        "#242424": palette.border,
        "#292929": palette.border,
        "#303030": palette.border_strong,
        "#3a3a3a": palette.border_strong,
        "#454545": palette.border_strong,
        "#484848": palette.focus,
        "#4a4a4a": palette.focus,
        "#555": palette.muted,
        "#555555": palette.muted,
        "#5d5d5d": palette.disabled,
        "#666": palette.muted,
        "#737373": palette.muted,
        "#777": palette.focus,
        "#8c8c8c": palette.focus,
        "#9a9a9a": palette.focus,
        "#a8a8a8": palette.muted,
        "#aaa": palette.muted,
        "#c6c6c6": palette.foreground,
        "#d0d0d0": palette.foreground,
        "#d2d2d2": palette.foreground,
        "#d7d7d7": palette.focus,
        "#d8d8d8": palette.foreground,
        "#ddd": palette.foreground,
        "#e8e8e8": palette.foreground,
        "#eee": palette.focus,
        "#f0f0f0": palette.focus,
        "#f2f2f2": palette.foreground,
        "#f4f4f4": palette.focus,
        "#fff": palette.focus,
        "#111": palette.input,
        "#141414": palette.button_hover,
        "#222": palette.hover,
        "#333": palette.border_strong,
        "#414141": palette.focus,
        "#090909": palette.on_accent,
    }
    style = base
    for old, new in replacements.items():
        style = re.sub(re.escape(old) + r"(?![0-9a-fA-F])", f"#{new}", style)
    style = style.replace("font-size:13px", f"font-size:{palette.font_size}px")
    return style


def chrome_preference_path() -> Path:
    return xdg_data_home() / "nirupres" / "chrome.json"


def read_follow_preference() -> bool | None:
    path = chrome_preference_path()
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
        return bool(value["follow_nirux"]) if isinstance(value, dict) and "follow_nirux" in value else None
    except (OSError, json.JSONDecodeError, TypeError):
        return None


def write_follow_preference(follow: bool) -> None:
    path = chrome_preference_path()
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_suffix(".tmp")
        temporary.write_text(json.dumps({"follow_nirux": bool(follow)}, indent=2) + "\n", encoding="utf-8")
        temporary.replace(path)
    except OSError:
        pass


def watch_paths(config_home: Path | None = None) -> list[Path]:
    config_home = config_home or xdg_config_home()
    root = config_home / "niruwm"
    return [root / "config.toml", root / "appearance.toml"]


def status_text(palette: ChromePalette | None, follow: bool) -> str:
    if not follow:
        return "Editor chrome: Noir (NIRUX follow off)"
    if palette is None:
        return "Editor chrome: NIRUX unavailable · Noir fallback"
    return f"Editor chrome: NIRUX · {palette.active_theme}"


def monotonic() -> float:
    return time.monotonic()
