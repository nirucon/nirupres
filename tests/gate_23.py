"""NIRUPRES 2.3 release-gate checks: safe sharing, presenter view, template fit.

Imported and run by ``release_gate.py`` (``run(m, app, check)``).
"""

import json
import os
import stat
import tempfile
from pathlib import Path

from PySide6.QtWidgets import QTextEdit

from gate_compat import QTest


def _settle(app, rounds=10):
    for _ in range(rounds):
        app.processEvents()


def _deck(m, n=4):
    deck = m.clone(m.DEFAULT)
    deck["transition"] = "NONE"
    deck["cinematicOpen"] = False
    deck["cinematicClose"] = False
    deck["slides"] = [
        m.slide_defaults({"layout": "STATEMENT", "title": f"Slide {i+1}",
                          "notes": f"PRIVATE NOTE {i+1}"})
        for i in range(n)
    ]
    return deck


def check_titles_and_privacy(m, app, check):
    screen = app.primaryScreen()
    s = m.Presenter(_deck(m), 0, screen, screen, online=True)
    _settle(app)
    check(s.audience.windowTitle() == "AUDIENCE · share this · NIRUPRES", "audience title")
    check(s.windowTitle() == "PRIVATE presenter · do not share", "presenter title")
    check("PRIVATE" in s.private_bar.text() and s.private_bar.isVisible(), "PRIVATE bar missing")
    check(not s.audience.findChildren(QTextEdit), "audience window must not hold a notes widget")
    texts = " ".join(w.text() for w in s.audience.findChildren(m.QLabel) if hasattr(w, "text"))
    check("PRIVATE NOTE" not in texts, "notes leaked into the audience window")
    check("PRIVATE NOTE 1" in s.notes.toPlainText(), "notes missing in presenter view")
    g = s.audience.geometry()
    check(abs(g.width() / g.height() - 16 / 9) < 0.02, f"audience not 16:9: {g}")
    # Share flash overlay marks only the audience window.
    s.audience.flash_share(300)
    _settle(app)
    check(s.audience._share_flash is not None and s.audience._share_flash.isVisible(),
          "share flash not shown")
    QTest.qWait(450)
    _settle(app)
    check(not s.audience._share_flash.isVisible(), "share flash did not end")
    s._finish_session()
    _settle(app)


def check_presenter_layout(m, app, check):
    screen = app.primaryScreen()
    s = m.Presenter(_deck(m), 0, screen, screen, online=True)
    _settle(app)
    for (w, h, compact_ok) in ((1366, 768, None), (2560, 1440, False), (800, 480, True)):
        s.showNormal()
        s.setGeometry(0, 0, w, h)
        s.resize(w, h)
        _settle(app, 20)
        b = s.body
        b.relayout()
        notes_h = b.notes.height()
        check(notes_h >= s.height() / 3 - 50, f"notes pane too small at {w}x{h}: {notes_h}")
        check(b.now.width() >= min(300, w * 0.3) and b.now.isVisibleTo(s), f"current slide too small at {w}x{h}")
        check(b.timer.font().pixelSize() >= 30, f"timer not big at {w}x{h}")
        if compact_ok is False:
            check(not b.compact_next and b.next.isVisibleTo(s), f"next slide hidden at {w}x{h}")
        if compact_ok is True:
            check(b.compact_next and b.next_title.isVisibleTo(s), f"next did not collapse at {w}x{h}")
        for child in (b.now, b.next, b.timer, b.clock, b.notes):
            if child.isVisibleTo(s):
                cg = child.geometry()
                check(cg.right() <= b.width() + 1 and cg.bottom() <= b.height() + 1,
                      f"presenter widget outside the window at {w}x{h}")
    before = s.notes_px
    s.notes_zoom(4)
    check(s.notes_px == before + 4 and s.notes.font().pixelSize() == before + 4, "A+ failed")
    s._finish_session()
    _settle(app)
    s2 = m.Presenter(_deck(m), 0, screen, screen, online=True)
    _settle(app)
    check(s2.notes_px == before + 4, "notes size was not remembered")
    s2.notes_zoom(-4)
    s2._finish_session()
    _settle(app)


def check_share_check(m, app, check):
    screen = app.primaryScreen()
    s = m.Presenter(_deck(m), 0, screen, screen, online=True, share_check=True)
    QTest.qWait(900)
    _settle(app)
    d = s.share_dialog
    check(d is not None and d.isVisible(), "share check dialog missing")
    labels = " ".join(x.text() for x in d.findChildren(m.QLabel))
    check("AUDIENCE · share this · NIRUPRES" in labels and "Window" in labels,
          "share check does not name the window to pick")
    check(s.audience._share_flash is not None and s.audience._share_flash.isVisible(),
          "share check did not flash the audience window")
    s.handle_key(int(m.Qt.Key_Right))
    check(s.i == 0, "keys must not navigate during the share check")
    d.accept()
    _settle(app)
    check(not s._share_checking and s.share_dialog is None, "share check did not finish")
    s.handle_key(int(m.Qt.Key_Right))
    check(s.i == 1, "navigation after the share check failed")
    s._finish_session()
    _settle(app)
    # Cancel ends the session.
    ended = []
    s = m.Presenter(_deck(m), 0, screen, screen, online=True, share_check=True)
    s.finished.connect(lambda: ended.append(1))
    QTest.qWait(600)
    s.share_dialog.reject()
    _settle(app)
    check(ended, "cancelling the share check must end the session")


