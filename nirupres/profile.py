"""Organization profiles: opt-in identity for new decks and editor wording.

A profile only decides *defaults* and *labels*: whether a new deck starts with
the logo switch on, which starter themes the New dialog offers, the title preset
name and placeholder texts. Decks always store their own ``showLogo`` value, so
switching profile never changes an existing presentation, and every theme
(including the Uddevalla family) stays available in every profile.

The selected profile lives in ``$XDG_DATA_HOME/nirupres/profile.json``. On the
first start without that file, an installation that already has NIRUPRES
settings (or that ``install.sh`` upgraded from a pre-2.2 release) keeps the
Uddevalla profile it always had; a fresh installation starts neutral.
"""

import json
import os
from pathlib import Path

PROFILE_FILE = "profile.json"
DEFAULT_PROFILE = "neutral"
# Files a pre-2.2 NIRUPRES writes under its data root once it has been used.
LEGACY_SETTINGS = (
    "autosave.json",
    "recent.json",
    "display.json",
    "file-workflow.json",
    "chrome.json",
)

PROFILES = {
    "neutral": {
        "label": "Neutral",
        "show_logo_default": False,
        "logo_tooltip": (
            "Master switch for the organization logo on themes that carry one"
            " · slide AUTO/ON/OFF is in STYLE"
        ),
        "logo_checkbox": "Show organization logo (Uddevalla themes)",
        "logo_row": "Logo",
        "footer_placeholder": "Optional footer · e.g. Team or event · 2026-09-24",
        "footer_placeholder_short": "Optional · e.g. Team or event · 2026-09-24",
        "title_preset": "Title",
        "accent_tooltip": "Per-slide accent override for Uddevalla themes",
        "starters": [("Blank", "B&W"), ("Light", "W&B"), ("Gray", "GRAY")],
    },
    "uddevalla": {
        "label": "Uddevalla",
        "show_logo_default": True,
        "logo_tooltip": (
            "Master switch for the official Uddevalla kommun logo · slide AUTO/ON/OFF is in STYLE"
        ),
        "logo_checkbox": "Show official Uddevalla logo",
        "logo_row": "Uddevalla logo",
        "footer_placeholder": "Optional footer · e.g. Socialtjänsten · 2026-09-24",
        "footer_placeholder_short": "Optional · e.g. Socialtjänsten · 2026-09-24",
        "title_preset": "Uddevalla title",
        "accent_tooltip": "Per-slide Uddevalla accent override",
        "starters": [
            ("Blank", "B&W"),
            ("Uddevalla Blue", "UDDEVALLA BLUE"),
            ("Uddevalla Dark", "UDDEVALLA DARK"),
            ("Uddevalla Black", "UDDEVALLA BLACK"),
        ],
    },
}


def _data_root():
    return Path(os.environ.get("XDG_DATA_HOME") or (Path.home() / ".local/share")) / "nirupres"


def _write_profile(path, name, reason):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(
        json.dumps({"profile": name, "reason": reason}, indent=2) + "\n", encoding="utf-8"
    )
    tmp.replace(path)


def resolve_profile(data_root=None, persist=True):
    """Return the active profile name, creating profile.json once if missing.

    Idempotent: an existing valid file always wins and is never rewritten. An
    unreadable file is left untouched (the owner may want to fix it by hand) and
    the legacy detection is used for this run only.
    """
    root = Path(data_root) if data_root is not None else _data_root()
    path = root / PROFILE_FILE
    unreadable = False
    if path.exists():
        try:
            name = json.loads(path.read_text(encoding="utf-8")).get("profile")
            if name in PROFILES:
                return name
        except (OSError, ValueError, AttributeError):
            pass
        unreadable = True
    legacy = any((root / item).exists() for item in LEGACY_SETTINGS)
    name = "uddevalla" if legacy else DEFAULT_PROFILE
    if persist and not unreadable:
        try:
            _write_profile(path, name, "existing settings" if legacy else "fresh install")
        except OSError:
            pass
    return name


def set_profile(name, data_root=None):
    if name not in PROFILES:
        raise ValueError(f"unknown profile: {name}")
    root = Path(data_root) if data_root is not None else _data_root()
    _write_profile(root / PROFILE_FILE, name, "selected")
    return name


def profile_info(name):
    return PROFILES.get(name, PROFILES[DEFAULT_PROFILE])


def apply_new_deck_defaults(deck, name):
    """Stamp profile defaults onto a freshly created deck (never a loaded one)."""
    deck["showLogo"] = bool(profile_info(name)["show_logo_default"])
    return deck
