"""NIRUPRES 2.0.0 — extracted application subsystem.

Refactored from the verified 1.7.3 baseline. Existing project and Markdown
format versions remain unchanged.
"""
import json, sys, zipfile, tempfile, shutil, uuid, re, stat, os, platform, hashlib
from collections import OrderedDict
from pathlib import Path
from PySide6.QtCore import Qt, QTimer, QSize, Signal, QRectF, QElapsedTimer, QEvent, QPropertyAnimation, QEasingCurve, QParallelAnimationGroup, QStandardPaths, QUrl, QPointF, QVariantAnimation
from PySide6.QtGui import QAction, QKeySequence, QShortcut, QPixmap, QPainter, QColor, QFont, QFontMetrics, QFontDatabase, QPdfWriter, QPageSize, QPageLayout, QImage, QImageReader, QIcon, QTextDocument, QAbstractTextDocumentLayout, QPainterPath
from PySide6.QtWidgets import (QApplication,QMainWindow,QWidget,QVBoxLayout,QHBoxLayout,QListWidget,QListWidgetItem,QLineEdit,QTextEdit,QComboBox,QPushButton,QLabel,QFileDialog,QSplitter,QMessageBox,QToolBar,QCheckBox,QDialog,QGridLayout,QDialogButtonBox,QGraphicsScene,QGraphicsPixmapItem,QGraphicsBlurEffect,QSlider,QGroupBox,QFormLayout,QMenu,QInputDialog,QGraphicsOpacityEffect)

from .foundation import *
from .foundation import _safe_zip_name, _validate_project_data, _cached_pixmap, _image_dimensions, _optimize_asset, _paired_font, _parse_two_column_body, _plain_markup, _inline_html, _format_markdown_selection, _IMAGE_CACHE, _IMAGE_CACHE_BYTES
class SlideList(QListWidget):
    reordered=Signal(int,int)
    def __init__(self):
        super().__init__(); self.setSelectionMode(QListWidget.ExtendedSelection); self.setDragDropMode(QListWidget.InternalMove); self.setDefaultDropAction(Qt.MoveAction)
    def dropEvent(self,e):
        old=self.currentRow(); super().dropEvent(e); new=self.currentRow()
        if old>=0 and new>=0 and old!=new:self.reordered.emit(old,new)