def _fake_niructl(root, has_present):
    log = Path(root) / "calls.log"
    script = Path(root) / "niructl"
    present = (
        'present) case "$2" in status) echo off;; esac; exit 0;;'
        if has_present
        else 'present) echo "unknown command" >&2; exit 2;;'
    )
    script.write_text(
        "#!/bin/sh\n"
        f'echo "$*" >> "{log}"\n'
        'case "$1" in\n'
        f"  {present}\n"
        '  dnd) case "$2" in status) echo "dnd off";; esac; exit 0;;\n'
        "esac\nexit 1\n"
    )
    script.chmod(script.stat().st_mode | stat.S_IEXEC)
    return script, log


def check_presenting_mode(m, app, check):
    from nirupres import presenting

    screen = app.primaryScreen()
    old = os.environ.get("NIRUPRES_NIRUCTL")
    try:
        for has_present in (True, False):
            with tempfile.TemporaryDirectory(prefix="nirupres-ctl-") as td:
                script, log = _fake_niructl(td, has_present)
                os.environ["NIRUPRES_NIRUCTL"] = str(script)
                s = m.Presenter(_deck(m), 0, screen, screen, online=True)
                _settle(app)
                started = log.read_text().splitlines()
                if has_present:
                    check(f"present on --pid {os.getpid()}" in started,
                          f"present on --pid not called: {started}")
                else:
                    check("dnd on" in started, f"fallback dnd on not called: {started}")
                    state = json.loads(presenting.STATE.read_text())
                    check(state.get("prior") == "off", "fallback did not store the DND state")
                s._finish_session()
                _settle(app)
                calls = log.read_text().splitlines()
                check(calls[-1] == ("present off" if has_present else "dnd off"),
                      f"presenting mode not restored: {calls}")
                check(not presenting.STATE.exists(), "presenting state file left behind")
                # Crash recovery for the fallback: stale state, dead pid.
                if not has_present:
                    presenting.STATE.write_text(
                        json.dumps({"mode": "dnd", "prior": "off", "pid": 999999999})
                    )
                    check(presenting.recover_stale() == "dnd", "stale DND state not recovered")
                    check(log.read_text().splitlines()[-1] == "dnd off", "crash restore failed")
    finally:
        if old is None:
            os.environ.pop("NIRUPRES_NIRUCTL", None)
        else:
            os.environ["NIRUPRES_NIRUCTL"] = old


def check_single_screen(m, app, check):
    if len(app.screens()) != 1:
        return
    w = m.Main()
    w.data = _deck(m)
    w.refresh(0)
    w.show()
    _settle(app)
    w._save_display_prefs(singleScreen="laptop")
    w.present(False)
    _settle(app)
    p = w.presenter
    check(p is not None and p.online and p.dual, "F5 on one screen did not use laptop mode")
    check(not w.isVisible(), "editor must be hidden while presenting")
    QTest.qWait(600)
    if p.share_dialog is not None:
        p.share_dialog.accept()
    _settle(app)
    p.close_session()
    QTest.qWait(50)
    p._finish_session()
    _settle(app)
    check(w.presenter is None and w.isVisible(), "editor not restored after presenting")
    w._save_display_prefs(singleScreen=None)
    w.dirty = False
    w.close()
    _settle(app)


def check_fit_and_health(m, app, check):
    import template_fit

    failures, _notes = template_fit.run(m)
    check(not failures, "template overflow:\n" + "\n".join(failures[:12]))
    deck = _deck(m, 1)
    deck["slides"] = [
        m.slide_defaults({"layout": "BULLETS", "title": "Fine", "body": "One\nTwo"}),
        m.slide_defaults({"layout": "BULLETS", "title": "Crowded",
                          "body": "\n".join(f"Point {i}" for i in range(11))}),
        m.slide_defaults({"layout": "STATEMENT", "title": "word " * 400}),
    ]
    check(m.slide_fit_issues(deck["slides"][0], deck, 0) == [], "false fit issue")
    crowded = m.slide_fit_issues(deck["slides"][1], deck, 1)
    check(any("bullets hidden" in x for x in crowded), f"hidden bullets not reported: {crowded}")
    long_issue = m.slide_fit_issues(deck["slides"][2], deck, 2)
    check(any("does not fit" in x for x in long_issue), f"overflow not reported: {long_issue}")
    thumb = m.Thumb(deck["slides"][1], "B&W", 1, 3, deck=deck)
    check(thumb.fit_issues and thumb.findChild(m.QLabel, "fitBadge") is not None,
          "thumbnail fit badge missing")
    w = m.Main()
    w.data = deck
    w.refresh(0)
    issues = w.validate_deck(silent=True)
    check(any(i == 1 and "hidden" in msg for i, _k, msg in issues), "deck health misses slide 2")
    check(any(i == 2 and "does not fit" in msg for i, _k, msg in issues),
          "deck health misses slide 3")
    w.dirty = False
    w.close()
    # CODE keeps indentation through Markdown.
    code = m.clone(m.DEFAULT)
    code["slides"] = [m.slide_defaults({"layout": "CODE", "title": "Code",
                                        "body": "def f():\n    return 1\n\n\nprint(f())"})]
    rt = m.import_nirupres_markdown(m.export_nirupres_markdown(code))
    check(rt["slides"][0]["body"] == code["slides"][0]["body"],
          f"CODE body changed in Markdown round trip: {rt['slides'][0]['body']!r}")
    check({"CODE", "IMAGE + CAPTION", "THREE COLUMN"} <= set(m.LAYOUTS), "new layouts missing")


def run(m, app, check):
    check_titles_and_privacy(m, app, check)
    check_presenter_layout(m, app, check)
    check_share_check(m, app, check)
    check_presenting_mode(m, app, check)
    check_single_screen(m, app, check)
    check_fit_and_health(m, app, check)
