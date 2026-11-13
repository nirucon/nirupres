"""NIRUPRES 2.4 release-gate checks: visible whole-show fades, aiming from the
presenter view, and the CURRENT preview mirroring what the audience sees.

Imported and run by ``release_gate.py`` (``run(m, app, check)``).
"""

from PySide6.QtCore import QElapsedTimer, QEvent, QPointF, Qt
from PySide6.QtGui import QMouseEvent
from PySide6.QtWidgets import QApplication

from gate_compat import QTest


def _settle(app, rounds=10):
    for _ in range(rounds):
        app.processEvents()


def _deck(m, n=3, aspect="16:9", motion=True):
    deck = m.clone(m.DEFAULT)
    deck["theme"] = "W&B"  # light slides: a fade from black is measurable
    deck["aspect"] = aspect
    deck["transition"] = "FADE"
    deck["cinematicOpen"] = motion
    deck["cinematicClose"] = motion
    deck["slides"] = [
        m.slide_defaults({"layout": "STATEMENT", "title": f"Slide {i+1}", "notes": f"note {i+1}"})
        for i in range(n)
    ]
    return deck


def _luma(widget):
    """Mean brightness (0..255) of a rendered frame of the widget, 8x8 samples."""
    img = widget.grab().toImage()
    if img.isNull() or img.width() < 8 or img.height() < 8:
        return -1.0
    total = 0
    for gx in range(8):
        for gy in range(8):
            c = img.pixelColor(int((gx + 0.5) * img.width() / 8), int((gy + 0.5) * img.height() / 8))
            total += (c.red() * 299 + c.green() * 587 + c.blue() * 114) / 1000
    return total / 64.0


def _sample(app, fn, ms, step=40):
    """Call fn every ``step`` ms for ``ms`` ms; returns [(t, value)]."""
    out = []
    clock = QElapsedTimer()
    clock.start()
    while clock.elapsed() < ms:
        out.append((clock.elapsed(), fn()))
        QTest.qWait(step)
    return out


def check_opening_fade(m, app, check):
    screen = app.primaryScreen()
    s = m.Presenter(_deck(m), 0, screen, screen, online=True)
    eng = s.audience.transition_engine
    mirror = s.mirror
    check(mirror is not None and s.audience.mirror is mirror, "presenter preview mirror missing")
    # The black cover is there before the first frame, in both views.
    check(eng.cover_opacity() == 1.0, f"opening cover not black at start: {eng.cover_opacity()}")
    check(mirror.engine.cover_opacity() == 1.0, "preview opening cover not black at start")
    frames = _sample(
        app,
        lambda: (eng.cover_opacity(), _luma(s.audience.view), _luma(s.now)),
        m.OPEN_WAIT_MAX_MS + m.OPEN_SETTLE_MS + m.OPEN_FADE_MS + 600,
    )
    ops = [(t, v[0]) for t, v in frames if v[0] is not None]
    mid = [t for t, o in ops if 0.1 < o < 0.9]
    check(len(mid) >= 4, f"opening fade shows too few intermediate frames: {ops}")
    started = next((t for t, o in ops if o < 0.98), None)
    ended = next((t for t, v in frames if v[0] is None), None)
    check(started is not None and ended is not None, f"opening fade did not run: {ops}")
    if started is not None and ended is not None:
        span = ended - started
        check(550 <= span <= 1300, f"opening fade took {span} ms (want about 900)")
    # Real frames: dark -> intermediate -> bright, in the audience and in the preview.
    aud = [v[1] for _t, v in frames]
    pre = [v[2] for _t, v in frames]
    for name, series in (("audience", aud), ("preview", pre)):
        lo, hi = min(series), max(series)
        check(lo < 40 and hi > 180, f"{name} frames never went from black to slide: {lo}..{hi}")
        inter = [x for x in series if lo + 0.2 * (hi - lo) < x < hi - 0.2 * (hi - lo)]
        check(len(inter) >= 2, f"{name} frames show no intermediate brightness: {series}")
    check(not s.audience._opening and eng.overlay is None, "opening cover left behind")
    check(mirror.engine.overlay is None, "preview opening cover left behind")
    # Input is never blocked: the cover lets mouse events through.
    s.audience.begin_opening()
    cover = eng.overlay
    check(
        cover is not None and cover.testAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents),
        "opening cover blocks the mouse",
    )
    # Navigating during the opening ends it at once (no stale black cover).
    s.navigate(1)
    _settle(app)
    check(s.i == 1 and not s.audience._opening, "navigation during the opening failed")
    check(eng.cover_opacity() is None and mirror.engine.cover_opacity() is None,
          "opening cover survived navigation")
    s._finish_session()
    _settle(app)


