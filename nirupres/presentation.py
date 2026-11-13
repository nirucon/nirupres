"""NIRUPRES 2.0.0 — extracted application subsystem.

Refactored from the verified 1.7.3 baseline. Existing project and Markdown
format versions remain unchanged.
"""

import json, sys, zipfile, tempfile, shutil, uuid, re, stat, os, platform, hashlib
from collections import OrderedDict
from pathlib import Path
from PySide6.QtCore import (
    Qt,
    QTimer,
    QSize,
    Signal,
    QRectF,
    QElapsedTimer,
    QEvent,
    QPropertyAnimation,
    QEasingCurve,
    QParallelAnimationGroup,
    QStandardPaths,
    QUrl,
    QPointF,
    QVariantAnimation,
    QObject,
)
from PySide6.QtGui import (
    QAction,
    QKeySequence,
    QShortcut,
    QPixmap,
    QPainter,
    QColor,
    QFont,
    QFontMetrics,
    QFontDatabase,
    QPdfWriter,
    QPageSize,
    QPageLayout,
    QImage,
    QImageReader,
    QIcon,
    QTextDocument,
    QAbstractTextDocumentLayout,
    QPainterPath,
)
from PySide6.QtWidgets import (
    QApplication,
    QMainWindow,
    QWidget,
    QVBoxLayout,
    QHBoxLayout,
    QListWidget,
    QListWidgetItem,
    QLineEdit,
    QTextEdit,
    QComboBox,
    QPushButton,
    QLabel,
    QFileDialog,
    QSplitter,
    QMessageBox,
    QToolBar,
    QCheckBox,
    QDialog,
    QGridLayout,
    QDialogButtonBox,
    QGraphicsScene,
    QGraphicsPixmapItem,
    QGraphicsBlurEffect,
    QSlider,
    QGroupBox,
    QFormLayout,
    QMenu,
    QInputDialog,
    QGraphicsOpacityEffect,
)

from .foundation import *
from . import x11props
from .presenting import PresentingMode
import time
from .foundation import (
    _safe_zip_name,
    _validate_project_data,
    _cached_pixmap,
    _image_dimensions,
    _optimize_asset,
    _paired_font,
    _parse_two_column_body,
    _plain_markup,
    _inline_html,
    _format_markdown_selection,
    _IMAGE_CACHE,
    _IMAGE_CACHE_BYTES,
)


def _mod_int(mods):
    """Keyboard modifiers as int; PySide6 6.12 flags no longer convert with int()."""
    try:
        return int(mods)
    except TypeError:
        return int(getattr(mods, "value", 0))


def _aspect_rect(r, aspect):
    """The letterboxed slide rectangle inside ``r`` for a deck aspect ratio."""
    r = QRectF(r)
    ar = {"16:9": 16 / 9, "16:10": 16 / 10, "4:3": 4 / 3, "A4": 297 / 210}.get(aspect, 16 / 9)
    if r.width() / max(1, r.height()) > ar:
        nw = r.height() * ar
        return QRectF(r.x() + (r.width() - nw) / 2, r.y(), nw, r.height())
    nh = r.width() / ar
    return QRectF(r.x(), r.y() + (r.height() - nh) / 2, r.width(), nh)


class SlideCanvas(QWidget):
    editRequested = Signal()

    def __init__(self, compact=False):
        super().__init__()
        self.s = {}
        self.theme = "B&W"
        self.i = 0
        self.total = 1
        self.showNumbers = False
        self.showLogo = True
        self.footerText = ""
        self.footerAlign = "LEFT"
        self.footerSize = "MEDIUM"
        self.compact = compact
        self.guides = False
        self.deckFont = "Noto Sans"
        self.aspect = "16:9"
        self.setMinimumSize(320, 180)

    def render(
        self,
        s,
        theme,
        i,
        total,
        show=True,
        deck_font="Noto Sans",
        aspect="16:9",
        show_logo=True,
        footer_text="",
        footer_align="LEFT",
        footer_size="MEDIUM",
    ):
        self.s = s
        self.theme = theme
        self.i = i
        self.total = total
        self.showNumbers = show
        self.showLogo = show_logo
        self.footerText = footer_text
        self.footerAlign = normalize_footer_align(footer_align)
        self.footerSize = normalize_footer_size(footer_size)
        self.deckFont = deck_font
        self.aspect = aspect
        self.update()

    def slide_rect(self):
        """Where the slide is drawn inside this widget (widget coordinates)."""
        return _aspect_rect(self.rect(), self.aspect)

    def paintEvent(self, e):
        p = QPainter(self)
        try:
            r = self.slide_rect()
            p.fillRect(self.rect(), QColor("#0B0B0B"))
            p.fillRect(r.adjusted(-1, -1, 1, 1), QColor("#2A2A2A"))
            try:
                render_slide(
                    p,
                    r,
                    self.s,
                    self.theme,
                    self.i,
                    self.total,
                    self.showNumbers and not self.compact,
                    self.deckFont,
                    self.showLogo,
                    self.footerText,
                    self.footerAlign,
                    self.footerSize,
                )
            except Exception as exc:
                print(f"NIRUPRES slide render warning: {exc}", file=sys.stderr)
                p.fillRect(r, QColor("#050505"))
                p.setPen(QColor("#F4F2ED"))
                p.drawText(r, Qt.AlignCenter, "Unable to render slide")
            if self.guides and not self.compact:
                p.save()
                try:
                    p.setPen(QColor(255, 255, 255, 55))
                    safe = r.adjusted(
                        r.width() * 0.055,
                        r.height() * 0.055,
                        -r.width() * 0.055,
                        -r.height() * 0.055,
                    )
                    p.drawRect(safe)
                    p.drawLine(
                        int(r.center().x()), int(r.top()), int(r.center().x()), int(r.bottom())
                    )
                    p.drawLine(
                        int(r.left()), int(r.center().y()), int(r.right()), int(r.center().y())
                    )
                finally:
                    p.restore()
        finally:
            if p.isActive():
                p.end()

    def mouseDoubleClickEvent(self, e):
        if not self.compact:
            self.editRequested.emit()
            e.accept()
        else:
            super().mouseDoubleClickEvent(e)


class Thumb(QWidget):
    def __init__(
        self,
        s,
        theme,
        i,
        total,
        show_logo=True,
        footer_text="",
        footer_align="LEFT",
        footer_size="MEDIUM",
        deck=None,
    ):
        super().__init__()
        l = QVBoxLayout(self)
        l.setContentsMargins(4, 4, 4, 5)
        l.setSpacing(3)
        c = SlideCanvas(True)
        c.setFixedSize(176, 99)
        c.render(
            s,
            theme,
            i,
            total,
            False,
            show_logo=show_logo,
            footer_text=footer_text,
            footer_align=footer_align,
            footer_size=footer_size,
        )
        l.addWidget(c)
        self.fit_issues = []
        if deck is not None:
            try:
                self.fit_issues = slide_fit_issues(s, deck, i)
            except Exception:
                self.fit_issues = []
        if self.fit_issues:
            # Deck health badge: the slide has text that does not fit.
            badge = QLabel("FIT", c)
            badge.setObjectName("fitBadge")
            badge.setStyleSheet(
                "QLabel#fitBadge{background:#E8A33D;color:#111;font-size:9px;font-weight:900;"
                "padding:1px 4px;border-radius:2px}"
            )
            badge.adjustSize()
            badge.move(3, 3)
            badge.setToolTip("Text does not fit:\n" + "\n".join(self.fit_issues))
            self.setToolTip("Deck health: " + "; ".join(self.fit_issues))
        flags = (
            ("  ◇" if s.get("notes") else "")
            + ("  ⊘" if s.get("hidden") else "")
            + ("  ●" if s.get("themeOverride", "DECK") != "DECK" else "")
            + ("  ⚠" if self.fit_issues else "")
        )
        q = QLabel(f'{i+1:02d}  {(s.get("title") or s.get("layout"))[:20]}{flags}')
        q.setObjectName("thumbtitle")
        l.addWidget(q)


class DropLine(QLineEdit):
    fileDropped = Signal(str)

    def __init__(self):
        super().__init__()
        self.setAcceptDrops(True)
        self.setPlaceholderText("Drop image here or choose…")

    def dragEnterEvent(self, e):
        if e.mimeData().hasUrls():
            e.acceptProposedAction()

    def dropEvent(self, e):
        for u in e.mimeData().urls():
            p = u.toLocalFile()
            if Path(p).suffix.lower() in (".png", ".jpg", ".jpeg", ".webp", ".bmp"):
                self.fileDropped.emit(p)
                e.acceptProposedAction()
                return


# Whole-show fades (2.4). The opening fade waits until the audience window is
# really on screen, so it is never spent before the window manager has mapped
# it; both fades are mirrored in the presenter's CURRENT preview.
OPEN_FADE_MS = 900
CLOSE_FADE_MS = 800
BLACK_HOLD_MS = 1000
OPEN_SETTLE_MS = 160
OPEN_WAIT_MAX_MS = 1500