class LayoutPicker(QDialog):
    def __init__(self,parent=None):
        super().__init__(parent); self.choice=None; self.setWindowTitle('Choose layout'); self.setModal(True); self.setMinimumWidth(760)
        v=QVBoxLayout(self); h=QLabel('CHOOSE LAYOUT'); h.setObjectName('dialogtitle'); v.addWidget(h); g=QGridLayout(); v.addLayout(g)
        hints={'TITLE':'Opening / cover','STATEMENT':'One strong idea','TEXT':'Heading + narrative','BULLETS':'Focused key points','AGENDA':'Numbered agenda','IMAGE':'Image + caption','PHOTO':'Editorial photo','HERO IMAGE':'Image-led story','SPLIT':'Text + image','TWO COLUMN':'Two text columns','COMPARE':'A vs B / now vs next','IMAGE + QUOTE':'Image + quotation','QUOTE':'Editorial quotation','BIG NUMBER':'One dominant metric','DATA / KPI':'Primary + supporting KPIs','NUMBER GRID':'Up to four figures','PROCESS':'3–5 sequential steps','TIMELINE':'Milestones / sequence','MATRIX':'Four-part framework','TABLE':'Compact structured data','SECTION':'Chapter divider','FULL BLEED':'Immersive image','END':'Closing / contact'}
        for n,name in enumerate(LAYOUTS):
            b=QPushButton(f'{name}\n{hints[name]}'); b.setMinimumSize(168,76); b.clicked.connect(lambda _=False,x=name:self.pick(x)); g.addWidget(b,n//4,n%4)
        c=QPushButton('CANCEL'); c.clicked.connect(self.reject); v.addWidget(c,0,Qt.AlignRight)
    def pick(self,x): self.choice=x; self.accept()

class DeckSettings(QDialog):
    def __init__(self,parent,data):
        super().__init__(parent); self.data=data; self.setWindowTitle('Deck settings'); self.setMinimumWidth(430); f=QFormLayout(self)
        self.theme=QComboBox(); self.theme.addItems(THEMES.keys()); decorate_theme_combo(self.theme); self.theme.setCurrentText(normalize_theme(data.get('theme','B&W'))); f.addRow('Theme',self.theme)
        self.font=QComboBox(); self.font.addItems(QFontDatabase.families()); self.font.setCurrentText(data.get('fontFamily','Noto Sans')); f.addRow('Typeface',self.font)
        self.aspect=QComboBox(); self.aspect.addItems(['16:9','16:10','4:3','A4']); self.aspect.setCurrentText(data.get('aspect','16:9')); f.addRow('Format',self.aspect); self.transition=QComboBox(); self.transition.addItems(TRANSITIONS); self.transition.setCurrentText(normalize_transition(data.get('transition','NONE'))); self.transition.setToolTip('Audience transition · MORPH adds subtle depth · REVEAL is a restrained directional dissolve'); f.addRow('Transition',self.transition); self.reduce_motion=QCheckBox('Disable presentation motion without changing saved transitions'); self.reduce_motion.setChecked(data.get('reduceMotion',False)); f.addRow('Reduce motion',self.reduce_motion); self.cinematic_open=QCheckBox('Fade in first slide from black'); self.cinematic_open.setChecked(bool(data.get('cinematicOpen',True))); f.addRow('Opening',self.cinematic_open); self.cinematic_close=QCheckBox('Fade out to black when ending'); self.cinematic_close.setChecked(bool(data.get('cinematicClose',True))); f.addRow('Closing',self.cinematic_close); self.target_minutes=QSlider(Qt.Horizontal); self.target_minutes.setRange(0,120); self.target_minutes.setValue(int(data.get('targetMinutes',0))); self.target_minutes.setToolTip('0 = off · Presenter View shows pace against target duration'); self.target_label=QLabel('OFF' if self.target_minutes.value()==0 else f'{self.target_minutes.value()} min'); self.target_minutes.valueChanged.connect(lambda v:self.target_label.setText('OFF' if v==0 else f'{v} min')); tr=QWidget(); th=QHBoxLayout(tr); th.setContentsMargins(0,0,0,0); th.addWidget(self.target_minutes,1); th.addWidget(self.target_label); f.addRow('Target time',tr)
        self.numbers=QCheckBox('Show minimal slide numbers'); self.numbers.setChecked(data.get('showNumbers',False)); f.addRow('Slide numbers',self.numbers)
        self.logo=QCheckBox('Show official Uddevalla logo'); self.logo.setChecked(data.get('showLogo',True)); self.logo.setEnabled(THEMES.get(data.get('theme','B&W'),{}).get('uddevalla',False)); self.theme.currentTextChanged.connect(lambda th:self.logo.setEnabled(THEMES.get(th,{}).get('uddevalla',False))); f.addRow('Uddevalla logo',self.logo)
        self.footer=QLineEdit(data.get('footerText',''));self.footer.setPlaceholderText('Optional · e.g. Socialtjänsten · 2026-09-24');f.addRow('Footer',self.footer)
        self.footer_align=QComboBox(); self.footer_align.addItems(['LEFT','CENTER','RIGHT']); self.footer_align.setCurrentText(normalize_footer_align(data.get('footerAlign','LEFT'))); self.footer_align.setToolTip('Footer alignment across the deck'); f.addRow('Footer alignment',self.footer_align)
        self.footer_size=QComboBox(); self.footer_size.addItems(['SMALL','MEDIUM','LARGE']); self.footer_size.setCurrentText(normalize_footer_size(data.get('footerSize','MEDIUM'))); self.footer_size.setToolTip('Footer text size · all choices remain intentionally small'); f.addRow('Footer size',self.footer_size)
        files=QPushButton('FILE WORKFLOW…'); files.setToolTip('Default open/save folder and last-folder memory'); files.clicked.connect(parent.file_workflow_settings); f.addRow('Files',files)
        bb=neutralize_button_box(QDialogButtonBox(QDialogButtonBox.Save|QDialogButtonBox.Cancel)); bb.accepted.connect(self.accept);bb.rejected.connect(self.reject);f.addRow(bb)

class FileWorkflowSettings(QDialog):
    def __init__(self,parent,prefs):
        super().__init__(parent); self.prefs=dict(prefs); self.setWindowTitle('File workflow'); self.setMinimumWidth(620); f=QFormLayout(self)
        row=QWidget(); h=QHBoxLayout(row); h.setContentsMargins(0,0,0,0); h.setSpacing(8)
        self.folder=QLineEdit(self.prefs.get('defaultFolder','')); self.folder.setPlaceholderText(str(system_documents_dir())); h.addWidget(self.folder,1)
        choose=QPushButton('CHOOSE…'); h.addWidget(choose); f.addRow('Default folder',row)
        self.remember=QCheckBox('Remember the last folder used'); self.remember.setChecked(self.prefs.get('rememberLastFolder',True)); f.addRow('Open / save',self.remember)
        self.asset_quality=QComboBox(); self.asset_quality.addItems(['OPTIMIZED','HIGH','ORIGINAL']); self.asset_quality.setCurrentText(self.prefs.get('assetQuality','OPTIMIZED')); self.asset_quality.setToolTip('OPTIMIZED keeps presentation images sharp while reducing deck size · HIGH keeps extra 4K headroom · ORIGINAL embeds source bytes'); f.addRow('Embedded images',self.asset_quality)
        last=QLineEdit(self.prefs.get('lastFolder','')); last.setReadOnly(True); last.setPlaceholderText('No remembered folder yet'); f.addRow('Last folder',last)
        reset=QPushButton('RESET TO SYSTEM DOCUMENTS'); reset.setToolTip('Clear the custom default and remembered folder'); f.addRow('',reset)
        def pick():
            start=self.folder.text().strip() or str(preferred_folder(self.prefs,None,False)); path=QFileDialog.getExistingDirectory(self,'Choose default presentation folder',start)
            if path:self.folder.setText(path)
        choose.clicked.connect(pick); reset.clicked.connect(lambda:(self.folder.clear(),last.clear()))
        bb=neutralize_button_box(QDialogButtonBox(QDialogButtonBox.Save|QDialogButtonBox.Cancel)); bb.accepted.connect(self.accept); bb.rejected.connect(self.reject); f.addRow(bb)
    def values(self):
        folder=self.folder.text().strip(); folder=str(Path(folder).expanduser().resolve()) if folder and Path(folder).expanduser().is_dir() else ''
        out=dict(self.prefs); out['defaultFolder']=folder; out['rememberLastFolder']=self.remember.isChecked(); out['assetQuality']=self.asset_quality.currentText()
        if not out['rememberLastFolder']: out['lastFolder']=''
        return out

class TextEditor(QDialog):
    """Layout-aware plain-text editor.

    Structured layouts should expose their structure in the GUI rather than
    forcing users to remember serialization delimiters.  The deck format stays
    plain text: TWO COLUMN and COMPARE are composed back to `|||` on save.
    """
    def __init__(self,parent,title,body,layout='',structured_left=None,structured_right=None):
        super().__init__(parent); self.layout=str(layout or '').upper(); self.setWindowTitle('Edit slide text'); self.resize(760 if self.layout in ('TWO COLUMN','COMPARE') else 620,460)
        v=QVBoxLayout(self); v.addWidget(QLabel('TITLE / STATEMENT / NUMBER')); self.title=QLineEdit(title); v.addWidget(self.title)
        fmt=QHBoxLayout(); fmt.addWidget(QLabel('FORMAT'))
        self.bold_btn=QPushButton('B'); self.bold_btn.setFixedWidth(36); self.bold_btn.setToolTip('Bold · Ctrl+B · Markdown **text**')
        self.italic_btn=QPushButton('I'); self.italic_btn.setFixedWidth(36); self.italic_btn.setToolTip('Italic · Ctrl+I · Markdown *text*')
        self.normal_btn=QPushButton('N'); self.normal_btn.setFixedWidth(36); self.normal_btn.setToolTip('Normal · remove Markdown emphasis · Ctrl+0')
        for button in (self.bold_btn,self.italic_btn,self.normal_btn):
            button.setFocusPolicy(Qt.NoFocus)
        fmt.addWidget(self.bold_btn); fmt.addWidget(self.italic_btn); fmt.addWidget(self.normal_btn); fmt.addStretch(1); v.addLayout(fmt)
        self.bold_btn.clicked.connect(lambda:self.apply_format('bold')); self.italic_btn.clicked.connect(lambda:self.apply_format('italic')); self.normal_btn.clicked.connect(lambda:self.apply_format('normal'))
        QShortcut(QKeySequence('Ctrl+B'),self,activated=lambda:self.apply_format('bold')); QShortcut(QKeySequence('Ctrl+I'),self,activated=lambda:self.apply_format('italic')); QShortcut(QKeySequence('Ctrl+0'),self,activated=lambda:self.apply_format('normal'))
        self.body=None; self.left_body=None; self.right_body=None; self._last_format_target=self.title
        if self.layout in ('TWO COLUMN','COMPARE'):
            v.addWidget(QLabel('CONTENT · edit each side directly — no separator syntax needed'))
            row=QHBoxLayout(); left_wrap=QWidget(); lv=QVBoxLayout(left_wrap); lv.setContentsMargins(0,0,0,0)
            right_wrap=QWidget(); rv=QVBoxLayout(right_wrap); rv.setContentsMargins(0,0,0,0)
            lv.addWidget(QLabel('LEFT SIDE'))
            rv.addWidget(QLabel('RIGHT SIDE'))
            if structured_left is not None and structured_right is not None:
                left=str(structured_left); right=str(structured_right)
            else:
                cols,_=_parse_two_column_body(body); left=cols[0]['text']; right=cols[1]['text']
            helper='Blank lines add spacing · **bold** and *italic* supported · rendered exactly as written · no automatic headings'
            self.left_body=QTextEdit(); self.left_body.setAcceptRichText(False); self.left_body.setPlainText(left)
            self.right_body=QTextEdit(); self.right_body.setAcceptRichText(False); self.right_body.setPlainText(right)
            lv.addWidget(self.left_body,1); rv.addWidget(self.right_body,1); row.addWidget(left_wrap,1); row.addWidget(right_wrap,1); v.addLayout(row,1)
            hint=QLabel(helper); hint.setObjectName('editorHint'); v.addWidget(hint)
        else:
            v.addWidget(QLabel('BODY / SUBTITLE / ATTRIBUTION · **bold** · *italic*'))
            hints={
                'PROCESS':'One step per line: HEADING | description',
                'TIMELINE':'One milestone per line: MILESTONE | description',
                'NUMBER GRID':'One figure per line: VALUE | label · up to four',
                'DATA / KPI':'One KPI per line: VALUE | label',
                'MATRIX':'One quadrant per line: HEADING | description · up to four',
                'TABLE':'One row per line · separate columns with |',
                'BULLETS':'One list item per line', 'AGENDA':'One agenda item per line'
            }
            if self.layout in hints:
                hint=QLabel(hints[self.layout]); hint.setObjectName('editorHint'); v.addWidget(hint)
            self.body=QTextEdit(); self.body.setAcceptRichText(False); self.body.setPlainText(body); v.addWidget(self.body,1)
        for editor in (self.title,self.body,self.left_body,self.right_body):
            if editor is not None: editor.installEventFilter(self)
        bb=neutralize_button_box(QDialogButtonBox(QDialogButtonBox.Save|QDialogButtonBox.Cancel)); bb.accepted.connect(self.accept); bb.rejected.connect(self.reject); v.addWidget(bb); self.title.setFocus()

    def eventFilter(self,obj,event):
        if event.type()==QEvent.FocusIn and obj in (self.title,self.body,self.left_body,self.right_body):
            self._last_format_target=obj
        return super().eventFilter(obj,event)

    def _format_target(self):
        # Formatting controls deliberately use Qt.NoFocus.  Normally the last
        # focused editor is therefore authoritative.  A selected field is also
        # accepted as a fallback; this makes formatting deterministic for
        # keyboard/programmatic selection and avoids silently targeting TITLE
        # when BODY/RIGHT SIDE owns the actual selection.
        candidates=[self.title,self.body,self.left_body,self.right_body]
        w=QApplication.focusWidget()
        if w in candidates and w is not None:
            self._last_format_target=w
        target=self._last_format_target
        def has_selection(editor):
            if isinstance(editor,QLineEdit): return editor.hasSelectedText()
            if isinstance(editor,QTextEdit): return editor.textCursor().hasSelection()
            return False
        if target is not None and has_selection(target):
            return target
        selected=[editor for editor in candidates if editor is not None and has_selection(editor)]
        if len(selected)==1:
            self._last_format_target=selected[0]
            return selected[0]
        return target or self.body or self.left_body or self.title
    def apply_format(self,kind):
        w=self._format_target()
        if isinstance(w,QLineEdit):
            start=w.selectionStart(); text=w.selectedText()
            if start<0 or not text:return
            replacement=_format_markdown_selection(text,kind)
            w.setSelection(start,len(text)); w.insert(replacement); w.setSelection(start,len(replacement)); return
        if isinstance(w,QTextEdit):
            c=w.textCursor(); text=c.selectedText().replace('\u2029','\n')
            if not text:return
            replacement=_format_markdown_selection(text,kind)
            c.insertText(replacement)

    def body_text(self):
        if self.left_body is not None:
            left=self.left_body.toPlainText().strip(); right=self.right_body.toPlainText().strip()
            # `|||` is serialization only. It never needs to be typed in the GUI.
            return left + ('\n\n|||\n\n' if left or right else '') + right
        return self.body.toPlainText() if self.body is not None else ''

