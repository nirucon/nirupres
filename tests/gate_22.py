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
from PySide6.QtGui import QFontDatabase


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
            # The no-scroll budget is defined for the theme font (Noto Sans, as on
            # NIRUX hosts); wider fallback fonts may scroll but never squeeze.
            if palette is None and "Noto Sans" in QFontDatabase.families():
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


def check_toolbar(m, app, check):
    from PySide6.QtWidgets import QToolButton

    w = m.Main()
    sections = list(w.toolbar_sections)
    check(sections == ["File", "Slide", "View", "Present"], f"toolbar groups: {sections}")
    texts = {}
    for name, items in w.toolbar_sections.items():
        texts[name] = []
        for item in items:
            widget = w.toolbar.widgetForAction(item)
            if item.isSeparator() or widget is None:
                continue
            text = (
                widget.defaultAction().text()
                if isinstance(widget, QToolButton) and widget.defaultAction()
                else widget.text()
            )
            texts[name].append(text)
    check(
        texts["File"] == ["NEW", "OPEN", "SAVE", "EXPORT", "IMPORT MD"]
        and texts["Slide"] == ["+ SLIDE", "DUPLICATE", "DELETE", "↶", "↷"]
        and texts["Present"] == ["▶ PRESENT", "|▶", "▣", "● TALK", "◉ ONLINE", "▦"],
        f"toolbar grouping changed: {texts}",
    )
    separators = sum(1 for items in w.toolbar_sections.values() for x in items if x.isSeparator())
    check(separators == 3, "toolbar groups need visible separators")
    shortcuts = {a.shortcut().toString() for a in w.actions() if not a.shortcut().isEmpty()}
    for key in (
        "Ctrl+N",
        "Ctrl+O",
        "Ctrl+S",
        "Ctrl+Shift+S",
        "Ctrl+Enter",
        "Ctrl+D",
        "Del",
        "Ctrl+Z",
        "Ctrl+Shift+Z",
        "Tab",
        "Ctrl+K",
        "F5",
        "Shift+F5",
        "Ctrl+Shift+E",
    ):
        check(key in shortcuts, f"window shortcut missing: {key} ({sorted(shortcuts)})")
    w._available_width = lambda: 1920
    w.resize(1920, 1000)
    w.show()
    _settle(app)
    check(
        not w.toolbar_hidden_sections and not w.overflow_action.isVisible(),
        "1920 px must show every group",
    )
    w.resize(1100, 760)
    _settle(app)
    check(
        "View" in w.toolbar_hidden_sections and w.overflow_action.isVisible(),
        "narrow toolbar must overflow into ⋯",
    )
    check(
        all(not x.isVisible() for x in w.toolbar_sections["View"]),
        "overflowed View group still on the toolbar",
    )
    w._fill_overflow_menu()
    menu_texts = [a.text() for a in w.overflow_menu.actions()]
    for text in ("FOCUS", "NIRUX", "⌘", "About NIRUPRES / diagnostics"):
        check(text in menu_texts, f"overflow menu lacks {text}: {menu_texts}")
    check(w.toolbar.width() >= w._toolbar_width_needed() - 12, "toolbar still overflows at 1100 px")
    check(
        all(x.isVisible() for x in w.toolbar_sections["Present"]), "Present group must stay visible"
    )
    w.resize(1920, 1000)
    _settle(app)
    check(not w.overflow_action.isVisible(), "overflow must disappear when wide again")
    w.close()