def check_closing_fade(m, app, check):
    screen = app.primaryScreen()
    deck = _deck(m)
    deck["cinematicOpen"] = False
    s = m.Presenter(deck, 0, screen, screen, online=True)
    _settle(app)
    ended = []
    s.finished.connect(lambda: ended.append(1))
    eng = s.audience.transition_engine
    s.close_session()
    check("ENDING" in s.audience_state.text(), "presenter view does not say the show is ending")
    frames = _sample(
        app, lambda: (eng.cover_opacity(), s.mirror.engine.cover_opacity()), m.CLOSE_FADE_MS + 120
    )
    mid = [v for _t, v in frames if v[0] is not None and 0.1 < v[0] < 0.9]
    check(len(mid) >= 4, f"closing fade shows too few intermediate frames: {frames}")
    check(all(v[1] is not None for _t, v in frames), "preview does not mirror the closing fade")
    check(eng.cover_opacity() == 1.0 and not ended, "closing fade did not reach black")
    s.navigate(1)
    check(s.i == 0, "navigation must not undo the closing black")
    QTest.qWait(m.BLACK_HOLD_MS + 250)
    _settle(app)
    check(bool(ended), "session did not close after the black hold")

    # Esc during the opening fades out from where the fade is, and Esc twice
    # closes at once.
    s = m.Presenter(_deck(m), 0, screen, screen, online=True)
    ended = []
    s.finished.connect(lambda: ended.append(1))
    eng = s.audience.transition_engine
    clock = QElapsedTimer()
    clock.start()
    while clock.elapsed() < 4000:
        o = eng.cover_opacity()
        if o is not None and o < 0.6:
            break
        QTest.qWait(15)
    before = eng.cover_opacity()
    s.close_session()
    _settle(app, 3)
    after = eng.cover_opacity()
    check(before is not None and after is not None and after >= before - 0.05,
          f"closing jumped instead of continuing the opening fade: {before} -> {after}")
    QTest.qWait(m.OPEN_SETTLE_MS + m.OPEN_FADE_MS)
    check(eng.cover_opacity() == 1.0, "opening resumed after Esc")
    s.close_session()
    _settle(app)
    check(bool(ended), "second Escape did not close at once")


def check_end_after_last_slide(m, app, check):
    screen = app.primaryScreen()
    deck = _deck(m)
    deck["cinematicOpen"] = False
    s = m.Presenter(deck, 2, screen, screen, online=True)
    _settle(app)
    ended = []
    s.finished.connect(lambda: ended.append(1))
    eng = s.audience.transition_engine
    s.navigate(1)
    check(s._end_black and s.i == 2, "Next on the last slide did not fade to black")
    check("END OF SLIDES" in s.audience_state.text(), "presenter view does not show the black end")
    QTest.qWait(m.CLOSE_FADE_MS + 100)
    check(eng.cover_opacity() == 1.0 and s.mirror.engine.cover_opacity() == 1.0,
          "black end did not reach black in both views")
    check(not ended, "black end must wait for Next or Esc")
    s.navigate(-1)
    check(not s._end_black and s.i == 2, "Previous did not leave the black end")
    QTest.qWait(m.OPEN_FADE_MS + 100)
    check(eng.cover_opacity() is None, "slides did not fade back in")
    s.navigate(1)
    s.navigate(1)
    _settle(app)
    check(bool(ended), "Next on the black end did not close")
    # Without cinematic close, Next on the last slide does nothing.
    deck = _deck(m, motion=False)
    s = m.Presenter(deck, 2, screen, screen, online=True)
    _settle(app)
    s.navigate(1)
    check(not s._end_black and s.i == 2 and s.isVisible(), "no-motion deck changed at the end")
    s._finish_session()
    _settle(app)


