"""NIRUPRES 2.2 release-gate checks (profiles, label fit, inspector, toolbar,
presenter idle, autosave status, slide-list accessibility, round trips).

Called from tests/release_gate.py after the historical contracts. Every check
runs inside the gate's isolated XDG roots.
"""

import json
import tempfile
from pathlib import Path

from gate_compat import QTest


def _main(m, profile=None):
    if profile is not None:
        m.org_profile.set_profile(profile, m.DATA)
    w = m.Main()
    return w


def check_profiles(m, app, check):
    prof = m.org_profile
    with tempfile.TemporaryDirectory(prefix="nirupres-profile-") as td:
        fresh = Path(td) / "fresh"
        fresh.mkdir()
        check(prof.resolve_profile(fresh) == "neutral", "fresh install must start neutral")
        check(
            json.loads((fresh / "profile.json").read_text())["profile"] == "neutral",
            "fresh profile not persisted",
        )
        (fresh / "recent.json").write_text("[]")
        check(
            prof.resolve_profile(fresh) == "neutral",
            "persisted profile must win over later settings files",
        )
        owner = Path(td) / "owner"
        owner.mkdir()
        recent = b'["/home/owner/deck.nirupres"]'
        (owner / "recent.json").write_bytes(recent)
        check(prof.resolve_profile(owner) == "uddevalla", "existing install must keep Uddevalla")
        first = (owner / "profile.json").read_bytes()
        check(prof.resolve_profile(owner) == "uddevalla", "owner migration not idempotent")
        check((owner / "profile.json").read_bytes() == first, "migration rewrote profile.json")
        check((owner / "recent.json").read_bytes() == recent, "migration changed owner settings")
        broken = Path(td) / "broken"
        broken.mkdir()
        (broken / "profile.json").write_text("{not json")
        (broken / "display.json").write_text("{}")
        check(prof.resolve_profile(broken) == "uddevalla", "unreadable profile fallback failed")
        check(
            (broken / "profile.json").read_text() == "{not json",
            "unreadable profile.json must not be overwritten",
        )
    # Editor defaults per profile. Existing decks keep their stored logo value.
    neutral = _main(m, "neutral")
    check(neutral.data["showLogo"] is False, "neutral profile: new deck must not show logo")
    preset_labels = [a.text() for a in neutral.presets_menu.actions()]
    check(
        "Uddevalla title" not in preset_labels and "Title" in preset_labels,
        "neutral profile still offers the Uddevalla title preset",
    )
    check("Socialtjänsten" not in neutral.footer.placeholderText(), "neutral footer placeholder")
    ds = m.DeckSettings(neutral, neutral.data)
    check("Uddevalla logo" not in ds.logo.text(), "neutral deck settings still say Uddevalla logo")
    ds.close()
    neutral.close()
    udd = _main(m, "uddevalla")
    check(udd.data["showLogo"] is True, "Uddevalla profile: new deck must keep the logo default")
    check(udd.title_preset_action.text() == "Uddevalla title", "Uddevalla title preset missing")
    check("Socialtjänsten" in udd.footer.placeholderText(), "Uddevalla placeholder changed")
    udd.set_profile("neutral")
    check(udd.title_preset_action.text() == "Title", "live profile switch did not relabel")
    check(udd.data["showLogo"] is True, "profile switch changed the open deck")
    udd.close()
    with tempfile.TemporaryDirectory(prefix="nirupres-legacy-deck-") as td:
        legacy = Path(td) / "legacy.json"
        payload = m.clone(m.DEFAULT)
        payload.pop("showLogo")
        payload["theme"] = "UDDEVALLA BLUE"
        legacy.write_text(json.dumps(payload), encoding="utf-8")
        loaded = m.load_project(legacy, td)
        check(loaded["showLogo"] is True, "a deck without showLogo must keep showing the logo")
    m.org_profile.set_profile("neutral", m.DATA)


def run(m, app, check):
    check_profiles(m, app, check)