def check_presenter_idle(m, app, check):
    deck = m.clone(m.DEFAULT)
    deck["transition"] = "FADE"
    deck["slides"] = [
        m.slide_defaults({"title": "Still"}),
        m.slide_defaults(
            {
                "title": "Moving",
                "backgroundImage": "/nonexistent/bg.jpg",
                "backgroundMotion": "KEN BURNS",
            }
        ),
    ]
    screen = app.primaryScreen()
    session = m.Presenter(deck, 0, screen, screen, online=True)
    _settle(app)
    aud = session.audience
    check(not aud._motion_timer.isActive(), "motion timer must idle on a slide without motion")
    session.navigate(1)
    _settle(app)
    check(aud._motion_timer.isActive(), "KEN BURNS slide must animate")
    session.set_blank("B")
    check(not aud._motion_timer.isActive(), "motion must pause while the audience is black")
    session.set_blank(None)
    check(aud._motion_timer.isActive(), "motion must resume after black")
    session.set_blank("W")
    check(not aud._motion_timer.isActive(), "motion must pause while the audience is white")
    session.set_blank(None)
    session.toggle_curtain()
    check(not aud._motion_timer.isActive(), "motion must pause behind the curtain")
    session.toggle_curtain()
    check(aud._motion_timer.isActive(), "motion must resume after the curtain")
    session.toggle_pause()
    _settle(app)
    check(not aud._motion_timer.isActive(), "motion must pause during PAUS")
    tick = aud.pause_overlay._tick
    check(tick.isActive() and tick.remainingTime() >= 300, "pause clock must not wake every 100 ms")
    session.toggle_pause()
    _settle(app)
    check(aud._motion_timer.isActive(), "motion must resume after PAUS")
    # A finished pan stops the timer: emulate the end of the 18 s movement.
    aud._motion_clock = _FakeClock(int(m.KEN_BURNS_MS) + 10)
    aud.draw()
    check(not aud._motion_timer.isActive(), "motion timer must stop once the pan is complete")
    session.navigate(-1)
    _settle(app)
    check(not aud._motion_timer.isActive(), "motion timer must stop when leaving the slide")
    session.close_session()
    _settle(app)
    reduced = m.clone(deck)
    reduced["reduceMotion"] = True
    quiet = m.Audience(reduced, 1, online=True)
    check(not quiet._motion_timer.isActive(), "Reduce motion must keep the timer off")
    quiet.close()


class _FakeClock:
    def __init__(self, elapsed):
        self._elapsed = elapsed

    def elapsed(self):
        return self._elapsed

    def restart(self):
        self._elapsed = 0
        return 0

    def isValid(self):
        return True


def check_autosave_status(m, app, check):
    m.AUTOSAVE.unlink(missing_ok=True)
    w = m.Main()
    check(w.state.isVisibleTo(w), "save state must be visible in the status bar")
    w.title.setText("Edited")
    check("Unsaved" in w.state.text(), f"dirty state not shown: {w.state.text()!r}")
    w.autosave()
    check(m.AUTOSAVE.exists(), "autosave file not written")
    check(w.state.text().startswith("● Autosaved "), f"autosave state: {w.state.text()!r}")
    with tempfile.TemporaryDirectory(prefix="nirupres-save-") as td:
        w.path = Path(td) / "deck.nirupres"
        check(w.save(), "fixture save failed")
        check(w.state.text().startswith("Saved "), f"saved state: {w.state.text()!r}")
        check(not m.AUTOSAVE.exists(), "save must clear the autosave")
    w.dirty = False
    w.close()
    payload = {"source": "", "data": m.clone(m.DEFAULT)}
    payload["data"]["slides"][0]["title"] = "Recovered title"
    for choice, restored, kept in (
        ("restore", True, True),
        (None, False, True),
        ("discard", False, False),
    ):
        m.AUTOSAVE.write_text(json.dumps(payload), encoding="utf-8")
        r = m.Main()
        asked = []
        r._ask_recovery = lambda info, c=choice: (asked.append(info), c)[1]
        r.offer_recovery()
        check(asked and "Restore it, or discard" in asked[0], "recovery prompt text missing")
        got = r.data["slides"][0]["title"] == "Recovered title"
        check(got == restored, f"recovery choice {choice!r} restored={got}")
        if restored:
            check(
                r.state.text() == "● Recovered from autosave" and r.dirty,
                "recovered state not shown",
            )
        check(
            m.AUTOSAVE.exists() == kept,
            f"recovery choice {choice!r} autosave kept={m.AUTOSAVE.exists()}",
        )
        r.dirty = False
        r.offer_recovery = lambda: None
        r.close()
    m.AUTOSAVE.unlink(missing_ok=True)


