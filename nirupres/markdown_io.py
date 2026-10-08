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
def _md_bool(v): return str(v).strip().lower() in ('1','true','yes','on')
def _meta_block(text, kind):
    m=re.search(r'<!--\s*'+re.escape(kind)+r'\s*\n(.*?)-->',text,re.S|re.I)
    if not m:return {}
    out={}
    for line in m.group(1).splitlines():
        if ':' in line:
            k,v=line.split(':',1);out[k.strip().lower()]=v.strip().strip('"').strip("'")
    return out

def export_nirupres_markdown(data):
    lines=['<!-- deck',f'nirupres: {MD_FORMAT}',f'title: {data.get("title","Untitled")}',f'theme: {data.get("theme","B&W")}',f'font: {data.get("fontFamily","Noto Sans")}',f'ratio: {data.get("aspect","16:9")}',f'numbers: {str(bool(data.get("showNumbers",False))).lower()}',f'logo: {str(bool(data.get("showLogo",True))).lower()}',f'footer: {data.get("footerText","")}',f'footer-align: {normalize_footer_align(data.get("footerAlign","LEFT"))}',f'footer-size: {normalize_footer_size(data.get("footerSize","MEDIUM"))}',f'transition: {normalize_transition(data.get("transition","NONE"))}',f'reduce-motion: {str(bool(data.get("reduceMotion",False))).lower()}',f'cinematic-open: {str(bool(data.get("cinematicOpen",True))).lower()}',f'cinematic-close: {str(bool(data.get("cinematicClose",True))).lower()}','-->','']
    for n,s in enumerate(data.get('slides',[])):
        lines += ['<!-- slide',f'layout: {s.get("layout","TITLE")}',f'theme: {s.get("themeOverride","DECK")}',f'font: {s.get("fontFamily","DECK")}',f'hidden: {str(bool(s.get("hidden",False))).lower()}',f'image-mode: {s.get("imageMode","FILL")}',f'mono: {str(bool(s.get("mono",False))).lower()}',f'image-treatment: {s.get("imageTreatment","MONO" if s.get("mono") else "NATURAL")}',f'focal-x: {s.get("focalX",.5)}',f'focal-y: {s.get("focalY",.5)}',f'align: {s.get("align","LEFT")}',f'weight: {s.get("weight","AUTO")}',f'font-scale: {s.get("fontScale",1.0)}',f'brightness: {s.get("brightness",0)}',f'contrast: {s.get("contrast",0)}',f'overlay: {s.get("overlay",0)}',f'blur: {s.get("blur",0)}',f'list-style: {s.get("listStyle","NUMBERS")}',f'background-dim: {s.get("backgroundDim",75)}',f'background-blur: {s.get("backgroundBlur",0)}',f'background-mono: {str(bool(s.get("backgroundMono",False))).lower()}',f'background-focal-x: {s.get("backgroundFocalX",.5)}',f'background-focal-y: {s.get("backgroundFocalY",.5)}',f'background-motion: {s.get("backgroundMotion","NONE")}',f'logo: {s.get("logoMode","AUTO")}',f'accent: {s.get("accentOverride","AUTO")}',f'transition: {normalize_transition(s.get("transition","DECK"),True)}',f'video-autoplay: {str(bool(s.get("videoAutoplay",False))).lower()}',f'video-muted: {str(bool(s.get("videoMuted",False))).lower()}']
        if s.get('image'): lines.append(f'image: {s.get("image")}')
        if s.get('backgroundImage'): lines.append(f'background-image: {s.get("backgroundImage")}')
        if s.get('videoSource'): lines.append(f'video: {s.get("videoSource")}')
        if s.get('videoPoster'): lines.append(f'video-poster: {s.get("videoPoster")}')
        if s.get('imageRequest'): lines.append(f'image-query: {s.get("imageRequest")}')
        if s.get('caption'): lines.append(f'caption: {s.get("caption")}')
        lines += ['-->','',f'# {s.get("title","")}'.rstrip(),'']
        if s.get('body'):lines += [s.get('body',''),'']
        if s.get('notes'): lines += ['<!-- notes',s.get('notes',''),'-->','']
        if n < len(data.get('slides',[]))-1: lines += ['---','']
    return '\n'.join(lines).rstrip()+'\n'

