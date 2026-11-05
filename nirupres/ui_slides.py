"""The slide list panel: building, reordering (drag and Alt+Up/Down) and accessible names.

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


class SlideListMixin:
    def build_slide_panel(self, root):
        left = QWidget()
        self.left_panel = left
        lv = QVBoxLayout(left)
        lv.setContentsMargins(10, 10, 6, 10)
        hh = QHBoxLayout()
        hh.addWidget(QLabel("SLIDES"))
        hh.addStretch()
        self.count = QLabel()
        hh.addWidget(self.count)
        self.outline_btn = QPushButton("OUTLINE")
        self.fit_text(self.outline_btn, 72)
        self.outline_btn.setToolTip("Switch between slide thumbnails and compact outline")
        self.outline_btn.setCheckable(True)
        self.outline_btn.toggled.connect(self.toggle_outline)
        hh.addWidget(self.outline_btn)
        collapse = QPushButton("‹")
        self.fit_text(collapse, 28, fixed=True)
        collapse.setToolTip("Hide Slides panel")
        collapse.clicked.connect(lambda: self.set_side_panel("left", False))
        hh.addWidget(collapse)
        lv.addLayout(hh)
        self.list = SlideList()
        self.list.setSpacing(5)
        self.list.currentRowChanged.connect(self.select)
        self.list.reordered.connect(self.drag_reorder)
        self.list.moveRequested.connect(self.move_slide_keyboard)
        lv.addWidget(self.list)
        plus = QPushButton("+ ADD SLIDE")
        plus.clicked.connect(self.add)
        lv.addWidget(plus)
        presets = QPushButton("PRESETS")
        presets.setToolTip("Insert a ready-made slide pattern")
        self.fit_text(presets)
        pmenu = QMenu(presets)
        for label, key in [
            (self.profile_info["title_preset"], "TITLE"),
            ("Agenda", "BULLETS"),
            ("Section divider", "SECTION"),
            ("Key message", "STATEMENT"),
            ("Three points", "BULLETS"),
            ("Image + message", "SPLIT"),
            ("Result / KPI", "BIG NUMBER"),
            ("Thank you", "END"),
        ]:
            act = pmenu.addAction(label, lambda _=False, k=key: self.add_preset(k))
            if key == "TITLE" and not hasattr(self, "title_preset_action"):
                self.title_preset_action = act
        self.presets_menu = pmenu
        presets.setMenu(pmenu)
        lv.addWidget(presets)
        root.addWidget(left)

    def drag_reorder(self, old, new):
        if old < 0 or new < 0 or old == new:
            return
        slide = self.data["slides"].pop(old)
        self.data["slides"].insert(new, slide)
        self.refresh(new)
        self.mark_dirty()
        self.push_history()

    def slide_accessible_text(self, i):
        slides = self.data.get("slides", [])
        s = slides[i] if 0 <= i < len(slides) else {}
        title = _plain_markup(s.get("title") or "") or "untitled"
        hidden = ", hidden" if s.get("hidden") else ""
        return f"Slide {i + 1} of {len(slides)}: {title} ({s.get('layout', 'TITLE')}{hidden})"

    def move_slide_keyboard(self, d):
        i = self.list.currentRow()
        j = i + d
        if i < 0 or j < 0 or j >= len(self.data["slides"]):
            return
        self.move_slide(d)
        self.list.setFocus(Qt.OtherFocusReason)
        text = self.slide_accessible_text(j)
        self.list.setAccessibleDescription(f"Moved. {text}")
        self.statusBar().showMessage(f"Moved to {text}", 2500)

    def move_slide(self, d):
        i = self.list.currentRow()
        j = i + d
        if i < 0 or j < 0 or j >= len(self.data["slides"]):
            return
        self.data["slides"][i], self.data["slides"][j] = (
            self.data["slides"][j],
            self.data["slides"][i],
        )
        self.refresh(j)
        self.mark_dirty()
        self.push_history()