def check_slide_list_keyboard(m, app, check):
    from PySide6.QtCore import Qt

    w = m.Main()
    w.data["slides"] = [m.slide_defaults({"title": t}) for t in ("One", "Two", "Three")]
    w.data["slides"][2]["hidden"] = True
    w.refresh(0)
    w.show()
    _settle(app)
    w.list.setFocus()
    _settle(app)
    titles = lambda: [s["title"] for s in w.data["slides"]]
    QTest.keyClick(w.list, Qt.Key_Down, Qt.AltModifier)
    _settle(app)
    check(titles() == ["Two", "One", "Three"], f"Alt+Down did not move the slide: {titles()}")
    check(w.list.currentRow() == 1, "moved slide must stay selected")
    check(w.dirty, "keyboard move must mark the deck modified")
    QTest.keyClick(w.list, Qt.Key_Down, Qt.AltModifier)
    QTest.keyClick(w.list, Qt.Key_Up, Qt.AltModifier)
    _settle(app)
    check(titles() == ["Two", "One", "Three"], f"Alt+Down/Alt+Up round trip failed: {titles()}")
    QTest.keyClick(w.list, Qt.Key_Up, Qt.AltModifier)
    QTest.keyClick(w.list, Qt.Key_Up, Qt.AltModifier)
    _settle(app)
    check(titles() == ["One", "Two", "Three"], f"Alt+Up at the top must be a no-op: {titles()}")
    w.undo()
    check(titles() == ["Two", "One", "Three"], "keyboard move must be undoable")
    names = [w.list.item(i).data(Qt.AccessibleTextRole) for i in range(w.list.count())]
    check(
        names[0] == "Slide 1 of 3: Two (TITLE)"
        and names[2] == "Slide 3 of 3: Three (TITLE, hidden)",
        f"slide accessible names: {names}",
    )
    thumb = w.list.itemWidget(w.list.item(1))
    check(
        thumb is not None and thumb.accessibleName() == names[1],
        "thumbnail accessible name missing",
    )
    w.title.setText("Renamed")
    check(
        w.list.item(w.list.currentRow()).data(Qt.AccessibleTextRole).endswith(": Renamed (TITLE)"),
        "accessible name must follow title edits",
    )
    check(w.list.accessibleName() == "Slides", "slide list accessible name missing")
    w.dirty = False
    w.close()


def _project_json(path):
    import zipfile

    with zipfile.ZipFile(path) as z:
        return z.read("project.json")


def check_round_trip(m, app, check):
    """open -> save -> reopen -> save: unchanged decks keep byte-identical JSON."""
    from PySide6.QtGui import QColor, QImage

    with tempfile.TemporaryDirectory(prefix="nirupres-roundtrip-") as td:
        root = Path(td)
        photo = root / "photo.jpg"
        img = QImage(1600, 900, QImage.Format.Format_RGB32)
        img.fill(QColor("#556677"))
        img.save(str(photo), "JPEG", 90)
        fixtures = {}
        plain = m.clone(m.DEFAULT)
        plain["slides"] = [
            m.slide_defaults({"layout": "BULLETS", "title": "Plan", "body": "a\nb", "notes": "n"}),
            m.slide_defaults(
                {
                    "layout": "TWO COLUMN",
                    "title": "Två",
                    "body": "L\n\n|||\n\nR",
                    "leftColumn": "L",
                    "rightColumn": "R",
                    "twoColumnStructured": True,
                }
            ),
        ]
        fixtures["plain.json"] = plain
        legacy = {
            "title": "Legacy",
            "theme": "UDDEVALLA",
            "slides": [{"layout": "TITLE", "title": "Old"}],
        }
        fixtures["legacy.json"] = legacy
        branded = m.clone(m.DEFAULT)
        branded.update({"theme": "UDDEVALLA PURPLE", "showLogo": False, "footerText": "Fot"})
        branded["slides"] = [
            m.slide_defaults(
                {
                    "layout": "SPLIT",
                    "title": "Bild",
                    "image": str(photo),
                    "backgroundImage": str(photo),
                    "backgroundMotion": "KEN BURNS",
                    "logoMode": "ON",
                    "accentOverride": "GREEN",
                }
            )
        ]
        fixtures["branded.json"] = branded
        for name, data in fixtures.items():
            src = root / name
            src.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
            w = m.Main()
            w.open_path(str(src))
            first = root / (src.stem + "-1.nirupres")
            w.path = first
            check(w.save(), f"round trip save failed: {name}")
            w.close()
            again = m.Main()
            again.open_path(str(first))
            check(not again.dirty, f"reopened deck marked modified: {name}")
            second = root / (src.stem + "-2.nirupres")
            again.path = second
            check(again.save(), f"round trip re-save failed: {name}")
            again.close()
            a, b = _project_json(first), _project_json(second)
            check(a == b, f"round trip changed project.json bytes: {name}")
            data = json.loads(a)
            check(data["format"] == 9, f"project format changed: {name}")
            if name == "branded.json":
                check(
                    data["showLogo"] is False and data["theme"] == "UDDEVALLA PURPLE",
                    "deck logo/theme changed in round trip",
                )
            if name == "legacy.json":
                check(
                    data["showLogo"] is True and data["theme"] == "UDDEVALLA BLUE",
                    "legacy deck defaults changed in round trip",
                )


def run(m, app, check):
    check_round_trip(m, app, check)
    check_slide_list_keyboard(m, app, check)
    check_autosave_status(m, app, check)
    check_presenter_idle(m, app, check)
    check_toolbar(m, app, check)
    check_profiles(m, app, check)
    check_label_fit(m, app, check)
    check_inspector(m, app, check)