def import_nirupres_markdown(text, base_dir=None):
    deck=_meta_block(text,'deck'); data=clone(DEFAULT); data['slides']=[]
    data['title']=deck.get('title','Untitled'); th=normalize_theme(deck.get('theme','B&W')); data['theme']=th if th in THEMES else 'B&W'; data['fontFamily']=deck.get('font','Noto Sans'); data['aspect']=deck.get('ratio','16:9') if deck.get('ratio','16:9') in ('16:9','16:10','4:3','A4') else '16:9'; data['showNumbers']=_md_bool(deck.get('numbers','false')); data['showLogo']=_md_bool(deck.get('logo','true')); data['footerText']=deck.get('footer',''); data['footerAlign']=normalize_footer_align(deck.get('footer-align','LEFT')); data['footerSize']=normalize_footer_size(deck.get('footer-size','MEDIUM')); data['transition']=normalize_transition(deck.get('transition','NONE')); data['reduceMotion']=_md_bool(deck.get('reduce-motion','false')); data['cinematicOpen']=_md_bool(deck.get('cinematic-open','true')); data['cinematicClose']=_md_bool(deck.get('cinematic-close','true'))
    parts=re.split(r'^---\s*$',text,flags=re.M)
    for part in parts:
        meta=_meta_block(part,'slide')
        if not meta:continue
        layout=meta.get('layout','TITLE').upper(); layout=layout if layout in LAYOUTS else 'TEXT'
        clean=re.sub(r'<!--.*?-->','',part,flags=re.S).strip(); title=''; body=''
        ls=clean.splitlines()
        if ls and ls[0].lstrip().startswith('# '): title=ls[0].lstrip()[2:].strip(); body='\n'.join(ls[1:]).strip()
        else: body=clean
        img=meta.get('image','')
        if img and re.match(r'^[a-zA-Z]+://',img): img=''
        elif img and base_dir and not Path(img).is_absolute(): img=str((Path(base_dir)/img).resolve())
        bgimg=meta.get('background-image','')
        if bgimg and re.match(r'^[a-zA-Z]+://',bgimg): bgimg=''
        elif bgimg and base_dir and not Path(bgimg).is_absolute(): bgimg=str((Path(base_dir)/bgimg).resolve())
        def num(k,d):
            try:return float(meta.get(k,d))
            except:return d
        imode=meta.get('image-mode','FILL').upper(); imode=imode if imode in ('FIT','FILL') else 'FILL'
        align=meta.get('align','LEFT').upper(); align=align if align in ('LEFT','CENTER','RIGHT') else 'LEFT'
        weight=meta.get('weight','AUTO').upper(); weight=weight if weight in ('AUTO','LIGHT','REGULAR','BOLD') else 'AUTO'
        s=slide_defaults({'layout':layout,'title':title,'body':body,'themeOverride':normalize_theme(meta.get('theme','DECK')),'fontFamily':meta.get('font','DECK'),'hidden':_md_bool(meta.get('hidden','false')),'image':img,'imageMode':imode,'mono':_md_bool(meta.get('mono','false')),'imageTreatment':meta.get('image-treatment','MONO' if _md_bool(meta.get('mono','false')) else 'NATURAL').upper(),'listStyle':meta.get('list-style','NUMBERS').upper(),'backgroundImage':bgimg,'backgroundDim':max(0,min(90,int(num('background-dim',75)))),'backgroundBlur':max(0,min(20,int(num('background-blur',0)))),'backgroundMono':_md_bool(meta.get('background-mono','false')),'backgroundFocalX':max(0,min(1,num('background-focal-x',.5))),'backgroundFocalY':max(0,min(1,num('background-focal-y',.5))),'backgroundMotion':meta.get('background-motion','NONE').upper(),'focalX':max(0,min(1,num('focal-x',.5))),'focalY':max(0,min(1,num('focal-y',.5))),'align':align,'weight':weight,'fontScale':max(.5,min(2,num('font-scale',1.0))),'brightness':max(-30,min(30,int(num('brightness',0)))),'contrast':max(0,min(50,int(num('contrast',0)))),'overlay':max(0,min(60,int(num('overlay',0)))),'blur':max(0,min(10,int(num('blur',0)))),'caption':meta.get('caption',''),'imageRequest':meta.get('image-query',''),'logoMode':meta.get('logo','AUTO').upper(),'accentOverride':meta.get('accent','AUTO').upper(),'transition':normalize_transition(meta.get('transition','DECK'),True),'videoSource':meta.get('video',''),'videoPoster':meta.get('video-poster',''),'videoAutoplay':_md_bool(meta.get('video-autoplay','false')),'videoMuted':_md_bool(meta.get('video-muted','false'))})
        vsrc=str(s.get('videoSource','') or '')
        if vsrc and not re.match(r'^[a-zA-Z]+://',vsrc) and base_dir and not Path(vsrc).is_absolute(): s['videoSource']=str((Path(base_dir)/vsrc).resolve())
        vposter=str(s.get('videoPoster','') or '')
        if vposter and not re.match(r'^[a-zA-Z]+://',vposter) and base_dir and not Path(vposter).is_absolute(): s['videoPoster']=str((Path(base_dir)/vposter).resolve())
        nm=re.search(r'<!--\s*notes\s*\n(.*?)-->',part,re.S|re.I); s['notes']=nm.group(1).strip() if nm else ''
        if s['themeOverride']!='DECK' and s['themeOverride'] not in THEMES:s['themeOverride']='DECK'
        data['slides'].append(s)
    if not data['slides']: raise ValueError('No valid <!-- slide ... --> blocks found.')
    data['format']=FORMAT;data['appVersion']=VERSION;return data