class TransitionEngine(QObject):
    """Small, allocation-bounded transition engine for one slide view.

    It animates one snapshot or black cover overlay and never touches the slide
    renderer/model. This keeps editor rendering, exports and Presenter View
    deterministic. The black cover follows the view when the window manager
    resizes it (fullscreen and maximise settle after the first frame).
    """

    def __init__(self, view):
        super().__init__(view)
        self.view = view
        self.overlay = None
        self.group = None
        self.cover = False
        view.installEventFilter(self)

    def eventFilter(self, obj, e):
        if obj is self.view and e.type() == QEvent.Type.Resize and self.cover:
            try:
                self.overlay.setGeometry(self.view.rect())
            except (RuntimeError, AttributeError):
                pass
        return False

    def stop(self):
        if self.group is not None:
            try:
                self.group.stop()
            except RuntimeError:
                pass
        if self.overlay is not None:
            try:
                self.overlay.hide()
                self.overlay.deleteLater()
            except RuntimeError:
                pass
        self.group = None
        self.overlay = None
        self.cover = False

    def animate(self, old_pm, mode):
        self.stop()
        if old_pm is None or old_pm.isNull() or mode == "NONE":
            return
        overlay = QLabel(self.view)
        overlay.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, True)
        overlay.setPixmap(old_pm)
        overlay.setScaledContents(True)
        overlay.setGeometry(self.view.rect())
        overlay.show()
        overlay.raise_()
        effect = QGraphicsOpacityEffect(overlay)
        overlay.setGraphicsEffect(effect)
        effect.setOpacity(1.0)
        duration = {"FADE": 360, "MORPH": 560, "REVEAL": 460}.get(mode, 360)
        fade = QPropertyAnimation(effect, b"opacity", overlay)
        fade.setDuration(duration)
        fade.setStartValue(1.0)
        fade.setEndValue(0.0)
        fade.setEasingCurve(QEasingCurve.OutCubic)
        group = QParallelAnimationGroup(overlay)
        group.addAnimation(fade)
        r = self.view.rect()
        if mode == "MORPH":
            dx = max(7, int(r.width() * 0.014))
            dy = max(4, int(r.height() * 0.014))
            geo = QPropertyAnimation(overlay, b"geometry", overlay)
            geo.setDuration(duration)
            geo.setStartValue(r)
            geo.setEndValue(r.adjusted(-dx, -dy, dx, dy))
            geo.setEasingCurve(QEasingCurve.OutCubic)
            group.addAnimation(geo)
        elif mode == "REVEAL":
            dx = max(8, int(r.width() * 0.012))
            geo = QPropertyAnimation(overlay, b"geometry", overlay)
            geo.setDuration(duration)
            geo.setStartValue(r)
            geo.setEndValue(r.translated(-dx, 0))
            geo.setEasingCurve(QEasingCurve.OutCubic)
            group.addAnimation(geo)

        def done():
            if self.overlay is overlay:
                self.overlay = None
                self.group = None
            overlay.deleteLater()

        group.finished.connect(done)
        self.overlay = overlay
        self.group = group
        group.start()

    def cover_opacity(self):
        """Opacity of the black cover, or None when there is no cover."""
        if not self.cover or self.overlay is None:
            return None
        try:
            return self.overlay.graphicsEffect().opacity()
        except (RuntimeError, AttributeError):
            return None

    def hold_black(self):
        """Cover the view with black at once (before the first frame)."""
        self.stop()
        overlay = QLabel(self.view)
        overlay.setObjectName("fadeCover")
        overlay.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, True)
        overlay.setStyleSheet("background:#000;border:0")
        overlay.setGeometry(self.view.rect())
        effect = QGraphicsOpacityEffect(overlay)
        effect.setOpacity(1.0)
        overlay.setGraphicsEffect(effect)
        overlay.show()
        overlay.raise_()
        self.overlay = overlay
        self.cover = True

    def _fade(self, end, ms, finished):
        start = self.cover_opacity()
        if start is None:
            self.hold_black()
            start = 1.0 - end
        elif self.group is not None:
            try:
                self.group.stop()
            except RuntimeError:
                pass
        overlay = self.overlay
        effect = overlay.graphicsEffect()
        effect.setOpacity(start)
        overlay.raise_()
        anim = QPropertyAnimation(effect, b"opacity", overlay)
        # A fade that starts half-way (Esc during the opening) takes half the time.
        anim.setDuration(max(1, int(ms * abs(end - start))))
        anim.setStartValue(start)
        anim.setEndValue(end)
        anim.setEasingCurve(QEasingCurve.InOutSine)
        anim.finished.connect(lambda: finished(overlay))
        self.group = anim
        anim.start()

    def opening(self, callback=None):
        """Fade the black cover (or a new one) out over OPEN_FADE_MS."""

        def done(overlay):
            if self.overlay is overlay:
                self.stop()
            if callback:
                callback()

        self._fade(0.0, OPEN_FADE_MS, done)

    def closing(self, black_reached=None):
        # Fade only to black here. Presenter owns the short black hold and the
        # session lifetime, keeping visual animation separate from control state.
        completed = {"done": False}

        def reached(_overlay=None):
            if completed["done"]:
                return
            completed["done"] = True
            if black_reached:
                black_reached()

        self._fade(1.0, CLOSE_FADE_MS, reached)
        QTimer.singleShot(CLOSE_FADE_MS + 250, reached)


class LiveZoom:
    """Live Zoom: magnifies a snapshot of a view around a point (Z)."""

    def __init__(self, view):
        self.view = view
        self.overlay = None
        self.anim = None
        self.zoomed = False

    def toggle(self, pt):
        # Transforms a snapshot only; deck data and exports remain untouched.
        if self.zoomed:
            if self.overlay is not None:
                ov = self.overlay
                anim = QPropertyAnimation(ov, b"geometry", ov)
                anim.setDuration(280)
                anim.setStartValue(ov.geometry())
                anim.setEndValue(self.view.rect())
                anim.setEasingCurve(QEasingCurve.InOutCubic)

                def gone():
                    ov.deleteLater()
                    if self.overlay is ov:
                        self.overlay = None

                anim.finished.connect(gone)
                self.anim = anim
                anim.start()
            self.zoomed = False
            return
        pm = self.view.grab()
        if pm.isNull():
            return
        ov = QLabel(self.view)
        ov.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, True)
        ov.setPixmap(pm)
        ov.setScaledContents(True)
        ov.setGeometry(self.view.rect())
        ov.show()
        ov.raise_()
        if pt is None or pt.isNull():
            pt = QPointF(self.view.rect().center())
        r = self.view.rect()
        scale = 1.75
        nw = int(r.width() * scale)
        nh = int(r.height() * scale)
        tx = int(r.center().x() - (pt.x() / max(1, r.width())) * nw)
        ty = int(r.center().y() - (pt.y() / max(1, r.height())) * nh)
        # Near an edge the magnified slide must still cover the whole view.
        tx = min(0, max(r.width() - nw, tx))
        ty = min(0, max(r.height() - nh, ty))
        anim = QPropertyAnimation(ov, b"geometry", ov)
        anim.setDuration(420)
        anim.setStartValue(r)
        anim.setEndValue(QRectF(tx, ty, nw, nh).toRect())
        anim.setEasingCurve(QEasingCurve.InOutCubic)
        self.overlay = ov
        self.anim = anim
        self.zoomed = True
        anim.start()

    def clear(self):
        if self.overlay is not None:
            try:
                self.overlay.deleteLater()
            except RuntimeError:
                pass
            self.overlay = None
        self.zoomed = False


class PresenterFocusOverlay(QWidget):
    """Audience-only focus system: soft Spotlight, deep Focus Reveal and pinned Focus Trail.

    The presenter's CURRENT preview holds a mirror copy (``mirror``) so the
    presenter sees the same focus the audience sees.
    """

    changed = Signal()

    def __init__(self, parent):
        super().__init__(parent)
        self.mode = None
        self.point = QPointF()
        self.radius = 165
        self.pinned = False
        self._move_anim = None
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, True)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, True)
        self.hide()

    def set_mode(self, mode):
        if self.mode == mode:
            mode = None
        self.mode = mode
        self.pinned = False
        if mode == "SPOTLIGHT":
            self.radius = 165
        elif mode == "FOCUS":
            self.radius = 115
        self.setVisible(bool(mode))
        self.raise_()
        self.update()
        self.changed.emit()

    def set_point(self, p, force=False):
        if self.pinned and not force:
            return
        self.point = QPointF(p)
        self.update()
        self.changed.emit()

    def mirror(self, mode, point, radius, pinned):
        """Show a copy of another overlay's state (already mapped to this view)."""
        self.mode = mode
        self.point = QPointF(point)
        self.radius = radius
        self.pinned = pinned
        self.setGeometry(self.parentWidget().rect())
        self.setVisible(bool(mode))
        self.raise_()
        self.update()

    def pin_or_move(self, p):
        if not self.mode:
            return False
        target = QPointF(p)
        if not self.pinned:
            self.pinned = True
            self.point = target
            self.update()
            self.changed.emit()
            return True
        start = QPointF(self.point)
        anim = QVariantAnimation(self)
        anim.setDuration(360)
        anim.setStartValue(start)
        anim.setEndValue(target)
        anim.setEasingCurve(QEasingCurve.InOutCubic)
        anim.valueChanged.connect(lambda v: self.set_point(v, True))
        self._move_anim = anim
        anim.start()
        return True

    def unpin(self):
        self.pinned = False
        self.changed.emit()

    def adjust_radius(self, delta):
        self.radius = max(65, min(420, self.radius + delta))
        self.update()
        self.changed.emit()

    def paintEvent(self, e):
        if not self.mode:
            return
        q = QPainter(self)
        q.setRenderHint(QPainter.RenderHint.Antialiasing, True)
        outer = QPainterPath()
        outer.addRect(QRectF(self.rect()))
        hole = QPainterPath()
        hole.addEllipse(self.point, self.radius, self.radius)
        shade = outer.subtracted(hole)
        # Spotlight keeps context readable. Focus Reveal deliberately isolates the subject.
        alpha = 112 if self.mode == "SPOTLIGHT" else 218
        q.fillPath(shade, QColor(0, 0, 0, alpha))
        # A restrained feather ring prevents Focus Reveal from looking like a hard cut-out.
        if self.mode == "FOCUS":
            q.setPen(QColor(255, 255, 255, 20))
            q.drawEllipse(self.point, self.radius, self.radius)
        q.end()


KEN_BURNS_MS = 18000.0
AUDIENCE_TITLE = "AUDIENCE · share this · NIRUPRES"
PRESENTER_TITLE = "PRIVATE presenter · do not share"
SHARE_FLASH_MS = 5000


class ShareFlashOverlay(QWidget):
    """Coloured border and corner badge that marks the audience window.

    Shown for a few seconds by the Share check so the presenter can see in the
    Teams preview that the right window is shared. It contains no private
    information, so it is harmless if the audience sees it.
    """

    COLOR = QColor("#19C37D")

    def __init__(self, parent):
        super().__init__(parent)
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, True)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, True)
        self._timer = QTimer(self)
        self._timer.setSingleShot(True)
        self._timer.timeout.connect(self.hide)
        self.hide()

    def flash(self, ms=SHARE_FLASH_MS):
        self.setGeometry(self.parentWidget().rect())
        self.show()
        self.raise_()
        self._timer.start(ms)

    def paintEvent(self, e):
        q = QPainter(self)
        q.setRenderHint(QPainter.RenderHint.Antialiasing, True)
        r = QRectF(self.rect())
        bw = max(8.0, min(r.width(), r.height()) * 0.018)
        pen_rect = r.adjusted(bw / 2, bw / 2, -bw / 2, -bw / 2)
        pen = q.pen()
        pen.setColor(self.COLOR)
        pen.setWidthF(bw)
        q.setPen(pen)
        q.setBrush(Qt.NoBrush)
        q.drawRect(pen_rect)
        f = QFont("Noto Sans")
        f.setPixelSize(int(max(14, r.height() * 0.035)))
        f.setWeight(QFont.Weight.Black)
        q.setFont(f)
        label = "✓  SHARED WINDOW · AUDIENCE"
        fm = QFontMetrics(f)
        bwid = fm.horizontalAdvance(label) + f.pixelSize() * 1.4
        bh = f.pixelSize() * 2.0
        badge = QRectF(bw * 1.6, bw * 1.6, bwid, bh)
        q.setPen(Qt.NoPen)
        q.setBrush(self.COLOR)
        q.drawRoundedRect(badge, 4, 4)
        q.setPen(QColor("#04130C"))
        q.drawText(badge, Qt.AlignCenter, label)
        q.end()


def _web_video_url(url):
    url = str(url or "").strip()
    m = re.search(r"(?:youtube\.com/watch\?v=|youtu\.be/)([A-Za-z0-9_-]{6,})", url)
    if m:
        return f"https://www.youtube-nocookie.com/embed/{m.group(1)}?rel=0&modestbranding=1"
    m = re.search(r"vimeo\.com/(\d+)", url)
    if m:
        return f"https://player.vimeo.com/video/{m.group(1)}"
    return url