def _move(widget, p, kind=QEvent.Type.MouseMove, button=Qt.NoButton):
    glob = QPointF(widget.mapToGlobal(p.toPoint()))
    ev = QMouseEvent(kind, QPointF(p), glob, button, button, Qt.NoModifier)
    QApplication.sendEvent(widget, ev)


def check_aim_from_preview(m, app, check):
    screen = app.primaryScreen()
    for aspect in ("16:9", "4:3"):
        s = m.Presenter(_deck(m, aspect=aspect, motion=False), 0, screen, screen, online=True)
        _settle(app)
        aud = s.audience
        src = s.now.slide_rect()
        dst = aud.view.slide_rect()
        check(src.width() > 50 and dst.width() > 50, f"no slide rects: {src} {dst}")
        QTest.keyClick(s, Qt.Key_S)
        _settle(app)
        check(aud.focus_overlay.mode == "SPOTLIGHT", "S did not start Spotlight")
        for nx, ny in ((0.25, 0.4), (0.9, 0.1), (0.5, 0.5)):
            _move(s.now, QPointF(src.x() + nx * src.width(), src.y() + ny * src.height()))
            _settle(app, 3)
            got = aud.focus_overlay.point
            want = QPointF(dst.x() + nx * dst.width(), dst.y() + ny * dst.height())
            check(abs(got.x() - want.x()) < 2 and abs(got.y() - want.y()) < 2,
                  f"{aspect}: preview aim ({nx},{ny}) -> {got} want {want}")
            mp = s.mirror.focus.point
            check(
                abs(mp.x() - (src.x() + nx * src.width())) < 2
                and abs(mp.y() - (src.y() + ny * src.height())) < 2,
                f"{aspect}: preview focus not under the mouse: {mp}",
            )
        check(s.mirror.focus.mode == "SPOTLIGHT" and not s.mirror.focus.isHidden(),
              "preview does not show the Spotlight")
        ratio = src.width() / dst.width()
        check(abs(s.mirror.focus.radius - aud.focus_overlay.radius * ratio) < 1,
              "preview Spotlight radius not scaled")
        QTest.keyClick(s, Qt.Key_BracketRight)
        _settle(app)
        check(abs(s.mirror.focus.radius - aud.focus_overlay.radius * ratio) < 1,
              "] not mirrored in the preview")
        # Outside the slide (letterbox) clamps to the slide edge.
        _move(s.now, QPointF(src.x() - 40, src.y() - 40))
        _settle(app, 3)
        got = aud.focus_overlay.point
        check(abs(got.x() - dst.x()) < 2 and abs(got.y() - dst.y()) < 2,
              f"{aspect}: aim outside the slide not clamped: {got}")
        # Click on the preview pins the focus on the audience.
        pin = QPointF(src.x() + 0.7 * src.width(), src.y() + 0.3 * src.height())
        _move(s.now, pin, QEvent.Type.MouseButtonPress, Qt.LeftButton)
        _move(s.now, pin, QEvent.Type.MouseButtonRelease, Qt.LeftButton)
        _settle(app)
        check(aud.focus_overlay.pinned and s.mirror.focus.pinned, "click on the preview did not pin")
        check(s.i == 0, "click on the preview must not change the slide")
        # Zoom follows the aim, is mirrored, and covers the view near an edge.
        QTest.keyClick(s, Qt.Key_Z)
        QTest.qWait(480)
        _settle(app)
        check(aud._zoomed and s.mirror.zoom.zoomed, "Z not mirrored in the preview")
        check(aud.focus_overlay.isHidden() and s.mirror.focus.isHidden(),
              "Spotlight veil must rest while zoomed")
        for view, zoom in ((aud.view, aud.zoom), (s.now, s.mirror.zoom)):
            g = zoom.overlay.geometry()
            check(g.contains(view.rect()), f"zoomed slide does not cover the view: {g}")
        s.navigate(1)
        _settle(app)
        check(not aud._zoomed and not s.mirror.zoom.zoomed, "navigation did not clear zoom")
        check(not aud.focus_overlay.isHidden() and not s.mirror.focus.isHidden(),
              "Spotlight did not come back after zoom")
        QTest.keyClick(s, Qt.Key_S)
        _settle(app)
        check(aud.focus_overlay.mode is None and s.mirror.focus.isHidden(),
              "S again did not end Spotlight in both views")
        s._finish_session()
        _settle(app)


