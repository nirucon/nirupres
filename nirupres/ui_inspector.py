"""The slide inspector: sections, collapsed summaries, remembered state, width fitting.

Part of the NIRUPRES editor window (nirupres.main.Main); split out of main.py
in 2.2 without behaviour changes.
"""

import json, sys, zipfile, tempfile, shutil, uuid, re, stat, os, platform, hashlib, time
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
    QFileSystemWatcher,
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
    QScrollArea,
    QFrame,
    QToolButton,
)

from .foundation import *
from .foundation import _plain_markup
from .presentation import *
from .editor import *


class InspectorMixin:
    def build_inspector(self, root):
        right = QWidget()
        self.right_panel = right
        rv = QVBoxLayout(right)
        rv.setContentsMargins(6, 10, 10, 10)
        rh = QHBoxLayout()
        rh.addWidget(QLabel("INSPECTOR"))
        rh.addStretch()
        rc = QPushButton("›")
        self.fit_text(rc, 28, fixed=True)
        rc.setToolTip("Hide Inspector panel")
        rc.clicked.connect(lambda: self.set_side_panel("right", False))
        rh.addWidget(rc)
        rv.addLayout(rh)
        # The inspector scrolls vertically so the editor fits a 1366x768 screen;
        # the panel never squeezes its controls horizontally (see refit_text).
        self.inspector_scroll = QScrollArea()
        self.inspector_scroll.setWidgetResizable(True)
        self.inspector_scroll.setFrameShape(QFrame.NoFrame)
        self.inspector_scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarAsNeeded)
        inspector_body = QWidget()
        inspector_body.setObjectName("inspectorBody")
        self._inspector_body = inspector_body
        inspector_body.installEventFilter(self)
        panel_layout = rv
        rv = QVBoxLayout(inspector_body)
        rv.setContentsMargins(0, 0, 4, 0)
        self.inspector_scroll.setWidget(inspector_body)
        panel_layout.addWidget(self.inspector_scroll, 1)
        content = QGroupBox("▼  CONTENT")
        content.setCheckable(True)
        content.setChecked(True)
        cf = QFormLayout(content)
        self.layout = QComboBox()
        self.layout.addItems(LAYOUTS)
        cf.addRow("Layout", self.layout)
        self.title = QLineEdit()
        cf.addRow("Title", self.title)
        self.body = QTextEdit()
        self.body.setMinimumHeight(100)
        cf.addRow("Body", self.body)
        self.list_style = QComboBox()
        self.list_style.addItems(["NUMBERS", "DOTS", "DASHES", "NONE"])
        self.list_style.setToolTip("Marker style for BULLETS and AGENDA layouts")
        cf.addRow("List style", self.list_style)
        rv.addWidget(content)
        self.content_group = content
        content.toggled.connect(lambda on, g=content: self.collapse_group(g, on))
        image = QGroupBox("▼  IMAGE")
        self.image_group = image
        image.setCheckable(True)
        image.setChecked(True)
        iv = QVBoxLayout(image)
        rr = QHBoxLayout()
        self.img = DropLine()
        self.img.setPlaceholderText("Drop image here or choose…")
        self.img.fileDropped.connect(self.set_image)
        b = QPushButton("CHOOSE")
        b.clicked.connect(self.choose_image)
        rr.addWidget(self.img, 1)
        rr.addWidget(b)
        iv.addLayout(rr)
        ir = QHBoxLayout()
        self.imode = QComboBox()
        self.imode.addItems(["FIT", "FILL"])
        self.imode.setToolTip("FIT shows the whole image · FILL crops to the image frame")
        self.treatment = QComboBox()
        self.treatment.addItems(["NATURAL", "MONO", "DIM", "CONTRAST"])
        self.treatment.setToolTip("Non-destructive image treatment")
        self.mono = QCheckBox("MONO")
        self.mono.hide()
        clear = QPushButton("REMOVE")
        clear.clicked.connect(lambda: self.img.setText(""))
        ir.addWidget(QLabel("MODE", objectName="deckLabel"))
        ir.addWidget(self.imode)
        ir.addSpacing(8)
        ir.addWidget(QLabel("TREATMENT", objectName="deckLabel"))
        ir.addWidget(self.treatment, 1)
        ir.addWidget(clear)
        iv.addLayout(ir)
        fr = QHBoxLayout()
        self.fx = QComboBox()
        self.fx.addItems(["LEFT", "CENTER", "RIGHT"])
        self.fy = QComboBox()
        self.fy.addItems(["TOP", "CENTER", "BOTTOM"])
        fr.addWidget(QLabel("FOCAL X", objectName="deckLabel"))
        fr.addWidget(self.fx)
        fr.addWidget(QLabel("FOCAL Y", objectName="deckLabel"))
        fr.addWidget(self.fy)
        iv.addLayout(fr)
        bglabel = QLabel("BACKGROUND IMAGE")
        bglabel.setObjectName("deckLabel")
        iv.addWidget(bglabel)
        bgr = QHBoxLayout()
        self.bgimg = DropLine()
        self.bgimg.setPlaceholderText("Optional full-slide background…")
        self.bgimg.fileDropped.connect(self.set_background_image)
        bgchoose = QPushButton("CHOOSE")
        bgchoose.clicked.connect(self.choose_background_image)
        bgclear = QPushButton("REMOVE")
        bgclear.clicked.connect(lambda: self.bgimg.setText(""))
        bgr.addWidget(self.bgimg, 1)
        bgr.addWidget(bgchoose)
        bgr.addWidget(bgclear)
        iv.addLayout(bgr)
        bgopts = QHBoxLayout()
        self.bgmono = QCheckBox("MONO")
        self.bgdim = QSlider(Qt.Horizontal)
        self.bgdim.setRange(0, 90)
        self.bgdim.setToolTip("Theme-coloured dim veil · 75% is a safe readable default")
        self.bgblur = QSlider(Qt.Horizontal)
        self.bgblur.setRange(0, 20)
        self.bgblur.setToolTip("Background-only blur")
        bgopts.addWidget(self.bgmono)
        bgopts.addWidget(QLabel("DIM"))
        bgopts.addWidget(self.bgdim, 1)
        bgopts.addWidget(QLabel("BLUR"))
        bgopts.addWidget(self.bgblur, 1)
        iv.addLayout(bgopts)
        bgf = QHBoxLayout()
        self.bgfx = QComboBox()
        self.bgfx.addItems(["LEFT", "CENTER", "RIGHT"])
        self.bgfy = QComboBox()
        self.bgfy.addItems(["TOP", "CENTER", "BOTTOM"])
        bgf.addWidget(QLabel("BG FOCAL X", objectName="deckLabel"))
        bgf.addWidget(self.bgfx)
        bgf.addWidget(QLabel("Y", objectName="deckLabel"))
        bgf.addWidget(self.bgfy)
        iv.addLayout(bgf)
        bgmr = QHBoxLayout()
        bgmr.addWidget(QLabel("BG MOTION", objectName="deckLabel"))
        self.bgmotion = QComboBox()
        self.bgmotion.addItems(["NONE", "KEN BURNS"])
        self.bgmotion.setToolTip(
            "Subtle background-only pan during presentation · disabled by Reduce motion"
        )
        bgmr.addWidget(self.bgmotion, 1)
        iv.addLayout(bgmr)
        self.caption = QLineEdit()
        self.caption.setPlaceholderText("Caption / attribution")
        iv.addWidget(self.caption)
        self.image_request = QLineEdit()
        self.image_request.setPlaceholderText("AI image request / desired image…")
        iv.addWidget(self.image_request)
        rv.addWidget(image)
        image.toggled.connect(lambda on, g=image: self.collapse_group(g, on))
        video = QGroupBox("▶  VIDEO")
        self.video_group = video
        video.setCheckable(True)
        video.setChecked(False)
        vv = QVBoxLayout(video)
        self.video_source = QLineEdit()
        self.video_source.setPlaceholderText("YouTube / Vimeo URL or local video path…")
        vv.addWidget(self.video_source)
        vpr = QHBoxLayout()
        self.video_poster = DropLine()
        self.video_poster.setPlaceholderText("Optional poster image…")
        vpchoose = QPushButton("POSTER…")
        vpchoose.clicked.connect(self.choose_video_poster)
        vlocal = QPushButton("LOCAL VIDEO…")
        vlocal.clicked.connect(self.choose_video)
        vpr.addWidget(self.video_poster, 1)
        vpr.addWidget(vpchoose)
        vpr.addWidget(vlocal)
        vv.addLayout(vpr)
        vo = QHBoxLayout()
        self.video_autoplay = QCheckBox("AUTOPLAY")
        self.video_muted = QCheckBox("MUTED")
        vo.addWidget(self.video_autoplay)
        vo.addWidget(self.video_muted)
        vo.addStretch()
        vv.addLayout(vo)
        rv.addWidget(video)
        video.toggled.connect(lambda on, g=video: self.collapse_group(g, on))
        self.collapse_group(video, False)
        style = QGroupBox("▶  STYLE")
        self.style_group = style
        style.setCheckable(True)
        style.setChecked(False)
        sf = QFormLayout(style)
        self.override_theme = QCheckBox("Override deck theme")
        sf.addRow("", self.override_theme)
        self.slide_theme = QComboBox()
        self.slide_theme.addItems(list(THEMES.keys()))
        decorate_theme_combo(self.slide_theme)
        self.slide_theme.setEnabled(False)
        sf.addRow("Slide theme", self.slide_theme)
        self.logo_mode = QComboBox()
        self.logo_mode.addItems(["AUTO", "ON", "OFF"])
        self.logo_mode.setToolTip(
            "AUTO follows layout rules · ON forces logo · OFF hides logo on this slide"
        )
        sf.addRow("Logo", self.logo_mode)
        self.accent_override = QComboBox()
        self.accent_override.addItems(["AUTO", "BLUE", "GREEN", "YELLOW", "RED", "PINK", "PURPLE"])
        self.accent_override.setToolTip(self.profile_info["accent_tooltip"])
        sf.addRow("Accent", self.accent_override)
        self.override_font = QCheckBox("Override deck font")
        sf.addRow("", self.override_font)
        self.slide_font = QComboBox()
        self.slide_font.addItems(QFontDatabase.families())
        # Size from a sensible text length, not the longest installed family name.
        self.slide_font.setSizeAdjustPolicy(QComboBox.AdjustToMinimumContentsLengthWithIcon)
        self.slide_font.setMinimumContentsLength(14)
        self.slide_font.setEnabled(False)
        sf.addRow("Slide font", self.slide_font)
        self.override_theme.toggled.connect(self.slide_theme.setEnabled)
        self.override_font.toggled.connect(self.slide_font.setEnabled)
        tr = QHBoxLayout()
        self.fontscale = QComboBox()
        self.fontscale.addItems(["−", "AUTO", "+"])
        self.align = QComboBox()
        self.align.addItems(["LEFT", "CENTER", "RIGHT"])
        self.weight = QComboBox()
        self.weight.addItems(["AUTO", "LIGHT", "REGULAR", "BOLD"])
        # Two rows instead of one: the single row needed ~500 px and forced the
        # inspector wider than a 1366 px screen allows.
        tr = QGridLayout()
        tr.setContentsMargins(0, 0, 0, 0)
        tr.addWidget(QLabel("Size"), 0, 0)
        tr.addWidget(self.fontscale, 0, 1)
        tr.addWidget(QLabel("Align"), 0, 2)
        tr.addWidget(self.align, 0, 3)
        tr.addWidget(QLabel("Weight"), 1, 0)
        tr.addWidget(self.weight, 1, 1, 1, 3)
        tr.setColumnStretch(1, 1)
        tr.setColumnStretch(3, 1)
        sf.addRow("Typography", tr)
        self.brightness = QSlider(Qt.Horizontal)
        self.brightness.setRange(-30, 30)
        self.brightness.setValue(0)
        self.brightness.setToolTip("Image brightness · 0 = unchanged")
        sf.addRow("Brightness", self.brightness)
        self.contrast = QSlider(Qt.Horizontal)
        self.contrast.setRange(0, 50)
        self.contrast.setToolTip("Image contrast · 0 = unchanged")
        sf.addRow("Contrast", self.contrast)
        self.overlay = QSlider(Qt.Horizontal)
        self.overlay.setRange(0, 60)
        self.overlay.setToolTip("Dark overlay strength · 0 = none")
        sf.addRow("Overlay", self.overlay)
        self.blur = QSlider(Qt.Horizontal)
        self.blur.setRange(0, 10)
        self.blur.setToolTip("Image blur · 0 = none")
        sf.addRow("Blur", self.blur)
        self.apply_style_btn = QPushButton("APPLY STYLE TO SELECTED")
        self.apply_style_btn.setToolTip(
            "Copy this slide’s visual style to all selected slides without replacing their content"
        )
        self.apply_style_btn.clicked.connect(self.apply_style_to_selected)
        sf.addRow("", self.apply_style_btn)
        rv.addWidget(style)
        style.toggled.connect(lambda on, g=style: self.collapse_group(g, on))
        self.collapse_group(style, False)
        pres = QGroupBox("▶  PRESENTATION")
        self.presentation_group = pres
        pres.setCheckable(True)
        pres.setChecked(False)
        pv = QVBoxLayout(pres)
        self.hidden = QCheckBox("HIDDEN SLIDE — skip while presenting")
        pv.addWidget(self.hidden)
        transrow = QHBoxLayout()
        transrow.addWidget(QLabel("TRANSITION", objectName="deckLabel"))
        self.slide_transition = QComboBox()
        self.slide_transition.addItems(["DECK", "NONE", "FADE", "MORPH", "REVEAL"])
        self.slide_transition.setToolTip("Override the deck transition for this slide")
        transrow.addWidget(self.slide_transition, 1)
        self.transition_preview_btn = QPushButton("▶ PREVIEW")
        self.transition_preview_btn.setToolTip("Preview the transition into the current slide")
        self.transition_preview_btn.clicked.connect(self.preview_transition)
        transrow.addWidget(self.transition_preview_btn)
        pv.addLayout(transrow)
        pv.addWidget(QLabel("SPEAKER NOTES"))
        self.notes = QTextEdit()
        self.notes.setPlaceholderText("Private notes — never shown to audience")
        self.notes.setMinimumHeight(110)
        pv.addWidget(self.notes)
        rv.addWidget(pres)
        pres.toggled.connect(lambda on, g=pres: self.collapse_group(g, on))
        self.collapse_group(pres, False)
        self.restore_inspector_state()
        rv.addStretch()
        root.addWidget(right)

    def _fit_inspector_width(self):
        scroll = getattr(self, "inspector_scroll", None)
        if scroll is None:
            return
        inner = scroll.widget()
        bar = scroll.verticalScrollBar().sizeHint().width()
        need = inner.minimumSizeHint().width() + bar + 2 * scroll.frameWidth()
        # On a narrow screen (or with a large chrome font) the inspector scrolls
        # horizontally rather than pushing the window off-screen.
        rest = self.root_splitter.widget(1).minimumSizeHint().width() + 24
        if self.left_panel.isVisible():
            rest += self.left_panel.minimumSizeHint().width()
        cap = max(260, self._available_width() - rest)
        width = min(need, cap)
        if scroll.minimumWidth() != width:
            scroll.setMinimumWidth(width)

    def _available_width(self):
        screen = self.screen() or QApplication.primaryScreen()
        return screen.availableGeometry().width() if screen is not None else 1366

    def eventFilter(self, obj, event):
        # The inspector body re-lays out when sections expand or the chrome
        # font changes; keep the scroll area wide enough for its controls.
        if obj is getattr(self, "_inspector_body", None) and event.type() == QEvent.LayoutRequest:
            QTimer.singleShot(0, self._fit_inspector_width)
        return super().eventFilter(obj, event)

    INSPECTOR_SECTIONS = ("CONTENT", "IMAGE", "VIDEO", "STYLE", "PRESENTATION")

    SUMMARY_LIMIT = 30

    def _inspector_groups(self):
        return {
            "CONTENT": self.content_group,
            "IMAGE": self.image_group,
            "VIDEO": self.video_group,
            "STYLE": self.style_group,
            "PRESENTATION": self.presentation_group,
        }

    @staticmethod
    def _group_base(g):
        base = g.property("baseTitle")
        if not base:
            base = g.title().replace("▼  ", "").replace("▶  ", "").split("  ·  ")[0]
            g.setProperty("baseTitle", base)
        return base

    def collapse_group(self, g, on):
        base = self._group_base(g)
        g.setTitle(("▼  " if on else "▶  ") + base)
        for w in g.findChildren(QWidget, options=Qt.FindDirectChildrenOnly):
            w.setVisible(on)
        g.setMaximumHeight(16777215 if on else 34)
        if not getattr(self, "_restoring_inspector", True):
            self.save_inspector_state()
        if not on:
            self._apply_group_summary(g)

    def _apply_group_summary(self, g):
        base = self._group_base(g)
        if g.isChecked():
            g.setToolTip("")
            return
        summary = self.inspector_summary(base)
        shown = summary
        if len(shown) > self.SUMMARY_LIMIT:
            shown = shown[: self.SUMMARY_LIMIT - 1].rstrip() + "…"
        g.setTitle("▶  " + base + (f"  ·  {shown}" if shown else ""))
        g.setToolTip(f"{base.title()}: {summary}" if summary else "")

    def update_inspector_summaries(self):
        if not hasattr(self, "presentation_group"):
            return
        for g in self._inspector_groups().values():
            if not g.isChecked():
                self._apply_group_summary(g)

    def inspector_summary(self, section):
        """One-line state of a collapsed inspector section, e.g. "fill · center"."""
        s = self.current() or {}
        place = {0.0: "left", 0.5: "center", 1.0: "right"}
        vplace = {0.0: "top", 0.5: "center", 1.0: "bottom"}

        def focal(x, y):
            fx = place.get(float(s.get(x, 0.5)), "custom")
            fy = vplace.get(float(s.get(y, 0.5)), "custom")
            return "center" if fx == fy == "center" else f"{fx}/{fy}"

        if section == "CONTENT":
            title = (s.get("title") or "").strip() or "untitled"
            return f"{str(s.get('layout', 'TITLE')).lower()} · {title}"
        if section == "IMAGE":
            parts = []
            if s.get("image"):
                parts.append(str(s.get("imageMode", "FILL")).lower())
                parts.append(focal("focalX", "focalY"))
                treatment = str(s.get("imageTreatment", "NATURAL")).lower()
                if treatment != "natural":
                    parts.append(treatment)
            if s.get("backgroundImage"):
                parts.append("background")
                if s.get("backgroundMotion", "NONE") != "NONE":
                    parts.append("ken burns")
            return " · ".join(parts) or "none"
        if section == "VIDEO":
            source = str(s.get("videoSource") or "").strip()
            if not source:
                return "none"
            parts = ["web" if re.match(r"^https?://", source, re.I) else "local"]
            if s.get("videoAutoplay"):
                parts.append("autoplay")
            if s.get("videoMuted"):
                parts.append("muted")
            return " · ".join(parts)
        if section == "STYLE":
            parts = [
                (
                    "deck theme"
                    if s.get("themeOverride", "DECK") == "DECK"
                    else str(s.get("themeOverride")).lower()
                ),
                str(s.get("align", "LEFT")).lower(),
            ]
            if s.get("fontFamily", "DECK") != "DECK":
                parts.append(str(s.get("fontFamily")))
            if s.get("logoMode", "AUTO") != "AUTO":
                parts.append("logo " + str(s.get("logoMode")).lower())
            return " · ".join(parts)
        if section == "PRESENTATION":
            parts = ["hidden" if s.get("hidden") else "shown"]
            if (s.get("notes") or "").strip():
                parts.append("notes")
            transition = normalize_transition(s.get("transition", "DECK"), True)
            parts.append("deck transition" if transition == "DECK" else transition.lower())
            return " · ".join(parts)
        return ""

    def _ui_state_path(self):
        return DATA / "ui.json"

    def restore_inspector_state(self):
        """Open/closed state per section: CONTENT open, the rest closed by default."""
        state = {}
        try:
            raw = json.loads(self._ui_state_path().read_text(encoding="utf-8"))
            state = raw.get("inspector", {}) if isinstance(raw, dict) else {}
        except (OSError, ValueError):
            state = {}
        self._restoring_inspector = True
        try:
            for name, g in self._inspector_groups().items():
                g.setProperty("baseTitle", self._group_base(g))
                want = state.get(name, name == "CONTENT")
                want = bool(want) if isinstance(want, bool) else name == "CONTENT"
                if g.isChecked() != want:
                    g.setChecked(want)  # emits toggled -> collapse_group
                else:
                    self.collapse_group(g, want)
        finally:
            self._restoring_inspector = False

    def save_inspector_state(self):
        if not hasattr(self, "presentation_group"):
            return
        path = self._ui_state_path()
        try:
            raw = json.loads(path.read_text(encoding="utf-8")) if path.exists() else {}
            if not isinstance(raw, dict):
                raw = {}
        except (OSError, ValueError):
            raw = {}
        raw["inspector"] = {name: g.isChecked() for name, g in self._inspector_groups().items()}
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            tmp = path.with_name(path.name + ".tmp")
            tmp.write_text(json.dumps(raw, indent=2) + "\n", encoding="utf-8")
            tmp.replace(path)
        except OSError:
            pass