class PresenterPauseOverlay(QWidget):
    """Audience-only pause veil with an independent count-up clock."""

    def __init__(self, parent):
        super().__init__(parent)
        self._clock = QElapsedTimer()
        self._seconds = 0
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, True)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, True)
        # One wake-up per displayed second, aligned to the elapsed clock. The
        # 2.1 overlay repainted the full-screen veil every 100 ms.
        self._tick = QTimer(self)
        self._tick.setTimerType(Qt.TimerType.PreciseTimer)
        self._tick.setSingleShot(True)
        self._tick.timeout.connect(self._update_time)
        self._opacity = QGraphicsOpacityEffect(self)
        self.setGraphicsEffect(self._opacity)
        self._opacity.setOpacity(0.0)
        self._fade = None
        self.hide()

    def start_pause(self):
        self._seconds = 0
        self._clock.restart()
        self._schedule_tick()
        self.show()
        self.raise_()
        self._animate_opacity(1.0, 380)
        self.update()

    def stop_pause(self):
        self._tick.stop()
        self._animate_opacity(0.0, 300, hide_after=True)

    def _animate_opacity(self, target, duration, hide_after=False):
        if self._fade is not None:
            try:
                self._fade.stop()
            except Exception:
                pass
        anim = QPropertyAnimation(self._opacity, b"opacity", self)
        anim.setDuration(duration)
        anim.setStartValue(self._opacity.opacity())
        anim.setEndValue(target)
        anim.setEasingCurve(QEasingCurve.InOutCubic)
        if hide_after:
            anim.finished.connect(self.hide)
        self._fade = anim
        anim.start()

    def elapsed_seconds(self):
        # QElapsedTimer is the source of truth. QTimer only schedules repaints;
        # delayed timer delivery must never make the displayed pause clock stale.
        if not self._clock.isValid():
            return 0
        return max(0, self._clock.elapsed() // 1000)

    def _schedule_tick(self):
        elapsed = self._clock.elapsed() if self._clock.isValid() else 0
        self._tick.start(max(20, 1000 - elapsed % 1000 + 5))

    def _update_time(self):
        sec = self.elapsed_seconds()
        if sec != self._seconds:
            self._seconds = sec
            self.update()
        self._schedule_tick()

    def paintEvent(self, event):
        q = QPainter(self)
        q.setRenderHint(QPainter.RenderHint.Antialiasing, True)
        # Deep translucent veil: the slide remains recognizable without competing for attention.
        q.fillRect(self.rect(), QColor(0, 0, 0, 205))
        cx = self.rect().center().x()
        cy = self.rect().center().y()
        fg = QColor("#F4F2ED")
        muted = QColor(244, 242, 237, 165)
        title = QFont("Noto Sans")
        title.setPixelSize(max(28, int(min(self.width(), self.height()) * 0.055)))
        title.setWeight(QFont.Weight.DemiBold)
        title.setLetterSpacing(QFont.SpacingType.AbsoluteSpacing, 4)
        q.setFont(title)
        q.setPen(fg)
        tr = QRectF(0, cy - title.pixelSize() * 1.35, self.width(), title.pixelSize() * 1.5)
        q.drawText(tr, Qt.AlignmentFlag.AlignHCenter | Qt.AlignmentFlag.AlignVCenter, "PAUS")
        sec = self.elapsed_seconds()
        self._seconds = sec
        clock = f"{sec//60:02d}:{sec%60:02d}"
        tf = QFont("Noto Sans Mono")
        tf.setPixelSize(max(20, int(min(self.width(), self.height()) * 0.034)))
        tf.setWeight(QFont.Weight.Medium)
        q.setFont(tf)
        q.setPen(muted)
        rr = QRectF(0, cy + title.pixelSize() * 0.35, self.width(), tf.pixelSize() * 1.8)
        q.drawText(rr, Qt.AlignmentFlag.AlignHCenter | Qt.AlignmentFlag.AlignVCenter, clock)
        q.end()


class PreviewMirror(QObject):
    """The presenter's CURRENT preview as a small copy of the audience output.

    It repeats the whole-show fades, slide transitions, Spotlight/Focus Reveal
    and Live Zoom, so a presenter whose audience window is covered (laptop
    mode) or on another screen sees what the audience sees. The mouse over the
    preview aims Spotlight, Focus Reveal and Zoom on the audience window.
    """

    def __init__(self, canvas, audience):
        super().__init__(canvas)
        self.canvas = canvas
        self.audience = audience
        self.engine = TransitionEngine(canvas)
        self.focus = PresenterFocusOverlay(canvas)
        self.zoom = LiveZoom(canvas)
        canvas.setMouseTracking(True)
        canvas.installEventFilter(self)
        audience.focus_overlay.changed.connect(self.sync_focus)
        audience.mirror = self

    def to_audience(self, p):
        """Preview point -> audience view point (the same spot on the slide)."""
        src = self.canvas.slide_rect()
        dst = self.audience.view.slide_rect()
        if src.width() < 1 or src.height() < 1:
            return None
        nx = min(1.0, max(0.0, (p.x() - src.x()) / src.width()))
        ny = min(1.0, max(0.0, (p.y() - src.y()) / src.height()))
        return QPointF(dst.x() + nx * dst.width(), dst.y() + ny * dst.height())

    def from_audience(self, p):
        src = self.audience.view.slide_rect()
        dst = self.canvas.slide_rect()
        if src.width() < 1 or src.height() < 1:
            return QPointF(dst.center())
        nx = (p.x() - src.x()) / src.width()
        ny = (p.y() - src.y()) / src.height()
        return QPointF(dst.x() + nx * dst.width(), dst.y() + ny * dst.height())

    def scale(self):
        src = self.audience.view.slide_rect()
        return self.canvas.slide_rect().width() / max(1.0, src.width())

    def sync_focus(self):
        o = self.audience.focus_overlay
        point = self.from_audience(o.point) if not o.point.isNull() else QPointF()
        self.focus.mirror(o.mode if not o.isHidden() else None, point, o.radius * self.scale(), o.pinned)

    def transition(self, mode):
        # Called before the preview renders the new slide, so grab() is the old one.
        if mode == "NONE" or not self.canvas.isVisible():
            return
        self.engine.animate(self.canvas.grab(), mode)

    def toggle_zoom(self, audience_point):
        if not self.zoom.zoomed:
            self.focus.hide()
        self.zoom.toggle(self.from_audience(audience_point) if audience_point is not None else None)
        self.sync_focus()

    def eventFilter(self, obj, e):
        if obj is self.canvas:
            t = e.type()
            if t == QEvent.Type.MouseMove:
                q = self.to_audience(e.position())
                if q is not None:
                    self.audience.aim(q)
            elif (
                t == QEvent.Type.MouseButtonPress
                and e.button() == Qt.MouseButton.LeftButton
                and self.audience.focus_overlay.mode
            ):
                q = self.to_audience(e.position())
                if q is not None:
                    self.audience.focus_overlay.pin_or_move(q)
                return True
            elif t == QEvent.Type.Resize:
                self.sync_focus()
        return False


class Audience(QMainWindow):
    keyForward = Signal(int, int, str)
    closeRequested = Signal()

    def __init__(self, data, start=0, online=False):
        super().__init__()
        self.data = data
        self.i = start
        self.online = online
        self.view = SlideCanvas()
        self.blank = None
        self.transition_engine = TransitionEngine(self.view)
        self.setCentralWidget(self.view)
        self.setFocusPolicy(Qt.StrongFocus)
        self._video_widget = None
        self._video_player = None
        self._video_audio = None
        self._video_source_key = ""
        self._motion_clock = QElapsedTimer()
        self._motion_timer = QTimer(self)
        self._motion_timer.setInterval(40)
        self._motion_timer.timeout.connect(self.draw)
        self._motion_clock.start()
        # 2.2 (U3): the 25 fps redraw runs only while a KEN BURNS background is
        # actually moving and visible; it used to repaint every slide forever.
        self._curtain_on = False
        # 2.3: the important words come first, because screen-share pickers
        # (Teams in Chromium/Helium) truncate long window titles.
        self.setWindowTitle(AUDIENCE_TITLE)
        self._share_flash = None
        self._opening = False
        self._opening_token = 0
        self.mirror = None  # PreviewMirror of the presenter view, if any
        self._curtain = None
        self.zoom = LiveZoom(self.view)
        self._paused = False
        self._pause_video_was_playing = False
        self._pause_web_audio_muted = None
        self.focus_overlay = PresenterFocusOverlay(self.view)
        self.pause_overlay = PresenterPauseOverlay(self.view)
        self.view.setMouseTracking(True)
        self.view.installEventFilter(self)
        self._cursor_timer = QTimer(self)
        self._cursor_timer.setSingleShot(True)
        self._cursor_timer.setInterval(1500)
        self._cursor_timer.timeout.connect(lambda: self.setCursor(Qt.BlankCursor))
        if not online:
            self._cursor_timer.start()
        self.draw()
        self._update_motion_timer()

    def draw(self):
        if self.blank is not None:
            return
        slide = self.data["slides"][self.i]
        render_slide_data = slide
        if (
            slide.get("backgroundImage")
            and slide.get("backgroundMotion") == "KEN BURNS"
            and not self.data.get("reduceMotion", False)
        ):
            motion_key = str(slide.get("id") or self.i)
            if getattr(self, "_motion_slide_key", None) != motion_key:
                self._motion_slide_key = motion_key
                self._motion_clock.restart()
            render_slide_data = clone(slide)
            phase = min(1.0, self._motion_clock.elapsed() / KEN_BURNS_MS)
            if phase >= 1.0 and self._motion_timer.isActive():
                self._motion_timer.stop()
            ease = phase * phase * (3.0 - 2.0 * phase)
            direction = -1.0 if (sum(ord(c) for c in motion_key) % 2) else 1.0
            drift = (ease - 0.5) * 0.12 * direction
            render_slide_data["backgroundFocalX"] = max(
                0.0, min(1.0, float(slide.get("backgroundFocalX", 0.5)) + drift)
            )
            render_slide_data["backgroundFocalY"] = max(
                0.0, min(1.0, float(slide.get("backgroundFocalY", 0.5)) - drift * 0.35)
            )
            render_slide_data["_backgroundZoom"] = 1.0 + 0.085 * ease
        self.view.render(
            render_slide_data,
            self.data.get("theme", "B&W"),
            self.i,
            len(self.data["slides"]),
            self.data.get("showNumbers", False),
            self.data.get("fontFamily", "Noto Sans"),
            self.data.get("aspect", "16:9"),
            self.data.get("showLogo", True),
            self.data.get("footerText", ""),
            self.data.get("footerAlign", "LEFT"),
            self.data.get("footerSize", "MEDIUM"),
        )
        if not self._opening:
            self._sync_video(slide)
        self.focus_overlay.setGeometry(self.view.rect())
        self.focus_overlay.raise_()
        self.pause_overlay.setGeometry(self.view.rect())
        if self._paused:
            self.pause_overlay.raise_()

    def _motion_wanted(self):
        if self.blank is not None or self._paused or self._curtain_on:
            return False
        if self.data.get("reduceMotion", False):
            return False
        slide = self.data["slides"][self.i]
        return (
            bool(slide.get("backgroundImage"))
            and slide.get("backgroundMotion") == "KEN BURNS"
            and self._motion_clock.elapsed() < KEN_BURNS_MS
        )

    def _update_motion_timer(self):
        if self._motion_wanted():
            if not self._motion_timer.isActive():
                self._motion_timer.start()
        elif self._motion_timer.isActive():
            self._motion_timer.stop()

    def _clear_video(self):
        if self._video_player is not None:
            try:
                self._video_player.stop()
            except Exception:
                pass
        if self._video_widget is not None:
            try:
                self._video_widget.hide()
                self._video_widget.deleteLater()
            except Exception:
                pass
        self._video_widget = None
        self._video_player = None
        self._video_audio = None
        self._video_source_key = ""

    def _video_rect(self, slide):
        r = _aspect_rect(self.view.rect(), self.data.get("aspect", "16:9"))
        m = r.width() * 0.055
        vr = (
            QRectF(r.x() + m, r.y() + r.height() * 0.23, r.width() - 2 * m, r.height() * 0.58)
            if slide.get("layout") == "VIDEO"
            else QRectF(r.x() + m, r.y() + r.height() * 0.25, r.width() * 0.56, r.height() * 0.52)
        )
        return vr.toRect()

    def _sync_video(self, slide):
        source = str(slide.get("videoSource", "") or "").strip()
        is_video = slide.get("layout") in ("VIDEO", "VIDEO + TEXT") and bool(source)
        if not is_video:
            if self._video_widget is not None:
                self._clear_video()
            return
        key = f"{self.i}|{source}"
        if key != self._video_source_key:
            self._clear_video()
            self._video_source_key = key
            try:
                if re.match(r"^https?://", source, re.I):
                    from PySide6.QtWebEngineWidgets import QWebEngineView

                    w = QWebEngineView(self.view)
                    w.setUrl(QUrl(_web_video_url(source)))
                    self._video_widget = w
                else:
                    from PySide6.QtMultimedia import QMediaPlayer, QAudioOutput
                    from PySide6.QtMultimediaWidgets import QVideoWidget

                    w = QVideoWidget(self.view)
                    player = QMediaPlayer(self)
                    audio = QAudioOutput(self)
                    audio.setMuted(bool(slide.get("videoMuted", False)))
                    player.setAudioOutput(audio)
                    player.setVideoOutput(w)
                    player.setSource(QUrl.fromLocalFile(source))
                    self._video_widget = w
                    self._video_player = player
                    self._video_audio = audio
                    if slide.get("videoAutoplay", False):
                        player.play()
                self._video_widget.show()
                self._video_widget.raise_()
            except Exception as exc:
                print(f"NIRUPRES video warning: {exc}", file=sys.stderr)
                self._clear_video()
                return
        if self._video_widget is not None:
            self._video_widget.setGeometry(self._video_rect(slide))
            self._video_widget.show()
            self._video_widget.raise_()

    def toggle_video(self):
        if self._video_player is None:
            return False
        try:
            from PySide6.QtMultimedia import QMediaPlayer

            if self._video_player.playbackState() == QMediaPlayer.PlayingState:
                self._video_player.pause()
            else:
                self._video_player.play()
            return True
        except Exception:
            return False

    def _transition_for(self, i):
        slide = self.data["slides"][i]
        return (
            "NONE"
            if self.data.get("reduceMotion", False)
            else normalize_transition(slide.get("transition", "DECK"), True).replace(
                "DECK", normalize_transition(self.data.get("transition", "NONE"))
            )
        )

    def set_index(self, i):
        new_i = max(0, min(len(self.data["slides"]) - 1, i))
        if new_i == self.i:
            return
        mode = self._transition_for(new_i)
        old_pm = (
            self.view.grab()
            if self.blank is None and self.view.isVisible() and mode != "NONE"
            else None
        )
        if self._opening:
            # Navigating during the opening fade ends it at once.
            self._cancel_opening()
        self.clear_presenter_effects()
        self.i = new_i
        self._motion_clock.restart()
        self.draw()
        self._update_motion_timer()
        if old_pm is None or old_pm.isNull() or mode == "NONE":
            return
        self.transition_engine.animate(old_pm, mode)
        if self.mirror is not None:
            self.mirror.transition(mode)

    def set_blank(self, mode=None):
        self.blank = mode
        if mode == "B":
            self.setStyleSheet("background:#000")
            self.view.hide()
        elif mode == "W":
            self.setStyleSheet("background:#fff")
            self.view.hide()
        else:
            self.setStyleSheet("")
            self.view.show()
            self.draw()
        self._update_motion_timer()

    def _engines(self):
        engines = [self.transition_engine]
        if self.mirror is not None:
            engines.append(self.mirror.engine)
        return engines

    def begin_opening(self):
        """Fade in from black once the window is really on screen.

        The black cover is placed at once, before the first frame. The fade
        itself starts only when the window is exposed plus a short settle time
        (window manager fullscreen/maximise and compositor map fade), so it is
        never spent before anyone can see it. A timeout keeps it from waiting
        forever.
        """
        self._opening = True
        self._opening_token += 1
        token = self._opening_token
        self._motion_clock.restart()
        self.draw()
        self._update_motion_timer()
        for engine in self._engines():
            engine.hold_black()
        waited = QElapsedTimer()
        waited.start()

        def poll():
            if token != self._opening_token or not self._opening:
                return
            handle = self.windowHandle()
            exposed = handle is not None and handle.isExposed() and self.isVisible()
            if exposed or waited.elapsed() >= OPEN_WAIT_MAX_MS:
                QTimer.singleShot(OPEN_SETTLE_MS, lambda: self._start_opening_fade(token))
            else:
                QTimer.singleShot(25, poll)

        poll()

    def _start_opening_fade(self, token):
        if token != self._opening_token or not self._opening:
            return
        self.transition_engine.opening(lambda: self.finish_opening(token))
        if self.mirror is not None:
            self.mirror.engine.opening()

    def finish_opening(self, token=None):
        if token is not None and token != self._opening_token:
            return
        self._opening = False
        self.draw()

    def _cancel_opening(self):
        self._opening_token += 1
        self._opening = False
        for engine in self._engines():
            engine.stop()

    def begin_closing(self, black_reached):
        """Fade to black (also in the presenter preview); calls black_reached."""
        if self._opening:
            # Keep the cover: the closing fade continues from where it is.
            self._opening_token += 1
            self._opening = False
        self.transition_engine.closing(black_reached)
        if self.mirror is not None:
            self.mirror.engine.closing()

    def aim(self, p):
        """Point Spotlight, Focus Reveal and Zoom at p (audience view coordinates)."""
        self.focus_overlay.set_point(p)

    @property
    def _zoomed(self):
        return self.zoom.zoomed

    @property
    def _zoom_overlay(self):
        return self.zoom.overlay

    def set_focus_mode(self, mode):
        self.focus_overlay.set_mode(None if self.focus_overlay.mode == mode else mode)
        if self.focus_overlay.mode and self.focus_overlay.point.isNull():
            self.focus_overlay.set_point(self.view.rect().center())
        self._cursor_activity()

    def adjust_focus_radius(self, delta):
        self.focus_overlay.adjust_radius(delta)

    def toggle_zoom(self):
        pt = self.focus_overlay.point if not self.focus_overlay.point.isNull() else None
        # Zoom magnifies the clean slide; the Spotlight/Focus veil rests while
        # zoomed (its hole would no longer match the magnified slide).
        if not self.zoom.zoomed:
            self.focus_overlay.hide()
        self.zoom.toggle(pt)
        if not self.zoom.zoomed:
            self.focus_overlay.setVisible(bool(self.focus_overlay.mode))
        self.focus_overlay.raise_()
        if self.mirror is not None:
            self.mirror.toggle_zoom(pt)

    def clear_presenter_effects(self):
        self.zoom.clear()
        if self.mirror is not None:
            self.mirror.zoom.clear()
        self.focus_overlay.setVisible(bool(self.focus_overlay.mode))
        self.focus_overlay.unpin()

    def _cursor_activity(self):
        if self.online:
            return
        self.setCursor(Qt.ArrowCursor)
        self._cursor_timer.start()

    def set_curtain(self, on):
        if on:
            if self._curtain is None:
                self._curtain = QLabel(self.view)
                self._curtain.setStyleSheet("background:#000;border:0")
            self._curtain.setGeometry(self.view.rect())
            self._curtain.show()
            self._curtain.raise_()
        else:
            if self._curtain is not None:
                self._curtain.hide()
        self._curtain_on = bool(on)
        self._update_motion_timer()
        self.focus_overlay.raise_()

    def set_pause(self, on):
        on = bool(on)
        if on == self._paused:
            return
        self._paused = on
        if on:
            self._update_motion_timer()
            # Local video is genuinely paused and resumes only if it was playing.
            self._pause_video_was_playing = False
            if self._video_player is not None:
                try:
                    from PySide6.QtMultimedia import QMediaPlayer

                    self._pause_video_was_playing = (
                        self._video_player.playbackState() == QMediaPlayer.PlayingState
                    )
                    if self._pause_video_was_playing:
                        self._video_player.pause()
                except Exception:
                    pass
            # Web embeds cannot be cross-origin paused reliably; silence them while the pause veil is active.
            if self._video_widget is not None and hasattr(self._video_widget, "page"):
                try:
                    page = self._video_widget.page()
                    self._pause_web_audio_muted = page.isAudioMuted()
                    page.setAudioMuted(True)
                except Exception:
                    self._pause_web_audio_muted = None
            self.pause_overlay.setGeometry(self.view.rect())
            self.pause_overlay.start_pause()
        else:
            self.pause_overlay.stop_pause()
            if self._video_player is not None and self._pause_video_was_playing:
                try:
                    self._video_player.play()
                except Exception:
                    pass
            if (
                self._video_widget is not None
                and hasattr(self._video_widget, "page")
                and self._pause_web_audio_muted is not None
            ):
                try:
                    self._video_widget.page().setAudioMuted(bool(self._pause_web_audio_muted))
                except Exception:
                    pass
            self._pause_video_was_playing = False
            self._pause_web_audio_muted = None
            self._update_motion_timer()
            self.focus_overlay.raise_()

    def flash_share(self, ms=SHARE_FLASH_MS):
        if self._share_flash is None:
            self._share_flash = ShareFlashOverlay(self.view)
        self._share_flash.flash(ms)

    def eventFilter(self, obj, event):
        if obj is self.view and event.type() == QEvent.Type.MouseMove:
            self.focus_overlay.set_point(event.position())
            self._cursor_activity()
        if (
            obj is self.view
            and event.type() == QEvent.Type.MouseButtonPress
            and self.focus_overlay.mode
        ):
            self.focus_overlay.pin_or_move(event.position())
            self._cursor_activity()
            return True
        return super().eventFilter(obj, event)

    def resizeEvent(self, e):
        super().resizeEvent(e)
        self.focus_overlay.setGeometry(self.view.rect())
        if self._curtain is not None:
            self._curtain.setGeometry(self.view.rect())
        self.pause_overlay.setGeometry(self.view.rect())
        if self._paused:
            self.pause_overlay.raise_()
        if self._share_flash is not None and self._share_flash.isVisible():
            self._share_flash.setGeometry(self.view.rect())
            self._share_flash.raise_()
        # Video geometry followed the old 25 fps redraw; keep it on resize.
        if self._video_widget is not None and 0 <= self.i < len(self.data["slides"]):
            self._video_widget.setGeometry(self._video_rect(self.data["slides"][self.i]))

    def keyPressEvent(self, e):
        self.keyForward.emit(int(e.key()), _mod_int(e.modifiers()), e.text())
        e.accept()

    def mousePressEvent(self, e):
        self.keyForward.emit(
            int(Qt.Key_Left if e.position().x() < self.width() / 2 else Qt.Key_Right), 0, ""
        )
        e.accept()

    def closeEvent(self, e):
        self._clear_video()
        if self.isVisible():
            self.closeRequested.emit()
        e.accept()


class DisplaySetup(QDialog):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowTitle("NIRUPRES — Display Setup")
        self.setMinimumWidth(560)
        screens = QApplication.screens()
        self.screens = screens
        v = QVBoxLayout(self)
        v.addWidget(QLabel("DISPLAY SETUP"))
        info = QLabel("Choose where Presenter View and the audience presentation should appear.")
        info.setWordWrap(True)
        v.addWidget(info)
        f = QFormLayout()
        self.presenter = QComboBox()
        self.audience = QComboBox()
        for i, sc in enumerate(screens):
            g = sc.geometry()
            label = f"{i+1}. {sc.name()} — {g.width()}×{g.height()} @ {g.x()},{g.y()}" + (
                " — PRIMARY" if sc == QApplication.primaryScreen() else ""
            )
            self.presenter.addItem(label, i)
            self.audience.addItem(label, i)
        prefs = {}
        try:
            prefs = (
                json.loads(DISPLAY_PREFS.read_text(encoding="utf-8"))
                if DISPLAY_PREFS.exists()
                else {}
            )
        except:
            pass

        def find(name, default):
            for i, sc in enumerate(screens):
                if sc.name() == name:
                    return i
            return default

        pi = find(
            prefs.get("presenter", ""),
            (
                screens.index(QApplication.primaryScreen())
                if QApplication.primaryScreen() in screens
                else 0
            ),
        )
        ai = find(prefs.get("audience", ""), next((i for i in range(len(screens)) if i != pi), pi))
        self.presenter.setCurrentIndex(pi)
        self.audience.setCurrentIndex(ai)
        f.addRow("Presenter display", self.presenter)
        f.addRow("Audience display", self.audience)
        v.addLayout(f)
        self.warn = QLabel()
        self.warn.setWordWrap(True)
        v.addWidget(self.warn)
        self.presenter.currentIndexChanged.connect(self.check)
        self.audience.currentIndexChanged.connect(self.check)
        self.check()
        swap = QPushButton("⇄  SWAP DISPLAYS")
        swap.setToolTip("Swap Presenter and Audience displays")
        swap.clicked.connect(self.swap_displays)
        v.addWidget(swap)
        bb = neutralize_button_box(QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel))
        bb.accepted.connect(self.accept)
        bb.rejected.connect(self.reject)
        v.addWidget(bb)

    def check(self):
        same = self.presenter.currentData() == self.audience.currentData()
        self.warn.setText(
            "Same display: laptop presenter mode. Presenter View fills this screen and you share the window “AUDIENCE · share this · NIRUPRES” in Teams, Meet or Zoom."
            if same
            else "Only the slide is shown on Audience. Controls, notes, timer and next-slide preview stay on Presenter."
        )

    def swap_displays(self):
        pi = self.presenter.currentIndex()
        ai = self.audience.currentIndex()
        self.presenter.setCurrentIndex(ai)
        self.audience.setCurrentIndex(pi)
        self.check()

    def selection(self):
        pi = int(self.presenter.currentData())
        ai = int(self.audience.currentData())
        return self.screens[pi], self.screens[ai]


