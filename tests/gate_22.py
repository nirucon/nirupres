"""NIRUPRES 2.2 release-gate checks (profiles, label fit, inspector, toolbar,
presenter idle, autosave status, slide-list accessibility, round trips).

Called from tests/release_gate.py after the historical contracts. Every check
runs inside the gate's isolated XDG roots.
"""

import dataclasses
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


def clipped_text_widgets(w):
    """Visible text controls narrower than their own size hint (elided/clipped)."""
    from PySide6.QtWidgets import QCheckBox, QGroupBox, QLabel, QPushButton, QToolButton

    clipped = []
    for cls in (QPushButton, QToolButton, QCheckBox, QLabel):
        for x in w.findChildren(cls):
            if not x.isVisibleTo(w) or not x.text():
                continue
            if isinstance(x, QLabel) and x.wordWrap():
                continue
            if x is getattr(w, "doc", None):  # elides itself with a tooltip
                continue
            if x.width() + 1 < x.sizeHint().width():
                clipped.append(f"{x.text()!r} {x.width()}<{x.sizeHint().width()}")
    for g in w.findChildren(QGroupBox):
        need = g.fontMetrics().horizontalAdvance(g.title()) + 16
        if g.isVisibleTo(w) and g.width() < need:
            clipped.append(f"group {g.title()!r} {g.width()}<{need}")
    return clipped


def _settle(app, rounds=12):
    for _ in range(rounds):
        app.processEvents()


def check_label_fit(m, app, check):
    catalog = {
        "active": "gate",
        "themes": [
            {
                "id": "gate",
                "name": "Gate",
                "colors": {
                    "background": "101010",
                    "foreground": "eeeeee",
                    "muted": "999999",
                    "border": "303030",
                    "focus": "abcdef",
                },
            }
        ],
    }
    with tempfile.TemporaryDirectory(prefix="nirupres-chrome-fit-") as td:
        (Path(td) / "niruwm").mkdir()
        (Path(td) / "niruwm" / "config.toml").write_text('theme = "gate"\n', encoding="utf-8")
        base = m.chrome_theme.load_nirux_chrome(Path(td), json.dumps(catalog))
    big = dataclasses.replace(base, font_size=15)
    for width, height in ((1366, 768), (1920, 1080)):
        for label, palette in (("default font", None), ("15px chrome font", big)):
            w = m.Main()
            # Size decisions follow the screen; emulate a screen of this size.
            w._available_width = lambda width=width: width
            if palette is not None:
                w.chrome_palette = palette
                w.setStyleSheet(m.chrome_theme.chrome_style(m.STYLE, palette))
                w.refit_text()
            w.resize(width, height)
            w.show()
            _settle(app)
            check(
                w.width() <= width and w.height() <= height,
                f"editor cannot fit {width}x{height} ({label}): {w.width()}x{w.height()}",
            )
            w.refit_text()
            _settle(app)
            scroll = w.inspector_scroll
            inner = scroll.widget()
            check(
                inner.width() >= inner.minimumSizeHint().width(),
                f"inspector controls squeezed at {width}x{height} ({label})",
            )
            if palette is None:
                check(
                    inner.width() <= scroll.viewport().width(),
                    f"inspector needs horizontal scrolling at {width}x{height} (default font)",
                )
            clipped = clipped_text_widgets(w)
            check(not clipped, f"clipped labels at {width}x{height} ({label}): {clipped}")
            w.close()


def check_inspector(m, app, check):
    (m.DATA / "ui.json").unlink(missing_ok=True)
    w = m.Main()
    groups = w._inspector_groups()
    check(
        {k: g.isChecked() for k, g in groups.items()}
        == {"CONTENT": True, "IMAGE": False, "VIDEO": False, "STYLE": False, "PRESENTATION": False},
        "inspector default must collapse every section except CONTENT",
    )
    w.data["slides"][0].update(
        {"image": "/nonexistent/photo.jpg", "imageMode": "FILL", "focalX": 0.5, "focalY": 0.5}
    )
    w.select(0)
    check(not groups["IMAGE"].isChecked(), "IMAGE section must not auto-expand")
    check(
        "IMAGE  ·  fill · center" in groups["IMAGE"].title(),
        f"collapsed IMAGE summary missing: {groups['IMAGE'].title()!r}",
    )
    check("Image: fill · center" in groups["IMAGE"].toolTip(), "IMAGE summary tooltip missing")
    check("VIDEO  ·  none" in groups["VIDEO"].title(), "collapsed VIDEO summary missing")
    w.notes.setPlainText("Speaker note")
    check("notes" in groups["PRESENTATION"].title(), "PRESENTATION summary did not follow edits")
    groups["STYLE"].setChecked(True)
    groups["CONTENT"].setChecked(False)
    check(groups["STYLE"].title() == "▼  STYLE", "expanded section must not show a summary")
    w.dirty = False
    w.close()
    again = m.Main()
    state = {k: g.isChecked() for k, g in again._inspector_groups().items()}
    check(
        state["STYLE"] and not state["CONTENT"] and not state["IMAGE"],
        f"inspector open/closed state not remembered: {state}",
    )
    check(
        again.content_group.title().startswith("▶  CONTENT  ·  "),
        "collapsed CONTENT summary missing after restart",
    )
    again.close()
    (m.DATA / "ui.json").write_text("{broken", encoding="utf-8")
    fallback = m.Main()
    check(fallback.content_group.isChecked(), "broken ui.json must fall back to defaults")
    fallback.close()
    (m.DATA / "ui.json").unlink(missing_ok=True)


def run(m, app, check):
    check_profiles(m, app, check)
    check_label_fit(m, app, check)
    check_inspector(m, app, check)