def check_preview_transition(m, app, check):
    screen = app.primaryScreen()
    s = m.Presenter(_deck(m, motion=False), 0, screen, screen, online=True)
    _settle(app)
    s.navigate(1)
    check(s.mirror.engine.overlay is not None, "slide transition not mirrored in the preview")
    QTest.qWait(450)
    _settle(app)
    check(s.mirror.engine.overlay is None, "preview transition overlay left behind")
    s._finish_session()
    _settle(app)


def check_key_events_with_modifiers(m, app, check):
    # PySide6 6.12 modifier flags do not convert with int(); key handlers
    # used to raise on every unhandled key with a modifier (Ctrl, Shift, Super).
    from PySide6.QtGui import QKeyEvent

    screen = app.primaryScreen()
    s = m.Presenter(_deck(m, motion=False), 0, screen, screen, online=True)
    _settle(app)
    errors = []
    for target in (s, s.audience):
        for key, mods in ((Qt.Key_A, Qt.ControlModifier), (Qt.Key_2, Qt.MetaModifier),
                          (Qt.Key_Return, Qt.ShiftModifier)):
            ev = QKeyEvent(QEvent.Type.KeyPress, key, mods, "")
            try:
                target.keyPressEvent(ev)
                s.eventFilter(target, ev)
            except Exception as exc:  # pragma: no cover - reported by check
                errors.append(f"{type(exc).__name__}: {exc}")
    check(not errors, f"key with modifier raised: {errors[:1]}")
    # Typed slide number + Enter jumps (goes through the event filter).
    s.jump = "3"
    s.eventFilter(s, QKeyEvent(QEvent.Type.KeyPress, Qt.Key_Return, Qt.NoModifier, "\r"))
    check(s.i == 2, f"number + Enter jump failed: {s.i}")
    s._finish_session()
    _settle(app)


def check_notes_size_with_app_style(m, app, check):
    screen = app.primaryScreen()
    app.setStyleSheet(m.STYLE)
    try:
        s = m.Presenter(_deck(m, motion=False), 0, screen, screen, online=True, style=m.STYLE)
        _settle(app, 20)
        px = s.notes.font().pixelSize()
        check(px == s.notes_px, f"notes start at {px}px, want {s.notes_px}px (A-/A+ size)")
        s._finish_session()
        _settle(app)
    finally:
        app.setStyleSheet("")


def run(m, app, check):
    check_opening_fade(m, app, check)
    check_closing_fade(m, app, check)
    check_end_after_last_slide(m, app, check)
    check_aim_from_preview(m, app, check)
    check_preview_transition(m, app, check)
    check_key_events_with_modifiers(m, app, check)
    check_notes_size_with_app_style(m, app, check)