PRESENTER_PREFS = DATA / "presenter.json"
PRIVATE_RED = "#C4161C"
PRESENTER_QSS = """
QMainWindow#presenterWindow{background:#080808}
QWidget#presenterHost{background:#080808}
QWidget#presenterHost QLabel{color:#a8a8a8;background:transparent}
QWidget#presenterHost QLabel#privateBar{background:__RED__;color:#ffffff;font-weight:900;letter-spacing:2px;padding:0 16px}
QWidget#presenterHost QLabel#presenterMode{color:#e8e8e8;font-weight:900;letter-spacing:1px}
QWidget#presenterHost QLabel#presenterState{color:#d0d0d0;font-weight:800}
QWidget#presenterHost QLabel#presenterState[warn="true"]{color:#ffb020}
QWidget#presenterHost QLabel#presenterLabel{font-family:"Noto Sans Mono",monospace;font-size:11px;font-weight:800;color:#7a7a7a;letter-spacing:1px}
QWidget#presenterHost QLabel#presenterTimer{color:#f4f2ed;font-family:"Noto Sans Mono",monospace;font-weight:800}
QWidget#presenterHost QLabel#presenterClock{color:#a8a8a8;font-family:"Noto Sans Mono",monospace;font-weight:600}
QWidget#presenterHost QLabel#presenterPace{color:#a8a8a8;font-family:"Noto Sans Mono",monospace}
QWidget#presenterHost QLabel#presenterPosition{color:#e8e8e8;font-family:"Noto Sans Mono",monospace;font-weight:800}
QWidget#presenterHost QLabel#nextTitle{color:#d8d8d8;font-weight:700}
QTextEdit#presenterNotes{background:#0e0e0e;color:#ececec;border:1px solid #262626;padding:10px;selection-background-color:#eee;selection-color:#090909}
QPushButton{background:#121212;color:#e8e8e8;border:1px solid #303030;padding:7px 14px;font-weight:700;min-height:20px}
QPushButton:hover{background:#202020;border-color:#555}
QPushButton:focus{border:1px solid #9a9a9a}
QPushButton#endPresentation{border-color:#5a2a2a}
QPushButton#endPresentation:hover{background:#2a1212;border-color:#a33}
QPushButton#notesSize{padding:4px 10px;min-width:34px}
""".replace("__RED__", PRIVATE_RED)