def import_nirupres_markdown_report(text, base_dir=None):
    report=[]
    deck=_meta_block(text,'deck')
    raw_theme=deck.get('theme','B&W'); raw_ratio=deck.get('ratio','16:9'); raw_trans=deck.get('transition','NONE'); raw_footer_align=deck.get('footer-align','LEFT'); raw_footer_size=deck.get('footer-size','MEDIUM')
    data=import_nirupres_markdown(text,base_dir)
    if normalize_theme(raw_theme) not in THEMES: report.append(f'Unknown deck theme {raw_theme!r} → B&W')
    if raw_ratio not in ('16:9','16:10','4:3','A4'): report.append(f'Unknown ratio {raw_ratio!r} → 16:9')
    if str(raw_trans).upper() not in ('NONE','FADE','MORPH','REVEAL','CUT'): report.append(f'Unknown transition {raw_trans!r} → NONE')
    if str(raw_footer_align).upper() not in ('LEFT','CENTER','RIGHT'): report.append(f'Unknown footer alignment {raw_footer_align!r} → LEFT')
    if str(raw_footer_size).upper() not in ('SMALL','MEDIUM','LARGE'): report.append(f'Unknown footer size {raw_footer_size!r} → MEDIUM')
    parts=[x for x in re.split(r'^---\s*$',text,flags=re.M) if _meta_block(x,'slide')]
    for idx,part in enumerate(parts,1):
        meta=_meta_block(part,'slide'); layout=meta.get('layout','TITLE').upper()
        if layout not in LAYOUTS: report.append(f'Slide {idx}: unknown layout {layout!r} → TEXT')
        if meta.get('logo','AUTO').upper() not in ('AUTO','ON','OFF'): report.append(f'Slide {idx}: invalid logo mode → AUTO')
        if meta.get('accent','AUTO').upper() not in ('AUTO',*UDDEVALLA_ACCENTS.keys()): report.append(f'Slide {idx}: invalid accent → AUTO')
        if meta.get('transition','DECK').upper() not in ('DECK','NONE','FADE','MORPH','REVEAL','CUT'): report.append(f'Slide {idx}: invalid transition → DECK')
        if re.match(r'^[a-zA-Z]+://',meta.get('image','')): report.append(f'Slide {idx}: remote image URL removed; use image-query instead')
        if meta.get('image-mode','FILL').upper() not in ('FIT','FILL'): report.append(f'Slide {idx}: invalid image mode → FILL')
        if meta.get('align','LEFT').upper() not in ('LEFT','CENTER','RIGHT'): report.append(f'Slide {idx}: invalid alignment → LEFT')
        if meta.get('weight','AUTO').upper() not in ('AUTO','LIGHT','REGULAR','BOLD'): report.append(f'Slide {idx}: invalid font weight → AUTO')
    return data,report

