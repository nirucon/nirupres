"""The grouped editor toolbar (File | Slide | View | Present) and its ⋯ overflow.

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


class ToolbarMixin:
    def build_toolbar(self):
        tb = QToolBar()
        tb.setMovable(False)
        self.addToolBar(tb)
        self.toolbar = tb
        # 2.2 (U2): the toolbar is grouped File | Slide | View | Present with
        # visible separators. Every QAction is also added to the window, so its
        # shortcut keeps working when its group moves into the ⋯ overflow menu
        # (or the whole toolbar is hidden in focus mode).
        self.toolbar_sections = {}
        self._toolbar_section = None

        def section(name):
            if self.toolbar_sections:
                self.toolbar_sections[self._toolbar_section].append(tb.addSeparator())
            self._toolbar_section = name
            self.toolbar_sections[name] = []

        def tool(action):
            self.addAction(action)
            button = QToolButton()
            button.setDefaultAction(action)
            button.setFocusPolicy(Qt.NoFocus)
            self.toolbar_sections[self._toolbar_section].append(tb.addWidget(button))
            return action

        def widget(w):
            self.toolbar_sections[self._toolbar_section].append(tb.addWidget(w))
            return w

        section("File")
        self.new_action = tool(self.action("NEW", self.new, "Ctrl+N"))
        self.open_action = tool(self.action("OPEN", self.open, "Ctrl+O"))
        self.save_action = tool(self.action("SAVE", self.save, "Ctrl+S"))
        self.addAction(self.action("SAVE AS", self.save_as, "Ctrl+Shift+S"))
        export = QPushButton("EXPORT")
        export.setObjectName("menuButton")
        export.setToolTip("Export presentation…")
        em = QMenu(export)
        em.addAction(self.action("Current slide → PNG", self.export_png, "Ctrl+Alt+E"))
        em.addAction(self.action("All slides → PNG", self.export_all_png))
        em.addAction(self.action("Deck → PDF", self.export_pdf, "Ctrl+Shift+E"))
        em.addAction(self.action("Deck → PPTX (pixel-perfect)", self.export_pptx))
        em.addSeparator()
        em.addAction(self.action("NIRUPRES Markdown…", self.export_markdown))
        em.addAction(self.action("AI brief (current deck)…", self.export_ai_brief))
        em.addAction(self.action("Copy AI prompt", self.copy_ai_prompt))
        em.addAction(self.action("AI template/spec…", self.export_ai_template))
        export.setMenu(em)
        self.export_menu = em
        # Export shortcuts must work while EXPORT sits in the overflow menu.
        self.addActions([a for a in em.actions() if not a.isSeparator()])
        self.fit_text(export)
        widget(export)
        self.import_action = tool(self.action("IMPORT MD", self.import_markdown))
        section("Slide")
        self.add_action = tool(self.action("+ SLIDE", self.add, "Ctrl+Enter"))
        self.dup_action = tool(self.action("DUPLICATE", self.dup, "Ctrl+D"))
        self.delete_action = tool(self.action("DELETE", self.delete, "Delete"))
        self.undo_action = tool(self.action("↶", self.undo, "Ctrl+Z"))
        self.undo_action.setToolTip("Undo · Ctrl+Z")
        self.redo_action = tool(self.action("↷", self.redo, "Ctrl+Shift+Z"))
        self.redo_action.setToolTip("Redo · Ctrl+Shift+Z")
        section("View")
        self.focus_action = tool(self.action("FOCUS", self.toggle_focus, "Tab"))
        self.chrome_action = QAction("NIRUX", self)
        self.chrome_action.setCheckable(True)
        self.chrome_action.setChecked(self.chrome_follow)
        self.chrome_action.setEnabled(self.chrome_available)
        self.chrome_action.setToolTip("Follow NIRUX editor chrome · slide themes stay independent")
        self.chrome_action.toggled.connect(self.set_chrome_follow)
        tool(self.chrome_action)
        self.palette_action = tool(self.action("⌘", self.command_palette, "Ctrl+K"))
        self.palette_action.setToolTip("Command palette · Ctrl+K")
        about = QPushButton("?")
        self.fit_text(about, 34, fixed=True)
        about.setToolTip("About NIRUPRES / diagnostics")
        about.clicked.connect(self.about)
        widget(about)
        section("Present")
        present = QPushButton("▶ PRESENT")
        present.clicked.connect(lambda: self.present(False))
        present.setObjectName("present")
        present.setToolTip("Present from current slide · F5")
        widget(present)
        pstart = QPushButton("|▶")
        pstart.setObjectName("presentAux")
        self.fit_text(pstart, 42, fixed=True)
        pstart.setToolTip("Present from beginning · Shift+F5")
        pstart.clicked.connect(lambda: self.present(True))
        widget(pstart)
        pview = QPushButton("▣")
        pview.setObjectName("presentAux")
        self.fit_text(pview, 42, fixed=True)
        pview.setToolTip("Presenter View / display")
        pview.clicked.connect(self.presenter_view)
        widget(pview)
        talk = QPushButton("● TALK")
        talk.setObjectName("presentAux")
        self.fit_text(talk, 84)
        talk.setToolTip("Present and record with NIRUCLIP · slide chapters and subtitles")
        tmenu = QMenu(talk)
        for label, profile in [
            ("Screen + microphone", "screen-mic"),
            ("Screen + camera", "screen-camera"),
            ("Camera + microphone", "camera-mic"),
        ]:
            tmenu.addAction(label, lambda _=False, p=profile: self.start_talk(p))
        tmenu.addSeparator()
        tmenu.addAction("Talk helps / profiles…", lambda: self.talk_help())
        talk.setMenu(tmenu)
        self.talk_menu = tmenu
        widget(talk)
        online = QPushButton("◉ ONLINE")
        online.setObjectName("presentAux")
        self.fit_text(online, 86)
        online.setToolTip(
            "Online presentation · opens a clean Audience window for Teams/Meet/Zoom sharing"
        )
        online.clicked.connect(lambda: self.online_present(False))
        widget(online)
        pover = QPushButton("▦")
        pover.setObjectName("presentAux")
        self.fit_text(pover, 42, fixed=True)
        pover.setToolTip("Presentation overview")
        pover.clicked.connect(self.presentation_overview)
        widget(pover)
        # Overflow: groups that do not fit move into ⋯, View first, then Slide,
        # then File. Present always stays on the toolbar.
        self.overflow_button = QToolButton()
        self.overflow_button.setText("⋯")
        self.overflow_button.setObjectName("overflow")
        self.overflow_button.setToolTip("More toolbar actions")
        self.overflow_button.setFocusPolicy(Qt.NoFocus)
        self.overflow_button.setPopupMode(QToolButton.InstantPopup)
        self.overflow_menu = QMenu(self.overflow_button)
        self.overflow_menu.aboutToShow.connect(self._fill_overflow_menu)
        self.overflow_button.setMenu(self.overflow_menu)
        self.overflow_action = tb.addWidget(self.overflow_button)
        self.overflow_action.setVisible(False)
        self.toolbar_hidden_sections = []

    TOOLBAR_COLLAPSE_ORDER = ("View", "Slide", "File")

    def _toolbar_width_needed(self):
        tb = self.toolbar
        margins = tb.contentsMargins()
        spacing = max(0, tb.layout().spacing()) if tb.layout() is not None else 4
        total = margins.left() + margins.right() + 12
        for name, items in self.toolbar_sections.items():
            if name in self.toolbar_hidden_sections:
                continue
            for item in items:
                w = tb.widgetForAction(item)
                total += (w.sizeHint().width() if w is not None else 8) + spacing
        if self.toolbar_hidden_sections:
            total += self.overflow_button.sizeHint().width() + spacing
        return total

    def update_toolbar_overflow(self):
        """Move whole groups into the ⋯ menu when the window is too narrow."""
        if not hasattr(self, "toolbar_sections"):
            return
        available = self.width()
        hidden = []
        self.toolbar_hidden_sections = hidden
        for name in self.TOOLBAR_COLLAPSE_ORDER:
            if self._toolbar_width_needed() <= available:
                break
            hidden.append(name)
        for name, items in self.toolbar_sections.items():
            for item in items:
                item.setVisible(name not in hidden)
        self.overflow_action.setVisible(bool(hidden))

    def _fill_overflow_menu(self):
        menu = self.overflow_menu
        menu.clear()
        entries = {
            "File": [
                self.new_action,
                self.open_action,
                self.save_action,
                ("Export", self.export_menu),
                self.import_action,
            ],
            "Slide": [
                self.add_action,
                self.dup_action,
                self.delete_action,
                self.undo_action,
                self.redo_action,
            ],
            "View": [
                self.focus_action,
                self.chrome_action,
                self.palette_action,
                ("About NIRUPRES / diagnostics", self.about),
            ],
        }
        for name in ("File", "Slide", "View"):
            if name not in self.toolbar_hidden_sections:
                continue
            menu.addSection(name.upper())
            for entry in entries[name]:
                if isinstance(entry, QAction):
                    menu.addAction(entry)
                elif isinstance(entry[1], QMenu):
                    sub = menu.addMenu(entry[0])
                    sub.addActions(entry[1].actions())
                else:
                    menu.addAction(entry[0], entry[1])