def _load_presenter_prefs():
    try:
        data = json.loads(PRESENTER_PREFS.read_text(encoding="utf-8"))
        return data if isinstance(data, dict) else {}
    except (OSError, ValueError):
        return {}


def _save_presenter_prefs(prefs):
    try:
        PRESENTER_PREFS.parent.mkdir(parents=True, exist_ok=True)
        PRESENTER_PREFS.write_text(json.dumps(prefs), encoding="utf-8")
    except OSError:
        pass


def _px(widget, px, weight=None):
    f = QFont(widget.font())
    f.setPixelSize(max(6, int(px)))
    if weight is not None:
        f.setWeight(weight)
    widget.setFont(f)


class PresenterBody(QWidget):
    """Presenter View body with a layout that adapts to the window size.

    Current slide, next slide (collapses to a title line when there is no
    room), a big timer and clock, and a notes pane that always gets at least
    a third of the window height.
    """

    def __init__(self, presenter):
        super().__init__()
        self.p = presenter
        self.now = SlideCanvas()
        self.next = SlideCanvas()
        self.now.setMinimumSize(1, 1)
        self.next.setMinimumSize(1, 1)
        self.now_label = QLabel("CURRENT")
        self.next_label = QLabel("NEXT")
        for lab in (self.now_label, self.next_label):
            lab.setObjectName("presenterLabel")
        self.next_title = QLabel()
        self.next_title.setObjectName("nextTitle")
        self.next_title.setWordWrap(True)
        self.timer = QLabel("00:00")
        self.timer.setObjectName("presenterTimer")
        self.timer.setAlignment(Qt.AlignLeft | Qt.AlignVCenter)
        self.pace = QLabel("")
        self.pace.setObjectName("presenterPace")
        self.clock = QLabel("")
        self.clock.setObjectName("presenterClock")
        self.position = QLabel("")
        self.position.setObjectName("presenterPosition")
        self.notes_label = QLabel("SPEAKER NOTES")
        self.notes_label.setObjectName("presenterLabel")
        self.smaller = QPushButton("A−")
        self.larger = QPushButton("A+")
        for b in (self.smaller, self.larger):
            b.setObjectName("notesSize")
            b.setFocusPolicy(Qt.NoFocus)
        self.smaller.setToolTip("Smaller notes · Ctrl+−")
        self.larger.setToolTip("Larger notes · Ctrl++")
        self.notes = QTextEdit()
        self.notes.setObjectName("presenterNotes")
        self.notes.setReadOnly(True)
        self.notes.setFocusPolicy(Qt.NoFocus)
        for w in (self.now, self.next, self.now_label, self.next_label, self.next_title,
                  self.timer, self.pace, self.clock, self.position, self.notes_label,
                  self.smaller, self.larger, self.notes):
            w.setParent(self)
        self.compact_next = False
        self.at_end = False  # last slide: NEXT shows a title line, not a slide

    def resizeEvent(self, e):
        super().resizeEvent(e)
        self.relayout()

    def relayout(self):
        W, H = self.width(), self.height()
        if W < 10 or H < 10:
            return
        gap = max(10, int(min(W, H) * 0.016))
        total_h = max(H, self.window().height())
        notes_h = max(int(total_h / 3), 140)
        notes_h = min(notes_h, H - 160)
        top_h = H - notes_h - gap
        label_h = 18
        # Current slide: as large as fits beside a right column of >= 30 %.
        min_right = max(240, int(W * 0.30))
        cw = min(W - min_right - gap, int((top_h - label_h) * 16 / 9))
        cw = max(160, cw)
        ch = int(cw * 9 / 16)
        self.now_label.setGeometry(0, 0, cw, label_h)
        self.now.setGeometry(0, label_h, cw, ch)
        rx = cw + gap
        rw = W - rx
        timer_px = max(30, min(120, int(top_h * 0.17), int(rw * 0.16)))
        _px(self.timer, timer_px)
        _px(self.clock, max(16, int(timer_px * 0.42)))
        _px(self.pace, max(13, int(timer_px * 0.24)))
        _px(self.position, max(16, int(timer_px * 0.40)))
        y = 0
        self.timer.setGeometry(rx, y, rw, int(timer_px * 1.25))
        y += int(timer_px * 1.25)
        pace_h = int(self.pace.font().pixelSize() * 1.5) if self.pace.text() else 0
        self.pace.setGeometry(rx, y, rw, pace_h)
        self.pace.setVisible(bool(pace_h))
        y += pace_h
        row_h = int(self.clock.font().pixelSize() * 1.5)
        self.clock.setGeometry(rx, y, rw // 2, row_h)
        self.position.setGeometry(rx + rw // 2, y, rw - rw // 2, row_h)
        self.position.setAlignment(Qt.AlignRight | Qt.AlignVCenter)
        y += row_h + gap
        avail = top_h - y - label_h
        nw = min(rw, int(avail * 16 / 9))
        nh = int(nw * 9 / 16)
        self.compact_next = nw < 200 or nh < 100 or self.at_end
        self.next_label.setGeometry(rx, y, rw, label_h)
        if self.compact_next:
            self.next.hide()
            self.next_title.show()
            _px(self.next_title, max(14, int(timer_px * 0.30)))
            self.next_title.setGeometry(rx, y + label_h, rw, max(30, avail))
            self.next_title.setAlignment(Qt.AlignLeft | Qt.AlignTop)
        else:
            self.next_title.hide()
            self.next.show()
            self.next.setGeometry(rx, y + label_h, nw, nh)
        ny = top_h + gap
        bh = 30
        self.notes_label.setGeometry(0, ny, W - 120, bh)
        self.smaller.setGeometry(W - 104, ny, 48, bh - 2)
        self.larger.setGeometry(W - 52, ny, 52, bh - 2)
        self.notes.setGeometry(0, ny + bh + 4, W, max(40, H - ny - bh - 4))


class ShareCheckDialog(QDialog):
    """Share check before an online presentation starts."""

    def __init__(self, presenter, audience):
        super().__init__(presenter)
        self.audience = audience
        self.setObjectName("shareCheck")
        self.setWindowTitle("Share check · NIRUPRES")
        self.setModal(True)
        self.setMinimumWidth(560)
        self.setStyleSheet(
            "QDialog#shareCheck{background:#0b0b0b;border:2px solid #19C37D}"
            "QLabel{color:#d8d8d8;font-size:14px} QLabel#shareHead{color:#fff;font-size:20px;"
            "font-weight:900;letter-spacing:1px} QLabel#sharePick{background:#19C37D;color:#04130C;"
            "font-size:17px;font-weight:900;padding:8px 12px} QLabel#shareWarn{color:#ffb020;"
            "font-weight:800} QLabel#shareNever{color:#ff8a80;font-weight:700}"
            "QPushButton{background:#151515;color:#eee;border:1px solid #3a3a3a;padding:8px 16px;"
            "font-weight:800} QPushButton:default{background:#e8e8e8;color:#090909}"
        )
        v = QVBoxLayout(self)
        v.setSpacing(10)
        head = QLabel("SHARE CHECK")
        head.setObjectName("shareHead")
        v.addWidget(head)
        steps = QLabel(
            "1.  In Teams, click <b>Share</b>.<br>"
            "2.  Open the <b>Window</b> tab (not Entire screen).<br>"
            "3.  Pick this entry:"
        )
        steps.setTextFormat(Qt.RichText)
        v.addWidget(steps)
        pick = QLabel(AUDIENCE_TITLE)
        pick.setObjectName("sharePick")
        v.addWidget(pick)
        never = QLabel(f"Never pick “{PRESENTER_TITLE}” (red bar) or Entire screen.")
        hop = QLabel(
            "You can switch workspaces; the audience keeps seeing the slide. "
            "Come back here and the arrow keys work at once."
        )
        hop.setWordWrap(True)
        never.setObjectName("shareNever")
        never.setWordWrap(True)
        v.addWidget(never)
        v.addWidget(hop)
        self.status = QLabel()
        self.status.setWordWrap(True)
        v.addWidget(self.status)
        row = QHBoxLayout()
        self.flash_button = QPushButton("Flash audience window")
        self.flash_button.setAutoDefault(False)
        self.flash_button.clicked.connect(self.flash)
        cancel = QPushButton("Cancel")
        cancel.setAutoDefault(False)
        cancel.clicked.connect(self.reject)
        start = QPushButton("Start presenting")
        start.setDefault(True)
        start.clicked.connect(self.accept)
        row.addWidget(self.flash_button)
        row.addStretch()
        row.addWidget(cancel)
        row.addWidget(start)
        v.addLayout(row)
        self._poll = QTimer(self)
        self._poll.setInterval(1000)
        self._poll.timeout.connect(self.refresh)
        self._poll.start()
        QTimer.singleShot(250, self.flash)
        self.refresh()

    def flash(self):
        self.audience.flash_share(SHARE_FLASH_MS)
        self.refresh(flashing=True)

    def refresh(self, flashing=False):
        viewable = x11props.is_viewable(self.audience)
        if viewable is False or not self.audience.isVisible():
            self.status.setObjectName("shareWarn")
            self.status.setText(
                "⚠  The audience window is hidden (another workspace or minimised). "
                "A shared hidden window freezes. Bring it back before you share."
            )
        else:
            self.status.setObjectName("")
            self.status.setText(
                "The audience window shows a green border and a SHARED WINDOW badge for "
                "5 seconds. Check that the Teams preview shows it."
                if flashing
                else "Press Flash to mark the audience window again."
            )
        self.status.style().unpolish(self.status)
        self.status.style().polish(self.status)

    def done(self, r):
        self._poll.stop()
        super().done(r)


class Presenter(QMainWindow):
    finished = Signal()
    slideChanged = Signal(int)

    def __init__(
        self,
        data,
        start=0,
        presenter_screen=None,
        audience_screen=None,
        online=False,
        talk=False,
        style=None,
        share_check=False,
    ):
        super().__init__()
        self.data = data
        self.online = online
        self.talk = talk
        self.talk_stop = None
        self._closing = False
        self._black_hold = False
        self._closing_fade_started = False
        self._end_black = False  # faded to black after the last slide
        self._share_checking = False
        self.share_dialog = None
        self.curtain = False
        self.paused = False
        self._paused_total_ms = 0
        self._pause_started_ms = None
        self.jump = ""
        self.blank = None
        self.clock = QElapsedTimer()
        self.clock.start()
        self.setFocusPolicy(Qt.StrongFocus)
        self.setObjectName("presenterWindow")
        self.presenting_mode = PresentingMode()
        screens = QApplication.screens()
        primary = QApplication.primaryScreen()
        presenter_screen = presenter_screen or primary
        audience_screen = audience_screen or next(
            (x for x in screens if x != presenter_screen), presenter_screen
        )
        self.presenter_screen = presenter_screen
        self.audience_screen = audience_screen
        self.i = self._nearest_visible(start, 1)
        QApplication.instance().screenRemoved.connect(self._screen_removed)
        self.audience = Audience(data, self.i, online=online)
        self.audience.keyForward.connect(self.handle_key)
        self.audience.closeRequested.connect(self.close_session)
        # Create native windows while still hidden, bind each QWindow to its target QScreen,
        # then enter fullscreen. Mapping first and moving later is unreliable under Wayland/Hyprland.
        self.audience.create()
        # Own WM_CLASS per window and _NIRUWM_KEEP_MAPPED: NIRUWM keeps the
        # audience window mapped (live in a share) when its workspace is hidden.
        x11props.set_window_identity(
            self.audience, "nirupres-audience", "NIRUPRES", True, opaque=True
        )
        if self.audience.windowHandle():
            self.audience.windowHandle().setScreen(audience_screen)
        if online:
            # Online/laptop mode: a normal 16:9 top-level Audience window that
            # Teams/Meet/Zoom share as a window. The app never resizes it away
            # from 16:9; the presenter view covers it on the same screen.
            ag = audience_screen.availableGeometry()
            aw = max(640, min(1280, int(ag.width() * 0.86)))
            ah = int(aw * 9 / 16)
            if ah > ag.height() * 0.9:
                ah = max(360, int(ag.height() * 0.9))
                aw = int(ah * 16 / 9)
            ax = ag.x() + (ag.width() - aw) // 2
            ay = ag.y() + (ag.height() - ah) // 2
            self.audience.setGeometry(ax, ay, aw, ah)
            self.audience.setMinimumSize(640, 360)
            self.audience.show()
        else:
            self.audience.setGeometry(audience_screen.geometry())
            self.audience.showFullScreen()
        if talk:
            stop = QPushButton("■  STOP TALK", self.audience.view)
            stop.setObjectName("talkStop")
            stop.setStyleSheet(
                "QPushButton#talkStop{background:rgba(8,8,8,220);color:#f4f2ed;border:1px solid #777;padding:8px 14px;font-weight:800;letter-spacing:1px} QPushButton#talkStop:hover{background:#282828;border-color:#fff}"
            )
            stop.setFixedSize(172, 38)
            stop.clicked.connect(self.close_session)
            stop.raise_()
            stop.show()
            self.talk_stop = stop

            def place_stop():
                if self.talk_stop is not None and self.audience.isVisible():
                    self.talk_stop.move(
                        max(16, self.audience.width() - self.talk_stop.width() - 20),
                        max(16, self.audience.height() - self.talk_stop.height() - 20),
                    )
                    self.talk_stop.raise_()

            QTimer.singleShot(220, place_stop)
            QTimer.singleShot(900, place_stop)
        self.dual = online or (len(screens) > 1 and presenter_screen != audience_screen)
        self.mirror = None
        open_now = (
            self.data.get("cinematicOpen", True)
            and not self.data.get("reduceMotion", False)
            and not share_check
        )
        self.presenting_mode.start(os.getpid())
        if not self.dual:
            if open_now:
                self.audience.begin_opening()
            # Single-display mode has no visible Presenter window. The Audience must
            # therefore be the explicit keyboard owner after Wayland has mapped the
            # fullscreen surface. Installing shortcuts on a hidden Presenter caused
            # navigation to be lost on Hyprland/Qt 6.11.
            self.hide()
            QApplication.instance().installEventFilter(self)
            self._install_shortcuts(self.audience)
            QTimer.singleShot(0, self.claim_audience_focus)
            QTimer.singleShot(180, self.claim_audience_focus)
            return
        self.setWindowTitle(PRESENTER_TITLE)
        self.setStyleSheet((style or "") + PRESENTER_QSS)
        self.resize(1180, 760)
        host = QWidget()
        host.setObjectName("presenterHost")
        outer = QVBoxLayout(host)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)
        self.setCentralWidget(host)
        # PRIVATE bar: large and red so it is obvious even in a share-picker thumbnail.
        self.private_bar = QLabel("PRIVATE  ·  NOT FOR SHARING  ·  PRESENTER VIEW")
        self.private_bar.setObjectName("privateBar")
        self.private_bar.setAlignment(Qt.AlignCenter)
        outer.addWidget(self.private_bar)
        inner = QWidget()
        root = QVBoxLayout(inner)
        root.setContentsMargins(16, 10, 16, 12)
        root.setSpacing(10)
        outer.addWidget(inner, 1)
        top = QHBoxLayout()
        mode = QLabel("PRESENTER · ONLINE" if online else "PRESENTER · DOCKED")
        mode.setObjectName("presenterMode")
        self.audience_state = QLabel()
        self.audience_state.setObjectName("presenterState")
        top.addWidget(mode)
        top.addSpacing(24)
        top.addWidget(self.audience_state, 1)
        root.addLayout(top)
        self.body = PresenterBody(self)
        self.now = self.body.now
        self.next = self.body.next
        self.timer = self.body.timer
        self.position_label = self.body.position
        self.notes = self.body.notes
        self.notes_label = self.body.notes_label
        # 2.4: the CURRENT preview mirrors fades, transitions and focus effects,
        # and the mouse over it aims Spotlight/Focus Reveal/Zoom on the audience.
        self.mirror = PreviewMirror(self.now, self.audience)
        self.now.setToolTip("Point here to aim Spotlight (S), Focus Reveal (F) and Zoom (Z)")
        root.addWidget(self.body, 1)
        controls = QHBoxLayout()
        prev = QPushButton("‹  PREVIOUS")
        end = QPushButton("✕  END PRESENTATION")
        nxt = QPushButton("NEXT  ›")
        for b in (prev, end, nxt):
            b.setFocusPolicy(Qt.NoFocus)
        prev.clicked.connect(lambda: self.navigate(-1))
        nxt.clicked.connect(lambda: self.navigate(1))
        end.clicked.connect(self.close_session)
        end.setObjectName("endPresentation")
        end.setToolTip("End presentation and return to the editor · Esc")
        controls.addWidget(prev)
        controls.addStretch()
        controls.addWidget(end)
        controls.addStretch()
        controls.addWidget(nxt)
        root.addLayout(controls)
        prefs = _load_presenter_prefs()
        # Default notes size follows the presenter screen; A-/A+ choice is remembered.
        default_px = max(20, min(40, round(presenter_screen.geometry().height() * 0.028)))
        self.notes_px = max(12, min(64, int(prefs.get("notesPx", default_px) or default_px)))
        self._apply_notes_size()
        self.body.smaller.clicked.connect(lambda: self.notes_zoom(-2))
        self.body.larger.clicked.connect(lambda: self.notes_zoom(2))
        self.tick = QTimer(self)
        self.tick.timeout.connect(self.update_clock)
        self.tick.start(1000)
        self.share_watch = QTimer(self)
        self.share_watch.setInterval(2000)
        self.share_watch.timeout.connect(self.refresh_audience_state)
        self.share_watch.start()
        self.create()
        x11props.set_window_identity(self, "nirupres-presenter", "NIRUPRES", opaque=True)
        if self.windowHandle():
            self.windowHandle().setScreen(presenter_screen)
        if online:
            # Laptop/online mode: the presenter view fills the screen and covers
            # the audience window, which stays mapped and therefore live.
            self.setGeometry(presenter_screen.availableGeometry())
            self.showMaximized()
        else:
            self.setGeometry(presenter_screen.geometry())
            self.showFullScreen()
        self._resize_private_bar()
        self.raise_()
        self.draw()
        self.update_clock()
        self.refresh_audience_state()
        QApplication.instance().installEventFilter(self)
        # Presenter View/Online has two top-level windows. A single application-level
        # shortcut set prevents duplicate activations while allowing keyboard control
        # from either NIRUPRES presentation window. Audience.keyForward and the event
        # filter remain independent fallbacks for compositor/window-manager quirks.
        self._install_shortcuts(self)
        for seq, delta in (("Ctrl+-", -2), ("Ctrl++", 2), ("Ctrl+=", 2)):
            sc = QShortcut(QKeySequence(seq), self)
            sc.setContext(Qt.ShortcutContext.ApplicationShortcut)
            sc.activated.connect(lambda d=delta: self.notes_zoom(d))
            self._shortcuts.append(sc)
        QTimer.singleShot(180, self.claim_focus)
        if open_now:
            self.audience.begin_opening()
        if share_check:
            self._share_checking = True
            QTimer.singleShot(260, self.open_share_check)

    # 2.3 presenter view helpers -------------------------------------------------

    def open_share_check(self):
        if self._closing:
            return
        d = ShareCheckDialog(self, self.audience)
        self.share_dialog = d
        d.finished.connect(self._share_check_done)
        d.open()
        d.raise_()
        d.activateWindow()

    def _share_check_done(self, result):
        self._share_checking = False
        self.share_dialog = None
        if result != QDialog.Accepted:
            self._finish_session()
            return
        # The talk starts now: reset the timer and run the opening fade.
        self.clock.restart()
        self._paused_total_ms = 0
        if self.data.get("cinematicOpen", True) and not self.data.get("reduceMotion", False):
            self.audience.begin_opening()
        self.update_clock()
        self.claim_focus()

    def _resize_private_bar(self):
        h = max(34, int(self.height() * 0.05))
        self.private_bar.setFixedHeight(h)
        _px(self.private_bar, max(14, int(h * 0.48)), QFont.Weight.Black)

    def resizeEvent(self, e):
        super().resizeEvent(e)
        if hasattr(self, "private_bar"):
            self._resize_private_bar()

    def notes_zoom(self, delta):
        if not self.dual:
            return
        self.notes_px = max(12, min(64, self.notes_px + delta))
        self._apply_notes_size()
        prefs = _load_presenter_prefs()
        prefs["notesPx"] = self.notes_px
        _save_presenter_prefs(prefs)

    def _apply_notes_size(self):
        f = QFont("Noto Sans")
        f.setPixelSize(self.notes_px)
        self.notes.setFont(f)
        self.notes.document().setDefaultFont(f)
        # The application style sheet sets font-size on every QWidget and won
        # over setFont() when the window was first polished: notes started at
        # 13 px until A-/A+ was pressed. A widget rule is more specific.
        self.notes.setStyleSheet(
            f'QTextEdit#presenterNotes{{font-family:"Noto Sans";font-size:{self.notes_px}px}}'
        )

    def _default_state(self):
        if self.online:
            return f"SHARE THE WINDOW  “{AUDIENCE_TITLE}”"
        return f"AUDIENCE → {self.audience_screen.name()}"

    def refresh_audience_state(self):
        if not self.dual or self._closing:
            return
        viewable = x11props.is_viewable(self.audience)
        warn = viewable is False or not self.audience.isVisible()
        if warn:
            text = "⚠  AUDIENCE WINDOW HIDDEN · a shared window is frozen · show it again"
        elif self._black_hold or self._closing_fade_started:
            text = "ENDING · press Esc again to close now"
        elif self._end_black:
            text = "END OF SLIDES · audience sees black · Next or Esc ends · Previous goes back"
        elif self.paused:
            text = "PAUSED · AUDIENCE DIMMED"
        elif self.curtain:
            text = "CURTAIN · AUDIENCE SCREEN HIDDEN"
        elif self.blank == "B":
            text = "AUDIENCE: BLACK"
        elif self.blank == "W":
            text = "AUDIENCE: WHITE"
        else:
            text = self._default_state()
        if self.audience_state.text() != text:
            self.audience_state.setText(text)
        if bool(self.audience_state.property("warn")) != warn:
            self.audience_state.setProperty("warn", warn)
            self.audience_state.style().unpolish(self.audience_state)
            self.audience_state.style().polish(self.audience_state)

    def _install_shortcuts(self, host):
        # Keep exactly one shortcut set per presentation session. ApplicationShortcut
        # makes navigation independent of which presentation child currently has focus.
        self._shortcuts = []
        bindings = [
            ("Escape", lambda: self.close_session()),
            ("Right", lambda: None if self.paused else self.navigate(1)),
            ("Down", lambda: None if self.paused else self.navigate(1)),
            (
                "Space",
                lambda: (
                    None
                    if self.paused
                    else (self.navigate(1) if not self.audience.toggle_video() else None)
                ),
            ),
            ("PgDown", lambda: None if self.paused else self.navigate(1)),
            ("Left", lambda: None if self.paused else self.navigate(-1)),
            ("Up", lambda: None if self.paused else self.navigate(-1)),
            ("PgUp", lambda: None if self.paused else self.navigate(-1)),
            ("Home", lambda: None if self.paused else self.navigate(-999)),
            ("End", lambda: None if self.paused else self.navigate(999)),
            (
                "B",
                lambda: None if self.paused else self.set_blank(None if self.blank == "B" else "B"),
            ),
            (
                "W",
                lambda: None if self.paused else self.set_blank(None if self.blank == "W" else "W"),
            ),
            ("S", lambda: None if self.paused else self.audience.set_focus_mode("SPOTLIGHT")),
            ("F", lambda: None if self.paused else self.audience.set_focus_mode("FOCUS")),
            ("Z", lambda: None if self.paused else self.audience.toggle_zoom()),
            ("C", lambda: None if self.paused else self.toggle_curtain()),
            ("P", lambda: self.toggle_pause()),
            ("[", lambda: None if self.paused else self.audience.adjust_focus_radius(-24)),
            ("]", lambda: None if self.paused else self.audience.adjust_focus_radius(24)),
        ]
        for sequence, callback in bindings:
            sc = QShortcut(QKeySequence(sequence), host)
            sc.setContext(Qt.ShortcutContext.ApplicationShortcut)
            sc.activated.connect(lambda cb=callback: None if self._share_checking else cb())
            self._shortcuts.append(sc)
        for n in range(1, 10):
            sc = QShortcut(QKeySequence(str(n)), host)
            sc.setContext(Qt.ShortcutContext.ApplicationShortcut)
            sc.activated.connect(
                lambda n=n: None
                if (self.paused or self._share_checking)
                else self.navigate(10000 + n - 1)
            )
            self._shortcuts.append(sc)

    def _screen_removed(self, screen):
        if screen in (self.presenter_screen, self.audience_screen):
            self.close_session()

    def claim_focus(self):
        if not self.isVisible():
            return
        self.raise_()
        self.activateWindow()
        self.setFocus(Qt.OtherFocusReason)
        if self.centralWidget():
            self.centralWidget().setFocus(Qt.OtherFocusReason)
        if self.share_dialog is not None and self.share_dialog.isVisible():
            # Keep the Share check above the presenter view (also without a WM).
            self.share_dialog.raise_()
            self.share_dialog.activateWindow()

    def claim_audience_focus(self):
        if not self.audience or not self.audience.isVisible():
            return
        self.audience.raise_()
        self.audience.activateWindow()
        self.audience.setFocus(Qt.OtherFocusReason)
        if self.audience.centralWidget():
            self.audience.centralWidget().setFocus(Qt.OtherFocusReason)

    def _visible(self, i):
        return 0 <= i < len(self.data["slides"]) and not self.data["slides"][i].get("hidden", False)

    def _nearest_visible(self, i, d=1):
        n = len(self.data["slides"])
        i = max(0, min(n - 1, i))
        if self._visible(i):
            return i
        for step in range(1, n):
            j = i + step * d
            if 0 <= j < n and self._visible(j):
                return j
        for j in range(n):
            if self._visible(j):
                return j
        return i

    def _next_visible(self, i, d):
        j = i + d
        while 0 <= j < len(self.data["slides"]):
            if self._visible(j):
                return j
            j += d
        return i

    def _active_elapsed_ms(self):
        current_pause = (
            (self.clock.elapsed() - self._pause_started_ms)
            if self.paused and self._pause_started_ms is not None
            else 0
        )
        return max(0, self.clock.elapsed() - self._paused_total_ms - current_pause)

    def update_clock(self):
        if not self.dual:
            return
        sec = self._active_elapsed_ms() // 1000
        if self._share_checking:
            sec = 0
        label = (
            f"{sec//60:02d}:{sec%60:02d}"
            if sec < 3600
            else f"{sec//3600}:{sec%3600//60:02d}:{sec%60:02d}"
        )
        pace = ""
        target = int(self.data.get("targetMinutes", 0) or 0)
        if target > 0:
            visible = [j for j, x in enumerate(self.data["slides"]) if not x.get("hidden")]
            pos = (visible.index(self.i) + 1) if self.i in visible else 1
            expected = (target * 60) * (pos / max(1, len(visible)))
            delta = int(sec - expected)
            sign = "+" if delta >= 0 else "−"
            d = abs(delta)
            pace = f"PACE {sign}{d//60:02d}:{d%60:02d}  ·  TARGET {target} MIN"
        self.timer.setText(label)
        had = bool(self.body.pace.text())
        self.body.pace.setText(pace)
        if had != bool(pace):
            self.body.relayout()
        self.body.clock.setText(time.strftime("%H:%M"))

    def draw(self):
        total = len(self.data["slides"])
        s = self.data["slides"][self.i]
        # position_label exists only in Presenter View/Online. In single-display
        # mode QWidget.pos is a built-in method, so using self.pos as a label
        # caused every keyboard navigation redraw to raise AttributeError.
        # The audience goes first: the preview mirror grabs the old slide for
        # its transition before the CURRENT preview renders the new one.
        self.audience.set_index(self.i)
        if self.dual:
            self.position_label.setText(f"{self.i+1} / {total}")
            self.now.render(
                s,
                self.data.get("theme", "B&W"),
                self.i,
                total,
                self.data.get("showNumbers", False),
                self.data.get("fontFamily", "Noto Sans"),
                self.data.get("aspect", "16:9"),
                self.data.get("showLogo", True),
                self.data.get("footerText", ""),
                self.data.get("footerAlign", "LEFT"),
                self.data.get("footerSize", "MEDIUM"),
            )
            notes = s.get("notes", "").strip()
            self.notes.setPlainText(notes)
            self.notes.setPlaceholderText("No speaker notes for this slide.")
            ni = self._next_visible(self.i, 1)
            if self.body.at_end != (ni == self.i):
                self.body.at_end = ni == self.i
                self.body.relayout()
            if ni == self.i:
                self.body.next_title.setText(
                    "End of presentation · Next fades to black"
                    if self._cinematic_close()
                    else "End of presentation"
                )
            else:
                nxt_s = self.data["slides"][ni]
                self.body.next_title.setText(
                    f"{ni+1}  ·  {nxt_s.get('title') or nxt_s.get('layout', '')}"
                )
            self.next.render(
                self.data["slides"][ni],
                self.data.get("theme", "B&W"),
                ni,
                total,
                False,
                self.data.get("fontFamily", "Noto Sans"),
                self.data.get("aspect", "16:9"),
                self.data.get("showLogo", True),
                self.data.get("footerText", ""),
                self.data.get("footerAlign", "LEFT"),
                self.data.get("footerSize", "MEDIUM"),
            )
        if getattr(self, "_marker_slide", None) != self.i:
            self._marker_slide = self.i
            self.slideChanged.emit(self.i)

    def navigate(self, d):
        if self._closing_fade_started or self._closing:
            return  # ending: the slides stay black; Esc again closes at once
        if self._end_black:
            if d == 1:
                self._finish_session()
                return
            # Back from the black end: fade the slides in again.
            self._end_black = False
            for engine in self.audience._engines():
                engine.opening()
            self.refresh_audience_state()
            if d == -1:
                return
        elif d == 1 and self._next_visible(self.i, 1) == self.i and self._cinematic_close():
            # Next on the last slide fades to black; Next or Esc again ends.
            self._end_black = True
            self.audience.begin_closing(None)
            self.refresh_audience_state()
            return
        if d == -999:
            self.i = self._nearest_visible(0, 1)
        elif d == 999:
            self.i = self._nearest_visible(len(self.data["slides"]) - 1, -1)
        elif d >= 10000:
            target = max(0, min(len(self.data["slides"]) - 1, d - 10000))
            self.i = self._nearest_visible(target, 1)
        else:
            self.i = self._next_visible(self.i, 1 if d > 0 else -1)
        if self.blank is not None:
            self.set_blank(None)
        self.draw()

    def set_blank(self, mode):
        self.blank = mode
        self.audience.set_blank(mode)
        self.refresh_audience_state()

    def toggle_curtain(self):
        self.curtain = not self.curtain
        self.audience.set_curtain(self.curtain)
        self.refresh_audience_state()

    def toggle_pause(self):
        if self._closing:
            return
        if not self.paused:
            # Pause owns the audience state. Clear temporary blackout/curtain so PAUS is unambiguous.
            if self.blank is not None:
                self.set_blank(None)
            if self.curtain:
                self.toggle_curtain()
            self.paused = True
            self._pause_started_ms = self.clock.elapsed()
            self.audience.set_pause(True)
            self.refresh_audience_state()
        else:
            now = self.clock.elapsed()
            if self._pause_started_ms is not None:
                self._paused_total_ms += max(0, now - self._pause_started_ms)
            self._pause_started_ms = None
            self.paused = False
            self.audience.set_pause(False)
            self.refresh_audience_state()
        self.update_clock() if self.dual else None

    def handle_key(self, key, mods=0, text=""):
        if self._share_checking:
            return
        if key == Qt.Key_Escape:
            self.close_session()
            return
        if key == Qt.Key_P:
            self.toggle_pause()
            return
        if self.paused:
            return
        if key == Qt.Key_B:
            self.set_blank(None if self.blank == "B" else "B")
            return
        if key == Qt.Key_W:
            self.set_blank(None if self.blank == "W" else "W")
            return
        if key == Qt.Key_S:
            self.audience.set_focus_mode("SPOTLIGHT")
            return
        if key == Qt.Key_F:
            self.audience.set_focus_mode("FOCUS")
            return
        if key == Qt.Key_Z:
            self.audience.toggle_zoom()
            return
        if key == Qt.Key_C:
            self.toggle_curtain()
            return
        if key == Qt.Key_BracketLeft:
            self.audience.adjust_focus_radius(-24)
            return
        if key == Qt.Key_BracketRight:
            self.audience.adjust_focus_radius(24)
            return
        if self.blank is not None:
            self.set_blank(None)
        if Qt.Key_0 <= key <= Qt.Key_9:
            self.jump += text or str(key - int(Qt.Key_0))
            return
        if key in (Qt.Key_Return, Qt.Key_Enter) and self.jump:
            try:
                self.navigate(10000 + int(self.jump) - 1)
            finally:
                self.jump = ""
            return
        if key == Qt.Key_Space and self.audience.toggle_video():
            return
        if key in (Qt.Key_Right, Qt.Key_Space, Qt.Key_PageDown, Qt.Key_Down):
            self.navigate(1)
        elif key in (Qt.Key_Left, Qt.Key_PageUp, Qt.Key_Up):
            self.navigate(-1)
        elif key == Qt.Key_Home:
            self.navigate(-999)
        elif key == Qt.Key_End:
            self.navigate(999)

    def changeEvent(self, e):
        super().changeEvent(e)
        # Back on the presenter workspace: keyboard focus goes straight to the
        # presenter view so the arrow keys work at once.
        if (
            e.type() == QEvent.Type.ActivationChange
            and self.isActiveWindow()
            and self.dual
            and not self._closing
            and self.centralWidget() is not None
        ):
            self.centralWidget().setFocus(Qt.ActiveWindowFocusReason)
            if self.share_dialog is not None and self.share_dialog.isVisible():
                self.share_dialog.raise_()
                self.share_dialog.activateWindow()

    def eventFilter(self, watched, event):
        # Online mode: if the window manager hands focus to the covered audience
        # window (e.g. after a workspace switch), give it back to the presenter
        # view and keep the presenter view on top.
        if (
            watched is self.audience
            and event.type() == QEvent.Type.WindowActivate
            and self.online
            and self.dual
            and not self._closing
        ):
            QTimer.singleShot(0, self.claim_focus)
        # Capture presentation keys even when a child widget/button owns focus.
        if event.type() == QEvent.Type.KeyPress:
            belongs = watched is self or watched is self.audience
            if isinstance(watched, QWidget):
                belongs = (
                    belongs or self.isAncestorOf(watched) or self.audience.isAncestorOf(watched)
                )
            if belongs:
                key = int(event.key())
                presentation_keys = (
                    Qt.Key_Escape,
                    Qt.Key_B,
                    Qt.Key_W,
                    Qt.Key_S,
                    Qt.Key_F,
                    Qt.Key_Z,
                    Qt.Key_C,
                    Qt.Key_P,
                    Qt.Key_BracketLeft,
                    Qt.Key_BracketRight,
                    Qt.Key_Return,
                    Qt.Key_Enter,
                    Qt.Key_Right,
                    Qt.Key_Space,
                    Qt.Key_PageDown,
                    Qt.Key_Down,
                    Qt.Key_Left,
                    Qt.Key_PageUp,
                    Qt.Key_Up,
                    Qt.Key_Home,
                    Qt.Key_End,
                )
                if key in presentation_keys or Qt.Key_0 <= key <= Qt.Key_9:
                    self.handle_key(key, _mod_int(event.modifiers()), event.text())
                    return True
        return super().eventFilter(watched, event)

    def keyPressEvent(self, e):
        self.handle_key(int(e.key()), _mod_int(e.modifiers()), e.text())
        e.accept()

    def _cinematic_close(self):
        return self.data.get("cinematicClose", True) and not self.data.get("reduceMotion", False)

    def close_session(self):
        # A second Escape always means "close now", including during fade/black
        # hold and on the black end after the last slide.
        if self._black_hold or self._closing_fade_started or self._end_black:
            self._finish_session()
            return
        if self._closing:
            return
        if self._cinematic_close():
            self._closing_fade_started = True
            self.audience.begin_closing(self._begin_black_hold)
            self.refresh_audience_state()
            return
        self._finish_session()

    def _begin_black_hold(self):
        if self._closing:
            return
        self._black_hold = True
        # A short black hold so the end reads as an end; Esc again skips it.
        QTimer.singleShot(BLACK_HOLD_MS, self._finish_session)

    def _finish_session(self):
        if self._closing:
            return
        self._closing = True
        if self.share_dialog is not None:
            try:
                self.share_dialog.blockSignals(True)
                self.share_dialog.close()
            except RuntimeError:
                pass
            self.share_dialog = None
        self._black_hold = False
        if self.paused:
            self.paused = False
            self.audience.set_pause(False)
        if self.audience:
            self.audience.blockSignals(True)
            self.audience.close()
        self.close()

    def closeEvent(self, e):
        if not self._closing:
            self._closing = True
            if self.audience:
                self.audience.blockSignals(True)
                self.audience.close()
        try:
            QApplication.instance().removeEventFilter(self)
        except Exception:
            pass
        self.presenting_mode.stop()
        self.finished.emit()
        e.accept()