AI_TEMPLATE='''# NIRUPRES AI PRESENTATION BRIEF
nirupres-spec: 2
target-format: NIRUPRES Markdown

## INSTRUCTIONS FOR AI
Return ONLY valid NIRUPRES Markdown. Do not wrap the result in a Markdown code fence. Preserve supported metadata names exactly. Do not invent layouts, themes, local file paths, URLs, scripts, HTML, or executable content. Prefer concise presentation language, one main message per slide, and speaker notes for supporting detail. Use `image-query:` when an image would materially improve a slide.

## NIRUPRES MARKDOWN v2
Deck metadata is a `<!-- deck` block. Every slide starts with a `<!-- slide` block. Slides are separated by a line containing exactly `---`.

Allowed layouts: TITLE, STATEMENT, TEXT, BULLETS, AGENDA, IMAGE, PHOTO, HERO IMAGE, SPLIT, TWO COLUMN, COMPARE, IMAGE + QUOTE, QUOTE, BIG NUMBER, DATA / KPI, NUMBER GRID, PROCESS, TIMELINE, MATRIX, TABLE, SECTION, VIDEO, VIDEO + TEXT, FULL BLEED, END.
Allowed themes: B&W, W&B, GRAY, REDWINE, SATIE, CARL LARSSON, BEKSINSKI, MARTIN, SFUMATO, UDDEVALLA BLUE, UDDEVALLA DARK, UDDEVALLA BLACK, UDDEVALLA GREEN, UDDEVALLA YELLOW, UDDEVALLA RED, UDDEVALLA PINK, UDDEVALLA PURPLE.
Allowed footer alignment: LEFT, CENTER, RIGHT. Allowed footer size: SMALL, MEDIUM, LARGE.
Allowed deck transitions: NONE, FADE, MORPH, REVEAL. Per-slide transition: DECK, NONE, FADE, MORPH, REVEAL. Optional deck `reduce-motion: true` suppresses motion at runtime without deleting transition choices. `cinematic-open` and `cinematic-close` control whole-show black fades.
Allowed logo values per slide: AUTO, ON, OFF. Allowed accents: AUTO, BLUE, GREEN, YELLOW, RED, PINK, PURPLE.
Inline emphasis: `**bold**` and `*italic*` are preserved and rendered. Backgrounds may use `background-motion: KEN BURNS`. VIDEO/VIDEO + TEXT use `video:` plus optional `video-poster:`. BULLETS: one point per body line. TWO COLUMN: use `|||` on its own line between literal left/right columns; no line is a heading and blank lines are preserved as spacing. NUMBER GRID: `value | label` per line, max 4. TIMELINE: `milestone | description` per line, max 5. Notes go in `<!-- notes` blocks and are private.

<!-- deck
nirupres: 1
title: Presentation title
theme: B&W
font: Noto Sans
ratio: 16:9
numbers: false
logo: true
footer:
footer-align: LEFT
footer-size: MEDIUM
transition: NONE
-->

<!-- slide
layout: TITLE
theme: DECK
font: DECK
logo: AUTO
accent: AUTO
transition: DECK
-->

# Presentation title

Short subtitle

---

<!-- slide
layout: STATEMENT
theme: DECK
image-query: editorial photo describing the subject
transition: DECK
-->

# One strong idea

A short supporting sentence.

<!-- notes
Private speaker notes.
-->

---

<!-- slide
layout: END
theme: DECK
transition: DECK
-->

# Thank you
'''

def export_ai_brief(data):
    return AI_TEMPLATE + '\n\n## CURRENT PRESENTATION\n\n' + export_nirupres_markdown(data)


