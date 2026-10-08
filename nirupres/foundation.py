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

APP='NIRUPRES'; VERSION='2.0.0'; FORMAT=9; MD_FORMAT=2
TAGLINE='A minimal, suckless presentation app by Nicklas Rudolfsson.'
MB=1024*1024
MAX_PROJECT_JSON=5*MB
MAX_ASSETS=250
MAX_PACKAGE_UNPACKED=500*MB
MAX_IMAGE_PIXELS=50_000_000
IMAGE_CACHE_BUDGET=192*MB
MAX_HISTORY_BYTES=24*MB
EXPORT_PROFILES={'preview':(1920,1080),'pptx':(2560,1440)}
_IMAGE_CACHE=OrderedDict(); _IMAGE_CACHE_BYTES=0
DATA=Path.home()/'.local/share/nirupres'; AUTOSAVE=DATA/'autosave.json'; RECENTS=DATA/'recent.json'; DISPLAY_PREFS=DATA/'display.json'
ASSETS=Path(__file__).resolve().parent/'assets'; BRANDING=ASSETS/'branding'; FILE_PREFS=DATA/'file-workflow.json'
THEMES={
 'B&W': {'bg':'#050505','fg':'#F4F2ED','muted':'#969696','accent':'#F4F2ED','rule':'#2A2A2A','imageMono':True},
 'W&B': {'bg':'#F4F2ED','fg':'#090909','muted':'#66635F','accent':'#090909','rule':'#D4D0C8','imageMono':True},
 'GRAY': {'bg':'#242424','fg':'#ECECEC','muted':'#A8A8A8','accent':'#CFCFCF','rule':'#505050','imageMono':True},
 'REDWINE': {'bg':'#210B11','fg':'#F1E8E3','muted':'#B89DA4','accent':'#8F263B','rule':'#54202C','imageMono':False},
 'SATIE': {'bg':'#EDE7D8','fg':'#191815','muted':'#6F695F','accent':'#2E2923','rule':'#C7BEAC','imageMono':True},
 'CARL LARSSON': {'bg':'#EFE1BF','fg':'#26372F','muted':'#78664F','accent':'#A33E32','rule':'#B8A47B','imageMono':False,'palette':['#A33E32','#557A68','#66859A','#C39A45']},
 'BEKSINSKI': {'bg':'#0A0807','fg':'#E9E0D2','muted':'#8F8274','accent':'#7B3028','rule':'#30251F','imageMono':False},
 'MARTIN': {'bg':'#0A0D0E','fg':'#EEE8D9','muted':'#92938D','accent':'#B78C4A','rule':'#293034','imageMono':False},
 'SFUMATO': {'bg':'#171713','fg':'#E8E3D5','muted':'#9C988C','accent':'#8A8067','rule':'#37362F','imageMono':False},
 # Uddevalla family. Each theme uses one official profile colour consistently.
 # Logo artwork itself is never recoloured; the renderer chooses the supplied
 # light/dark logo asset from the actual theme background luminance.
 'UDDEVALLA DARK':   {'bg':'#07141D','fg':'#ECEDED','muted':'#A9BAC4','accent':'#0065A6','rule':'#24404F','imageMono':False,'uddevalla':True},
 'UDDEVALLA BLACK':  {'bg':'#000000','fg':'#FFFFFF','muted':'#B8C0C5','accent':'#0065A6','rule':'#252525','imageMono':False,'uddevalla':True},
 'UDDEVALLA BLUE':   {'bg':'#F8FAFB','fg':'#173042','muted':'#526774','accent':'#0065A6','rule':'#D8E2E8','imageMono':False,'uddevalla':True},
 'UDDEVALLA GREEN':  {'bg':'#F8FAFB','fg':'#173042','muted':'#526774','accent':'#79AC54','rule':'#DCE7D5','imageMono':False,'uddevalla':True},
 'UDDEVALLA YELLOW': {'bg':'#F8FAFB','fg':'#173042','muted':'#526774','accent':'#FFCC00','rule':'#EEE5BF','imageMono':False,'uddevalla':True},
 'UDDEVALLA RED':    {'bg':'#F8FAFB','fg':'#173042','muted':'#526774','accent':'#B51726','rule':'#E8D5D8','imageMono':False,'uddevalla':True},
 'UDDEVALLA PINK':   {'bg':'#F8FAFB','fg':'#173042','muted':'#526774','accent':'#E0437C','rule':'#EED8E1','imageMono':False,'uddevalla':True},
 'UDDEVALLA PURPLE': {'bg':'#F8FAFB','fg':'#173042','muted':'#526774','accent':'#672A61','rule':'#E0D5DF','imageMono':False,'uddevalla':True},
}
LAYOUTS=['TITLE','STATEMENT','TEXT','BULLETS','AGENDA','IMAGE','PHOTO','HERO IMAGE','SPLIT','TWO COLUMN','COMPARE','IMAGE + QUOTE','QUOTE','BIG NUMBER','DATA / KPI','NUMBER GRID','PROCESS','TIMELINE','MATRIX','TABLE','SECTION','VIDEO','VIDEO + TEXT','FULL BLEED','END']
UDDEVALLA_ACCENTS={'BLUE':'#0065A6','GREEN':'#79AC54','YELLOW':'#FFCC00','RED':'#B51726','PINK':'#E0437C','PURPLE':'#672A61'}
DEFAULT={"format":FORMAT,"appVersion":VERSION,"title":"Untitled","theme":"B&W","showNumbers":False,"showLogo":True,"fontFamily":"Noto Sans","aspect":"16:9","transition":"NONE","reduceMotion":False,"cinematicOpen":True,"cinematicClose":True,"targetMinutes":0,"footerText":"","footerAlign":"LEFT","footerSize":"MEDIUM","slides":[{"layout":"TITLE","title":"NIRUPRES","body":"MINIMAL NOIR PRESENTATIONS","image":"","imageMode":"FILL","mono":False,"notes":"","focalX":0.5,"focalY":0.5,"imageTreatment":"NATURAL"}]}

def clone(x): return json.loads(json.dumps(x))

def system_documents_dir():
    p=QStandardPaths.writableLocation(QStandardPaths.StandardLocation.DocumentsLocation)
    candidate=Path(p).expanduser() if p else Path.home()/'Documents'
    return candidate if candidate.exists() else Path.home()

def load_file_prefs():
    defaults={'defaultFolder':'','rememberLastFolder':True,'lastFolder':'','lastFilter':'','assetQuality':'OPTIMIZED'}
    try:
        raw=json.loads(FILE_PREFS.read_text(encoding='utf-8')) if FILE_PREFS.exists() else {}
        if isinstance(raw,dict): defaults.update({k:raw.get(k,defaults[k]) for k in defaults})
    except Exception: pass
    defaults['rememberLastFolder']=bool(defaults.get('rememberLastFolder',True))
    defaults['assetQuality']=str(defaults.get('assetQuality','OPTIMIZED')).upper()
    if defaults['assetQuality'] not in ('OPTIMIZED','HIGH','ORIGINAL'): defaults['assetQuality']='OPTIMIZED'
    for key in ('defaultFolder','lastFolder'):
        value=str(defaults.get(key,'') or '')
        defaults[key]=value if value and Path(value).expanduser().is_dir() else ''
    return defaults

def save_file_prefs(prefs):
    DATA.mkdir(parents=True,exist_ok=True)
    payload={k:prefs.get(k) for k in ('defaultFolder','rememberLastFolder','lastFolder','lastFilter','assetQuality')}
    tmp=FILE_PREFS.with_suffix('.tmp'); tmp.write_text(json.dumps(payload,ensure_ascii=False,indent=2),encoding='utf-8'); tmp.replace(FILE_PREFS)

def preferred_folder(prefs, current_path=None, prefer_current=True):
    if prefer_current and current_path:
        try:
            parent=Path(current_path).expanduser().resolve().parent
            if parent.is_dir(): return parent
        except Exception: pass
    if prefs.get('rememberLastFolder') and prefs.get('lastFolder'):
        p=Path(prefs['lastFolder']).expanduser()
        if p.is_dir(): return p
    if prefs.get('defaultFolder'):
        p=Path(prefs['defaultFolder']).expanduser()
        if p.is_dir(): return p
    return system_documents_dir()

def remember_file_folder(prefs,path,selected_filter=''):
    try:
        p=Path(path).expanduser()
        folder=p if p.is_dir() else p.parent
        if prefs.get('rememberLastFolder',True) and folder.is_dir(): prefs['lastFolder']=str(folder.resolve())
        if selected_filter: prefs['lastFilter']=selected_filter
        save_file_prefs(prefs)
    except Exception: pass


TRANSITIONS=('NONE','FADE','MORPH','REVEAL')
def normalize_transition(value, allow_deck=False):
    value=str(value or ('DECK' if allow_deck else 'NONE')).strip().upper()
    if value=='CUT': value='NONE'
    allowed=(('DECK',)+TRANSITIONS) if allow_deck else TRANSITIONS
    return value if value in allowed else ('DECK' if allow_deck else 'NONE')
def slide_defaults(s=None):
    d={'id':str(uuid.uuid4()),'layout':'TITLE','title':'','body':'','image':'','imageMode':'FILL','mono':False,'notes':'','focalX':0.5,'focalY':0.5,'imageTreatment':'NATURAL','listStyle':'NUMBERS','backgroundImage':'','backgroundDim':75,'backgroundBlur':0,'backgroundMono':False,'backgroundFocalX':0.5,'backgroundFocalY':0.5,'themeOverride':'DECK','fontScale':1.0,'align':'LEFT','weight':'AUTO','brightness':0,'contrast':0,'overlay':0,'blur':0,'fontFamily':'DECK','hidden':False,'caption':'','imageRequest':'','logoMode':'AUTO','accentOverride':'AUTO','transition':'DECK','leftColumn':None,'rightColumn':None,'twoColumnStructured':False,'sideStructured':False,'backgroundMotion':'NONE','videoSource':'','videoPoster':'','videoAutoplay':False,'videoMuted':False}; d.update(s or {}); d['imageTreatment']=str(('MONO' if d.get('mono') else 'NATURAL') if (s is not None and 'imageTreatment' not in s) else d.get('imageTreatment','NATURAL')).upper(); d['imageTreatment']=d['imageTreatment'] if d['imageTreatment'] in ('NATURAL','MONO','DIM','CONTRAST') else 'NATURAL'; d['mono']=d['imageTreatment']=='MONO'; d['listStyle']=str(d.get('listStyle','NUMBERS')).upper() if str(d.get('listStyle','NUMBERS')).upper() in ('NUMBERS','DOTS','DASHES','NONE') else 'NUMBERS'; d['backgroundDim']=max(0,min(90,int(d.get('backgroundDim',75) or 0))); d['backgroundBlur']=max(0,min(20,int(d.get('backgroundBlur',0) or 0))); d['backgroundMono']=bool(d.get('backgroundMono',False)); d['backgroundFocalX']=max(0.0,min(1.0,float(d.get('backgroundFocalX',.5) or .5))); d['backgroundFocalY']=max(0.0,min(1.0,float(d.get('backgroundFocalY',.5) or .5))); d['transition']=normalize_transition(d.get('transition','DECK'),True); d['backgroundMotion']=str(d.get('backgroundMotion','NONE')).upper() if str(d.get('backgroundMotion','NONE')).upper() in ('NONE','KEN BURNS') else 'NONE'; d['logoMode']=str(d.get('logoMode','AUTO')).upper() if str(d.get('logoMode','AUTO')).upper() in ('AUTO','ON','OFF') else 'AUTO'; d['accentOverride']=str(d.get('accentOverride','AUTO')).upper() if str(d.get('accentOverride','AUTO')).upper() in ('AUTO',*UDDEVALLA_ACCENTS.keys()) else 'AUTO'; return d

def normalize_footer_align(value):
    value=str(value or 'LEFT').strip().upper()
    return value if value in ('LEFT','CENTER','RIGHT') else 'LEFT'

def normalize_footer_size(value):
    value=str(value or 'MEDIUM').strip().upper()
    return value if value in ('SMALL','MEDIUM','LARGE') else 'MEDIUM'

def normalize_theme(name):
    # Compatibility aliases stay readable while duplicate themes stay out of the UI.
    name=str(name or 'B&W').upper()
    if name in ('UDDEVALLA','UDDEVALLA LIGHT'): return 'UDDEVALLA BLUE'
    if name=='CL': return 'CARL LARSSON'
    return name

def normalize_themes(data):
    data['theme']=normalize_theme(data.get('theme','B&W'))
    if data['theme'] not in THEMES: data['theme']='B&W'
    for slide in data.get('slides',[]):
        ov=normalize_theme(slide.get('themeOverride','DECK'))
        slide['themeOverride']=ov if ov=='DECK' or ov in THEMES else 'DECK'
    return data

def decorate_theme_combo(combo):
    """Keep editor chrome monochrome; theme colour belongs to slide content only."""
    # Compatibility no-op: callers from older code paths may still invoke this.
    return combo

def _fit_dialog_button(button):
    """Keep monochrome dialog actions readable at any label/font scale."""
    button.setIcon(QIcon())
    # QSS previously imposed only a 90 px minimum. Qt standard labels such as
    # "Close without Saving" can be substantially wider, especially with
    # desktop font scaling. Size from the active font instead of hard-coding.
    text_width = button.fontMetrics().horizontalAdvance(button.text().replace('&', ''))
    button.setMinimumWidth(max(112, text_width + 36))
    button.setMinimumHeight(max(34, button.sizeHint().height()))
    return button

def neutralize_button_box(box):
    """Remove platform icons and guarantee unclipped monochrome actions."""
    for button in box.buttons():
        _fit_dialog_button(button)
    return box

def make_message_box(parent, title, text, buttons=QMessageBox.Ok, default=None):
    """Create a NIRUPRES-styled, icon-free message box.

    Qt/desktop themes often add blue question marks, red crosses, trash cans and
    floppy-disk icons to standard QMessageBox buttons. NIRUPRES deliberately
    strips those so application dialogs follow the same monochrome UI as the editor.
    """
    box=QMessageBox(parent)
    box.setWindowTitle(title)
    box.setText(text)
    box.setIcon(QMessageBox.NoIcon)
    box.setStandardButtons(buttons)
    if default is not None:
        try: box.setDefaultButton(default)
        except TypeError: pass
    neutralize_button_box(box)
    # QMessageBox may otherwise keep a compact platform-derived width and squeeze
    # long standard-button labels. Reserve enough width for every action + gaps.
    action_width = sum(b.minimumWidth() for b in box.buttons()) + max(0, len(box.buttons()) - 1) * 8 + 32
    box.setMinimumWidth(max(380, action_width))
    return box

def message_info(parent,title,text):
    return make_message_box(parent,title,text,QMessageBox.Ok).exec()

def message_warning(parent,title,text):
    return make_message_box(parent,title,text,QMessageBox.Ok).exec()

def message_critical(parent,title,text):
    return make_message_box(parent,title,text,QMessageBox.Ok).exec()

def message_question(parent,title,text,buttons,default=None):
    return make_message_box(parent,title,text,buttons,default).exec()

def _safe_zip_name(name):
    name=str(name).replace('\\','/')
    if not name or name.startswith('/') or re.match(r'^[A-Za-z]:',name): return False
    parts=Path(name).parts
    return '..' not in parts and all(x not in ('','.') for x in parts)

def _validate_project_data(data):
    if not isinstance(data,dict): raise ValueError('Invalid project: project.json must contain an object.')
    slides=data.get('slides',[])
    if not isinstance(slides,list): raise ValueError('Invalid project: slides must be a list.')
    if len(slides)>2000: raise ValueError('Project contains too many slides (maximum 2000).')
    for i,slide in enumerate(slides):
        if not isinstance(slide,dict): raise ValueError(f'Invalid slide {i+1}.')
        for key in ('title','body','notes','caption','imageRequest','image','backgroundImage','videoSource','videoPoster'):
            value=slide.get(key,'')
            if not isinstance(value,str): raise ValueError(f'Invalid {key} on slide {i+1}.')
            if len(value)>1_000_000: raise ValueError(f'Slide {i+1} contains an excessively large text field.')
    return data

def load_project(path, extract_dir):
    p=Path(path)
    if zipfile.is_zipfile(p):
        root=Path(extract_dir).resolve(); assets=0; total=0; project_bytes=None
        with zipfile.ZipFile(p) as z:
            infos=z.infolist()
            if len(infos)>MAX_ASSETS+8: raise ValueError('Project package contains too many files.')
            for info in infos:
                name=info.filename.replace('\\','/')
                if not _safe_zip_name(name): raise ValueError(f'Unsafe path in project package: {name}')
                mode=(info.external_attr >> 16) & 0xFFFF
                ftype=stat.S_IFMT(mode)
                if stat.S_ISLNK(mode) or (ftype and ftype not in (stat.S_IFREG,stat.S_IFDIR)): raise ValueError(f'Unsupported file type in package: {name}')
                if name!='project.json' and not name.startswith('assets/'):
                    if not name.endswith('/'): raise ValueError(f'Unexpected file in project package: {name}')
                if name.startswith('assets/') and not name.endswith('/'): assets+=1
                total+=info.file_size
                if total>MAX_PACKAGE_UNPACKED: raise ValueError('Project package is too large when unpacked.')
                if name=='project.json':
                    if info.file_size>MAX_PROJECT_JSON: raise ValueError('project.json is too large.')
                    project_bytes=z.read(info)
            if project_bytes is None: raise ValueError('Invalid NIRUPRES package: project.json is missing.')
            if assets>MAX_ASSETS: raise ValueError(f'Project contains too many assets (maximum {MAX_ASSETS}).')
            data=json.loads(project_bytes.decode('utf-8'))
            _validate_project_data(data)
            for info in infos:
                name=info.filename.replace('\\','/')
                if not name.startswith('assets/') or name.endswith('/'): continue
                target=(root/name).resolve()
                if root not in target.parents: raise ValueError(f'Unsafe asset path: {name}')
                target.parent.mkdir(parents=True,exist_ok=True)
                with z.open(info) as src, target.open('wb') as dst: shutil.copyfileobj(src,dst,1024*1024)
        for slide in data.get('slides',[]):
            for field in ('image','backgroundImage','videoPoster','videoSource'):
                image=str(slide.get(field,''))
                if image.startswith('asset:'):
                    rel=image[6:].replace('\\','/')
                    if not _safe_zip_name(rel) or '/' in rel: slide[field]=''
                    else: slide[field]=str(root/'assets'/rel)
    else:
        if p.stat().st_size>MAX_PROJECT_JSON: raise ValueError('Project JSON is too large.')
        data=_validate_project_data(json.loads(p.read_text(encoding='utf-8')))
    data.setdefault('title',p.stem); data.setdefault('theme','B&W'); data.setdefault('showNumbers',False); data.setdefault('showLogo',True); data.setdefault('footerText',''); data.setdefault('footerAlign','LEFT'); data.setdefault('footerSize','MEDIUM'); data['footerAlign']=normalize_footer_align(data.get('footerAlign')); data['footerSize']=normalize_footer_size(data.get('footerSize')); data.setdefault('fontFamily','Noto Sans'); data.setdefault('aspect','16:9'); data.setdefault('transition','NONE'); data.setdefault('reduceMotion',False); data.setdefault('cinematicOpen',True); data.setdefault('cinematicClose',True); data['transition']=normalize_transition(data.get('transition','NONE'))
    data['slides']=[slide_defaults(x) for x in data.get('slides',[])] or [slide_defaults(DEFAULT['slides'][0])]
    normalize_themes(data)
    data['format']=FORMAT; data['appVersion']=VERSION; return data

def _cached_pixmap(path, target_size):
    global _IMAGE_CACHE_BYTES
    try:
        p=Path(path); st=p.stat(); tw=max(64,int(target_size.width())); th=max(64,int(target_size.height()))
        bucket=(min(4096,((tw+255)//256)*256),min(4096,((th+255)//256)*256))
        key=(str(p.resolve()),st.st_mtime_ns,st.st_size,bucket)
        if key in _IMAGE_CACHE:
            pm,cost=_IMAGE_CACHE.pop(key); _IMAGE_CACHE[key]=(pm,cost); return pm
        reader=QImageReader(str(p)); reader.setAutoTransform(True); sz=reader.size()
        if not sz.isValid() or sz.width()<=0 or sz.height()<=0: return QPixmap()
        if sz.width()*sz.height()>MAX_IMAGE_PIXELS: return QPixmap()
        scale=min(1.0,max(bucket[0]/sz.width(),bucket[1]/sz.height())*1.15)
        if scale<1.0: reader.setScaledSize(QSize(max(1,int(sz.width()*scale)),max(1,int(sz.height()*scale))))
        img=reader.read()
        if img.isNull(): return QPixmap()
        pm=QPixmap.fromImage(img); cost=max(1,pm.width()*pm.height()*4)
        _IMAGE_CACHE[key]=(pm,cost); _IMAGE_CACHE_BYTES+=cost
        while _IMAGE_CACHE and _IMAGE_CACHE_BYTES>IMAGE_CACHE_BUDGET:
            _,(_,oldcost)=_IMAGE_CACHE.popitem(last=False); _IMAGE_CACHE_BYTES-=oldcost
        return pm
    except Exception:
        return QPixmap()

def _image_dimensions(path):
    try:
        reader=QImageReader(str(path)); reader.setAutoTransform(True); sz=reader.size()
        return (sz.width(),sz.height()) if sz.isValid() else (0,0)
    except Exception: return (0,0)

def _optimize_asset(src, out_dir, quality_mode='OPTIMIZED'):
    """Create an embedded presentation asset without touching the source file.

    OPTIMIZED targets normal presentation/export use, HIGH keeps extra headroom for
    4K displays, and ORIGINAL is byte-for-byte. The full frame is retained so a
    later focal-point change never loses source content.
    """
    src=Path(src); quality_mode=str(quality_mode or 'OPTIMIZED').upper()
    if quality_mode=='ORIGINAL':
        data=src.read_bytes(); digest=hashlib.sha256(data).hexdigest(); ext=src.suffix.lower() or '.bin'
        dst=Path(out_dir)/(digest[:20]+ext); dst.write_bytes(data)
        return dst, len(data), len(data), _image_dimensions(src), _image_dimensions(src)
    reader=QImageReader(str(src)); reader.setAutoTransform(True); sz=reader.size()
    if not sz.isValid() or sz.width()<=0 or sz.height()<=0 or sz.width()*sz.height()>MAX_IMAGE_PIXELS:
        raise ValueError(f'Unsupported or excessively large image: {src.name}')
    img=reader.read()
    if img.isNull(): raise ValueError(f'Could not decode image: {src.name}')
    before=(img.width(),img.height()); max_edge=4096 if quality_mode=='HIGH' else 3200
    if max(before)>max_edge:
        img=img.scaled(max_edge,max_edge,Qt.KeepAspectRatio,Qt.SmoothTransformation)
    suffix=src.suffix.lower(); has_alpha=img.hasAlphaChannel()
    # Preserve PNG/transparency and line-art formats; photographic JPEG/WebP is
    # encoded as high-quality JPEG for a much smaller portable deck.
    if has_alpha or suffix in ('.png','.bmp'):
        ext='.png'; fmt='PNG'; q=95
    else:
        ext='.jpg'; fmt='JPEG'; q=94 if quality_mode=='HIGH' else 88
    tmp=Path(out_dir)/('asset-tmp'+ext)
    if not img.save(str(tmp),fmt,q): raise ValueError(f'Could not encode image: {src.name}')
    data=tmp.read_bytes(); digest=hashlib.sha256(data).hexdigest(); dst=Path(out_dir)/(digest[:20]+ext)
    if dst.exists(): tmp.unlink()
    else: tmp.replace(dst)
    return dst, src.stat().st_size, dst.stat().st_size, before, (img.width(),img.height())

def _paired_font(texts, family, weight, max_px, min_px, rects):
    """Fit equivalent regions as one typographic system, not independently."""
    size=max_px
    while size>min_px:
        f=QFont(family,size); f.setWeight(weight); fm=QFontMetrics(f)
        ok=True
        for text,rect in zip(texts,rects):
            br=fm.boundingRect(rect.toRect(),int(Qt.TextWordWrap),text or '')
            if br.height()>rect.height() or br.width()>rect.width(): ok=False; break
        if ok:return f
        size-=2
    f=QFont(family,min_px); f.setWeight(weight); return f

def _parse_two_column_body(body):
    """Split TWO COLUMN into two literal text columns.

    `|||` is the portable/Markdown column delimiter. Blank lines are content
    and therefore stay inside their column. For legacy decks without an
    explicit delimiter only, the historical first-blank split is retained so
    they can still be opened and then normalized by the structured editor.
    No line is ever interpreted as a heading by TWO COLUMN.
    """
    raw=str(body or '').replace('\r\n','\n').replace('\r','\n')
    explicit=False
    m=re.search(r'^\s*\|\|\|\s*$',raw,re.M)
    if m:
        left,right=raw[:m.start()],raw[m.end():]; explicit=True
    else:
        m=re.search(r'^\s*\|\|\s*$',raw,re.M)
        if m:
            left,right=raw[:m.start()],raw[m.end():]; explicit=True
        else:
            parts=re.split(r'\n\s*\n',raw,maxsplit=1)
            left=parts[0] if parts else ''; right=parts[1] if len(parts)>1 else ''
    def column(text):
        # Preserve paragraph breaks exactly. Strip only outer whitespace so
        # editor -> save -> render is deterministic and WYSIWYM-like.
        return {'heading':'','groups':[],'text':str(text or '').strip(),'grouped':False}
    return [column(left),column(right)], explicit

def grayscale(pm):
    """Return a grayscale copy using the Qt 6 enum API. Never mutate the source pixmap."""
    if pm.isNull():
        return pm
    img=pm.toImage()
    fmt=getattr(QImage.Format, 'Format_Grayscale8', None)
    if fmt is None:
        return pm
    converted=img.convertToFormat(fmt)
    out=QPixmap.fromImage(converted)
    return out if not out.isNull() else pm

def _plain_markup(text):
    return re.sub(r'(?<!\\)(\*\*|__|\*|_)', '', str(text or ''))

def _inline_html(text):
    import html
    value=html.escape(str(text or ''))
    # Combined emphasis must be parsed before bold/italic so ***text*** is
    # deterministic rather than leaving a stray marker after the bold pass.
    value=re.sub(r'\*\*\*([^*\n]+?)\*\*\*', r'<b><i>\1</i></b>', value)
    value=re.sub(r'___([^_\n]+?)___', r'<b><i>\1</i></b>', value)
    value=re.sub(r'\*\*(.+?)\*\*', r'<b>\1</b>', value)
    value=re.sub(r'__(.+?)__', r'<b>\1</b>', value)
    value=re.sub(r'(?<!\*)\*([^*\n]+?)\*(?!\*)', r'<i>\1</i>', value)
    value=re.sub(r'(?<!_)_([^_\n]+?)_(?!_)', r'<i>\1</i>', value)
    return value.replace('\n','<br>')


def _format_markdown_selection(text, kind):
    """Apply/toggle inline Markdown emphasis without crossing line breaks.

    QTextEdit selections can include the paragraph separator at the end of a
    line. Wrapping that entire selection used to create dangling `*` markers on
    the following line. Each selected line is therefore formatted separately.
    """
    text=str(text or '').replace('\u2029','\n')
    if kind=='normal':
        return _plain_markup(text)
    marker='**' if kind=='bold' else '*'
    out=[]
    for line in text.split('\n'):
        if not line.strip():
            out.append(line); continue
        leading=line[:len(line)-len(line.lstrip())]
        trailing=line[len(line.rstrip()):]
        core=line.strip()
        if kind=='bold' and core.startswith('**') and core.endswith('**') and len(core)>=4:
            core=core[2:-2]
        elif kind=='italic' and core.startswith('*') and core.endswith('*') and not core.startswith('**') and not core.endswith('**') and len(core)>=2:
            core=core[1:-1]
        else:
            core=marker+core+marker
        out.append(leading+core+trailing)
    return '\n'.join(out)

def draw_text(p, rect, text, font, color, flags=Qt.AlignLeft|Qt.AlignVCenter|Qt.TextWordWrap):
    text=str(text or '')
    if not re.search(r'(\*\*\*.+?\*\*\*|\*\*.+?\*\*|__.+?__|(?<!\*)\*[^*\n]+?\*(?!\*)|(?<!_)_[^_\n]+?_(?!_))',text):
        p.setFont(font); p.setPen(QColor(color)); p.drawText(rect, flags, text); return
    doc=QTextDocument(); doc.setDocumentMargin(0); doc.setDefaultFont(font)
    align='center' if flags & Qt.AlignHCenter else ('right' if flags & Qt.AlignRight else 'left')
    doc.setHtml(f'<div style="color:{color}; text-align:{align}; white-space:pre-wrap">{_inline_html(text)}</div>')
    doc.setTextWidth(rect.width())
    natural=doc.size().height(); y=rect.y()
    if flags & Qt.AlignVCenter: y += max(0,(rect.height()-natural)/2)
    elif flags & Qt.AlignBottom: y += max(0,rect.height()-natural)
    p.save(); p.translate(rect.x(),y); p.setClipRect(QRectF(0,0,rect.width(),rect.height()))
    ctx=QAbstractTextDocumentLayout.PaintContext(); doc.documentLayout().draw(p,ctx); p.restore()

def fit_font(text, family, weight, max_px, min_px, rect, max_lines=3):
    size=max_px
    while size>min_px:
        f=QFont(family,size); f.setWeight(weight); fm=QFontMetrics(f)
        br=fm.boundingRect(rect.toRect(), int(Qt.TextWordWrap), _plain_markup(text))
        if br.height() <= rect.height() and br.width() <= rect.width(): return f
        size-=2
    f=QFont(family,min_px); f.setWeight(weight); return f

def draw_image(p, rect, path, mode='FILL', mono=False, fx=.5, fy=.5, opacity=1.0, brightness=0, contrast=0, overlay=0, blur=0, zoom=1.0):
    """Render an image defensively. A bad transform must never break the outer paint cycle."""
    if not path or not Path(path).exists(): return False
    target=rect.toRect()
    if target.width() <= 0 or target.height() <= 0: return False
    pm=_cached_pixmap(path,target.size())
    if pm.isNull(): return False
    try:
        if mono:
            pm=grayscale(pm)
        if blur:
            try:
                scene=QGraphicsScene(); item=QGraphicsPixmapItem(pm); eff=QGraphicsBlurEffect(); eff.setBlurRadius(float(blur)); item.setGraphicsEffect(eff); scene.addItem(item)
                out=QPixmap(pm.size()); out.fill(Qt.transparent); qp=QPainter(out)
                try:
                    scene.render(qp,QRectF(out.rect()),QRectF(pm.rect()))
                finally:
                    if qp.isActive(): qp.end()
                if not out.isNull(): pm=out
            except Exception:
                pass  # keep the unblurred pixmap
        aspect=Qt.KeepAspectRatioByExpanding if mode=='FILL' else Qt.KeepAspectRatio
        zoom=max(1.0,min(1.18,float(zoom or 1.0))) if mode=='FILL' else 1.0
        scale_size=QSize(max(1,int(target.width()*zoom)),max(1,int(target.height()*zoom)))
        scaled=pm.scaled(scale_size,aspect,Qt.SmoothTransformation)
        if scaled.isNull(): return False
        if mode=='FILL':
            maxx=max(0,scaled.width()-target.width()); maxy=max(0,scaled.height()-target.height())
            sx=int(maxx*max(0,min(1,fx))); sy=int(maxy*max(0,min(1,fy)))
            scaled=scaled.copy(sx,sy,min(target.width(),scaled.width()-sx),min(target.height(),scaled.height()-sy))
            dest=target
        else:
            dest=QRectF(target.x()+(target.width()-scaled.width())/2,target.y()+(target.height()-scaled.height())/2,scaled.width(),scaled.height()).toRect()
        p.save()
        try:
            p.setOpacity(max(0.0,min(1.0,float(opacity)))); p.drawPixmap(dest,scaled); p.setOpacity(1.0)
            if brightness:
                p.fillRect(dest,QColor(255,255,255,min(150,int(brightness*1.5))) if brightness>0 else QColor(0,0,0,min(150,int(-brightness*1.5))))
            if contrast>0:
                p.setCompositionMode(QPainter.CompositionMode_Overlay); p.fillRect(dest,QColor(255,255,255,min(100,int(contrast)))); p.setCompositionMode(QPainter.CompositionMode_SourceOver)
            if overlay:
                p.fillRect(dest,QColor(0,0,0,min(220,int(overlay*2.2))))
        finally:
            p.setOpacity(1.0)
            p.setCompositionMode(QPainter.CompositionMode_SourceOver)
            p.restore()
        return True
    except Exception as exc:
        print(f'NIRUPRES image render warning: {exc}', file=sys.stderr)
        return False

def timeline_cells(safe_rect, count, yline):
    """Return bounded TIMELINE node/text geometry for 1–5 milestones.

    Geometry is deliberately independent of slide content so old decks benefit
    automatically from renderer improvements without migration.
    """
    count=max(1,min(5,int(count or 1)))
    cell_w=safe_rect.width()/count
    pad=min(cell_w*.08, safe_rect.width()*.012)
    head_h=safe_rect.height()*.075
    desc_h=safe_rect.height()*.125
    out=[]
    for i in range(count):
        left=safe_rect.x()+i*cell_w
        inner=QRectF(left+pad,safe_rect.y(),max(1,cell_w-2*pad),safe_rect.height())
        xx=left+cell_w*.5
        head=QRectF(inner.x(),yline-safe_rect.height()*.17,inner.width(),head_h)
        desc=QRectF(inner.x(),yline+safe_rect.height()*.055,inner.width(),desc_h)
        out.append((xx,head,desc))
    return out

def render_slide(p, rect, s, theme_name, index=0, total=1, show_numbers=True, deck_font='Noto Sans', show_logo=True, footer_text='', footer_align='LEFT', footer_size='MEDIUM'):
    theme_name=s.get('themeOverride','DECK') if s.get('themeOverride','DECK')!='DECK' else theme_name; theme_name=normalize_theme(theme_name); t=dict(THEMES.get(theme_name,THEMES['B&W'])); accent_override=str(s.get('accentOverride','AUTO')).upper(); t['accent']=UDDEVALLA_ACCENTS.get(accent_override,t['accent']); p.save(); p.setRenderHint(QPainter.Antialiasing); p.setRenderHint(QPainter.SmoothPixmapTransform)
    p.fillRect(rect,QColor(t['bg'])); x,y,w,h=rect.x(),rect.y(),rect.width(),rect.height(); m=w*.055; top=h*.055
    # Optional slide background image. It always fills the canvas and is rendered
    # beneath theme decoration/content so branded accent edges remain intact.
    bgimg=str(s.get('backgroundImage','') or '')
    if bgimg:
        bgfx=float(s.get('backgroundFocalX',.5)); bgfy=float(s.get('backgroundFocalY',.5))
        bgdim=max(0,min(90,int(s.get('backgroundDim',75)))); bgblur=max(0,min(20,int(s.get('backgroundBlur',0))))
        draw_image(p,rect,bgimg,'FILL',bool(s.get('backgroundMono',False)),bgfx,bgfy,1.0,0,0,0,bgblur,float(s.get('_backgroundZoom',1.0) or 1.0))
        # Theme-coloured veil preserves readability on both light and dark themes.
        veil=QColor(t['bg']); veil.setAlpha(int(255*bgdim/100)); p.fillRect(rect,veil)
    layout=s.get('layout','TITLE'); title=s.get('title',''); body=s.get('body',''); img=s.get('image',''); treatment=str(s.get('imageTreatment','MONO' if s.get('mono') else 'NATURAL')).upper(); mono=(treatment=='MONO') or t['imageMono']; mode=s.get('imageMode','FILL'); fx=s.get('focalX',.5); fy=s.get('focalY',.5); br=s.get('brightness',0); ct=s.get('contrast',0); ov=s.get('overlay',0); bl=s.get('blur',0); br=br-10 if treatment=='DIM' else br; ov=max(ov,18) if treatment=='DIM' else ov; ct=max(ct,24) if treatment=='CONTRAST' else ct
    sans=s.get('fontFamily','DECK'); sans=deck_font if sans=='DECK' else sans; mono_font='Noto Sans Mono'; scale=float(s.get('fontScale',1.0)); align_name=s.get('align','LEFT'); base_align={'LEFT':Qt.AlignLeft,'CENTER':Qt.AlignHCenter,'RIGHT':Qt.AlignRight}.get(align_name,Qt.AlignLeft)
    # restrained metadata: no background container
    if show_numbers and layout not in ('FULL BLEED',):
        f=QFont(mono_font,max(7,int(h*.013))); f.setLetterSpacing(QFont.AbsoluteSpacing,max(1,w*.0012)); draw_text(p,QRectF(x+m,y+top,w*.5,h*.04),f'{index+1:02d} / {total:02d}',f,t['muted'])
    def hero(r,txt,maxp=.082,minp=.038,align=Qt.AlignLeft|Qt.AlignVCenter|Qt.TextWordWrap):
        hero_weight={'LIGHT':QFont.Light,'REGULAR':QFont.Normal,'BOLD':QFont.Bold,'AUTO':QFont.Black}.get(s.get('weight','AUTO'),QFont.Black); f=fit_font(txt,sans,hero_weight,int(h*maxp*scale),int(h*minp*scale),r); draw_text(p,r,txt,f,t['fg'], (base_align|Qt.AlignVCenter|Qt.TextWordWrap) if align==(Qt.AlignLeft|Qt.AlignVCenter|Qt.TextWordWrap) else align)
    def bodytxt(r,txt,align=Qt.AlignLeft|Qt.AlignVCenter|Qt.TextWordWrap,maxp=.027,minp=.016,color=None):
        # Content-aware body typography: short copy stays generous, dense copy
        # scales down rather than overflowing. This is shared by every layout.
        lines=max(1,len([q for q in str(txt or '').splitlines() if q.strip()]))
        density=max(1.0,len(str(txt or ''))/150.0,lines/6.0)
        top=maxp if density<=1.0 else max(minp,maxp/density**0.35)
        f=fit_font(txt,sans,QFont.Normal,int(h*top*scale),max(9,int(h*minp*scale)),r,max(2,lines+2))
        draw_text(p,r,txt,f,color or t['muted'],align)
    def accent_for(n=0):
        colors=t.get('profile') or [t['accent']]
        return colors[n % len(colors)]
    if layout in ('VIDEO','VIDEO + TEXT'):
        hero(QRectF(x+m,y+h*.08,w*.82,h*.10),title,.045,.026)
        vr=QRectF(x+m,y+h*.23,w-2*m,h*.58) if layout=='VIDEO' else QRectF(x+m,y+h*.25,w*.56,h*.52)
        poster=s.get('videoPoster') or img
        if poster: draw_image(p,vr,poster,'FILL',mono,fx,fy,1.0,br,ct,ov,bl)
        else: p.fillRect(vr,QColor(t['rule']))
        p.setPen(QColor(t['fg'])); p.setBrush(Qt.NoBrush); p.drawRect(vr)
        tri=QFont(sans,max(18,int(h*.055)),QFont.Bold); draw_text(p,QRectF(vr.center().x()-w*.04,vr.center().y()-h*.05,w*.08,h*.10),'▶',tri,t['fg'],Qt.AlignCenter)
        if layout=='VIDEO + TEXT': bodytxt(QRectF(x+w*.68,y+h*.27,w*.25,h*.46),body,Qt.AlignLeft|Qt.AlignTop|Qt.TextWordWrap,maxp=.024,minp=.015,color=t['fg'])
        elif body: bodytxt(QRectF(x+m,y+h*.84,w-2*m,h*.07),body,Qt.AlignLeft|Qt.AlignVCenter|Qt.TextWordWrap,maxp=.018,minp=.013)
    elif layout=='FULL BLEED':
        draw_image(p,rect,img,'FILL',mono,fx,fy,1.0,br,ct,ov,bl)
        p.fillRect(rect,QColor(0,0,0,105)); hero(QRectF(x+m,y+h*.55,w-2*m,h*.25),title,.085,.04); f=QFont(sans,int(h*.025)); draw_text(p,QRectF(x+m,y+h*.82,w-2*m,h*.08),body,f,'#E8E8E8')
    elif layout=='AGENDA':
        hero(QRectF(x+m,y+h*.11,w*.72,h*.13),title,.052,.030)
        items=[re.sub(r'^(?:[-•–—]\s+|\*\s+)','',q.strip()) for q in body.splitlines() if q.strip()][:6]
        yy=y+h*.31; step=h*.092
        for n,item in enumerate(items):
            style=str(s.get('listStyle','NUMBERS')).upper(); marker={'NUMBERS':f'{n+1:02d}','DOTS':'•','DASHES':'—','NONE':''}.get(style,f'{n+1:02d}')
            if marker: draw_text(p,QRectF(x+m,yy,w*.07,step*.72),marker,QFont(mono_font,max(11,int(h*.025))),accent_for(n),Qt.AlignLeft|Qt.AlignVCenter)
            tx=x+m+(w*.13 if marker else 0); tw=w*(.68 if marker else .81)
            if marker: p.fillRect(QRectF(x+m+w*.075,yy+step*.35,w*.035,max(1,h*.002)),QColor(t['rule']))
            f=fit_font(item,sans,QFont.Medium,int(h*.032),int(h*.020),QRectF(tx,yy,tw,step*.72),2); draw_text(p,QRectF(tx,yy,tw,step*.72),item,f,t['fg'])
            yy+=step
    elif layout=='HERO IMAGE':
        ir=QRectF(x+w*.40,y,w*.60,h); drawn=draw_image(p,ir,img,'FILL',mono,fx,fy,1.0,br,ct,ov,bl)
        if drawn: p.fillRect(QRectF(x+w*.40,y,w*.10,h),QColor(t['bg']))
        hero(QRectF(x+m,y+h*.22,w*.39,h*.30),title,.075,.034); p.fillRect(QRectF(x+m,y+h*.57,w*.08,max(2,h*.005)),QColor(t['accent'])); bodytxt(QRectF(x+m,y+h*.61,w*.31,h*.18),body,maxp=.024)
    elif layout=='COMPARE':
        # COMPARE uses the same literal two-pane contract as TWO COLUMN.
        # No first-line heading inference: what the user writes is what renders.
        hero(QRectF(x+m,y+h*.10,w*.82,h*.12),title,.050,.028)
        if bool(s.get('sideStructured') or s.get('twoColumnStructured')) and s.get('leftColumn') is not None and s.get('rightColumn') is not None:
            left_text=str(s.get('leftColumn') or '').strip(); right_text=str(s.get('rightColumn') or '').strip()
        else:
            cols,_=_parse_two_column_body(body); left_text=cols[0]['text']; right_text=cols[1]['text']
        gutter=w*.060; rw=(w-2*m-gutter)/2; top=y+h*.31; content_h=h*.45
        rects=[QRectF(x+m,top,rw,content_h),QRectF(x+m+rw+gutter,top,rw,content_h)]
        bf=_paired_font([left_text,right_text],sans,QFont.Normal,int(h*.027),int(h*.016),rects)
        draw_text(p,rects[0],left_text,bf,t['fg'],Qt.AlignLeft|Qt.AlignTop|Qt.TextWordWrap)
        draw_text(p,rects[1],right_text,bf,t['fg'],Qt.AlignLeft|Qt.AlignTop|Qt.TextWordWrap)
        p.fillRect(QRectF(rects[0].x(),top-h*.018,rects[0].width(),max(2,h*.004)),QColor(t['accent'])); p.fillRect(QRectF(rects[1].x(),top-h*.018,rects[1].width(),max(2,h*.004)),QColor(t['accent']))
    elif layout=='DATA / KPI':
        hero(QRectF(x+m,y+h*.09,w*.74,h*.10),title,.043,.025)
        items=[q.strip() for q in body.splitlines() if q.strip()][:4]
        primary=(items[0].split('|',1) if items else ['—','Primary metric']); num=primary[0].strip(); lab=primary[1].strip() if len(primary)>1 else ''
        f=fit_font(num,sans,QFont.Black,int(h*.20),int(h*.09),QRectF(x+m,y+h*.25,w*.48,h*.25),1); draw_text(p,QRectF(x+m,y+h*.22,w*.48,h*.28),num,f,t['accent'])
        bodytxt(QRectF(x+m,y+h*.50,w*.42,h*.10),lab,maxp=.025,color=t['fg'])
        rest=items[1:4]
        for n,item in enumerate(rest):
            a=item.split('|',1); val=a[0].strip(); label=a[1].strip() if len(a)>1 else ''; ry=y+h*(.27+n*.17)
            draw_text(p,QRectF(x+w*.61,ry,w*.15,h*.07),val,QFont(sans,max(12,int(h*.036)),QFont.Bold),t['fg'])
            draw_text(p,QRectF(x+w*.77,ry,w*.16,h*.07),label,QFont(sans,max(8,int(h*.017))),t['muted'])
            if n<len(rest)-1:p.fillRect(QRectF(x+w*.61,ry+h*.105,w*.31,max(1,h*.002)),QColor(t['rule']))
    elif layout=='PROCESS':
        hero(QRectF(x+m,y+h*.10,w*.80,h*.12),title,.050,.028)
        items=[q.strip() for q in body.splitlines() if q.strip()][:5]; n=max(1,len(items)); gap=w*.018; cell=(w-2*m-gap*(n-1))/n
        y0=y+h*.35
        for j,item in enumerate(items):
            a=item.split('|',1); head=a[0].strip(); desc=a[1].strip() if len(a)>1 else ''; rx=x+m+j*(cell+gap)
            draw_text(p,QRectF(rx,y0,cell,h*.06),f'{j+1:02d}',QFont(mono_font,max(9,int(h*.018))),accent_for(j),Qt.AlignLeft|Qt.AlignVCenter)
            p.fillRect(QRectF(rx,y0+h*.085,cell,max(2,h*.004)),QColor(accent_for(j)))
            f=fit_font(head,sans,QFont.Bold,int(h*.030),int(h*.019),QRectF(rx,y0+h*.12,cell,h*.10),2); draw_text(p,QRectF(rx,y0+h*.12,cell,h*.10),head,f,t['fg'],Qt.AlignLeft|Qt.AlignTop|Qt.TextWordWrap)
            bodytxt(QRectF(rx,y0+h*.23,cell,h*.17),desc,Qt.AlignLeft|Qt.AlignTop|Qt.TextWordWrap,maxp=.019,minp=.014)
    elif layout=='MATRIX':
        hero(QRectF(x+m,y+h*.08,w*.78,h*.11),title,.046,.026)
        items=[q.strip() for q in body.splitlines() if q.strip()][:4]
        for n,item in enumerate(items):
            a=item.split('|',1); head=a[0].strip(); desc=a[1].strip() if len(a)>1 else ''; col=n%2; row=n//2; rx=x+m+col*w*.445; ry=y+h*(.28+row*.29); rw=w*.39
            p.fillRect(QRectF(rx,ry,w*.035,max(2,h*.004)),QColor(accent_for(n)))
            draw_text(p,QRectF(rx,ry+h*.035,rw,h*.065),head,QFont(sans,max(10,int(h*.025)),QFont.Bold),t['fg'])
            bodytxt(QRectF(rx,ry+h*.105,rw,h*.12),desc,Qt.AlignLeft|Qt.AlignTop|Qt.TextWordWrap,maxp=.020,minp=.014)
    elif layout=='TABLE':
        hero(QRectF(x+m,y+h*.08,w*.78,h*.11),title,.046,.026)
        rows=[[c.strip() for c in q.split('|')] for q in body.splitlines() if q.strip()][:7]; cols=max([len(r) for r in rows],default=1); cols=min(cols,4); topy=y+h*.26; rh=h*.085; cw=(w-2*m)/cols
        for ri,row in enumerate(rows):
            for ci in range(cols):
                txt=row[ci] if ci<len(row) else ''; rr=QRectF(x+m+ci*cw,topy+ri*rh,cw-w*.008,rh)
                f=fit_font(txt,sans,QFont.Bold if ri==0 else QFont.Normal,int(h*(.021 if ri==0 else .019)),int(h*.014),rr,2); draw_text(p,rr,txt,f,t['fg'] if ri==0 else t['muted'],Qt.AlignLeft|Qt.AlignVCenter|Qt.TextWordWrap)
            p.fillRect(QRectF(x+m,topy+(ri+1)*rh-w*.001,w-2*m,max(1,h*.0015)),QColor(t['accent'] if ri==0 else t['rule']))
    elif layout=='IMAGE':
        ir=QRectF(x+m,y+h*.15,w-2*m,h*.68); draw_image(p,ir,img,mode,mono,fx,fy,1.0,br,ct,ov,bl)
        bodytxt(QRectF(x+m,y+h*.84,w-2*m,h*.06),body);
        if s.get('caption'): draw_text(p,QRectF(x+m,y+h*.92,w-2*m,h*.035),s.get('caption'),QFont(sans,max(7,int(h*.014))),t['muted'],Qt.AlignRight|Qt.AlignVCenter)
    elif layout=='SPLIT':
        # Editorial 42/58 composition: image carries slightly more visual weight
        # while text keeps a strict readable measure.
        left=QRectF(x+m,y+h*.18,w*.36,h*.64); right=QRectF(x+w*.50,y+h*.12,w*.445,h*.76)
        hero(left,title,.068,.032); bodytxt(QRectF(left.x(),left.y()+left.height()*.57,left.width(),left.height()*.39),body,maxp=.025)
        draw_image(p,right,img,mode,mono,fx,fy,1.0,br,ct,ov,bl)
        p.fillRect(QRectF(x+w*.465,y+h*.27,max(1,w*.0015),h*.46),QColor(t['rule']))
    elif layout=='PHOTO':
        # Photography is the subject: larger image, quieter supporting copy.
        ir=QRectF(x+m,y+h*.095,w-2*m,h*.735); draw_image(p,ir,img,mode,mono,fx,fy,1.0,br,ct,ov,bl)
        hero(QRectF(x+m,y+h*.845,w*.50,h*.06),title,.032,.020)
        bodytxt(QRectF(x+m+w*.54,y+h*.84,w*.40,h*.07),body,Qt.AlignRight|Qt.AlignVCenter|Qt.TextWordWrap,maxp=.019,minp=.013)
        if s.get('caption'): draw_text(p,QRectF(x+m,y+h*.915,w-2*m,h*.025),s.get('caption'),QFont(sans,max(7,int(h*.012))),t['muted'],Qt.AlignRight|Qt.AlignVCenter)
    elif layout=='BULLETS':
        hero(QRectF(x+m,y+h*.13,w*.80,h*.14),title,.058,.030)
        p.fillRect(QRectF(x+m,y+h*.305,w*.055,max(2,h*.0035)),QColor(t['accent']))
        items=[re.sub(r'^(?:[-•–—]\s+|\*\s+)','',q.strip()) for q in body.splitlines() if q.strip()][:7]
        yy=y+h*.365; step=min(h*.079, h*.50/max(1,len(items)))
        list_style=str(s.get('listStyle','NUMBERS')).upper()
        for bullet_i,item in enumerate(items):
            marker={'NUMBERS':f'{bullet_i+1:02d}','DOTS':'•','DASHES':'—','NONE':''}.get(list_style,f'{bullet_i+1:02d}')
            marker_w=w*.045 if marker else 0
            if marker: draw_text(p,QRectF(x+m,yy,marker_w,step*.82),marker,QFont(mono_font,max(8,int(h*.015))),accent_for(bullet_i),Qt.AlignLeft|Qt.AlignVCenter)
            tx=x+m+(w*.055 if marker else 0); tw=w*(.72 if marker else .78)
            f=fit_font(item,sans,QFont.Normal,int(h*.027),int(h*.018),QRectF(tx,yy,tw,step*.82)); draw_text(p,QRectF(tx,yy,tw,step*.82),item,f,t['fg'])
            yy+=step
    elif layout=='TWO COLUMN':
        # Literal paired columns. The editor owns the left/right split; the
        # renderer never infers headings, groups or column moves from content.
        # Blank lines are rendered as blank lines, exactly as authored.
        hero(QRectF(x+m,y+h*.115,w*.82,h*.14),title,.055,.030); p.fillRect(QRectF(x+m,y+h*.285,w*.065,max(2,h*.0035)),QColor(t['accent']))
        if bool(s.get('sideStructured') or s.get('twoColumnStructured')) and s.get('leftColumn') is not None and s.get('rightColumn') is not None:
            left_text=str(s.get('leftColumn') or '').strip()
            right_text=str(s.get('rightColumn') or '').strip()
        else:
            cols,_=_parse_two_column_body(body)
            left_text=cols[0]['text']; right_text=cols[1]['text']
        gutter=w*.070; half=(w-2*m-gutter)/2; top=y+h*.355; content_h=h*.40
        rects=[QRectF(x+m,top,half,content_h),QRectF(x+m+half+gutter,top,half,content_h)]
        # One font calculation for the pair keeps both sides visually balanced.
        bf=_paired_font([left_text,right_text],sans,QFont.Normal,int(h*.027),int(h*.016),rects)
        draw_text(p,rects[0],left_text,bf,t['fg'],Qt.AlignLeft|Qt.AlignTop|Qt.TextWordWrap)
        draw_text(p,rects[1],right_text,bf,t['fg'],Qt.AlignLeft|Qt.AlignTop|Qt.TextWordWrap)
        p.fillRect(QRectF(x+w*.5-w*.0006,top,max(1,w*.0012),content_h),QColor(t['rule']))
    elif layout=='IMAGE + QUOTE':
        ir=QRectF(x+m,y+h*.11,w*.49,h*.75); draw_image(p,ir,img,mode,mono,fx,fy,1.0,br,ct,ov,bl)
        p.fillRect(QRectF(x+w*.555,y+h*.25,max(1,w*.0014),h*.43),QColor(t['rule']))
        hero(QRectF(x+w*.59,y+h*.22,w*.34,h*.32),title,.056,.029); bodytxt(QRectF(x+w*.59,y+h*.60,w*.32,h*.14),body,maxp=.022)
    elif layout=='NUMBER GRID':
        hero(QRectF(x+m,y+h*.10,w*.82,h*.12),title,.05,.028)
        items=[q.strip() for q in body.splitlines() if q.strip()][:4]
        for n,item in enumerate(items):
            parts=item.split('|',1); num=parts[0].strip(); label=parts[1].strip() if len(parts)>1 else ''
            col=n%2; row=n//2; rx=x+m+col*w*.44; ry=y+h*(.30+row*.29)
            f=fit_font(num,sans,QFont.Black,int(h*.105),int(h*.055),QRectF(rx,ry,w*.36,h*.14),1); draw_text(p,QRectF(rx,ry,w*.36,h*.14),num,f,accent_for(n))
            draw_text(p,QRectF(rx,ry+h*.13,w*.36,h*.07),label,QFont(sans,max(10,int(h*.022))),t['muted'])
    elif layout=='TIMELINE':
        # Each milestone owns a bounded cell inside the slide safe area.  The
        # previous renderer centred a fixed-width text box on the first/last
        # nodes, which could push text beyond the slide edge.  Cell geometry
        # keeps existing decks intact while making 1–5 milestones reflow safely.
        hero(QRectF(x+m,y+h*.12,w*.82,h*.13),title,.052,.028)
        items=[q.strip() for q in body.splitlines() if q.strip()][:5]
        n=max(1,len(items)); yline=y+h*.54
        cells=timeline_cells(QRectF(x+m,y,w-2*m,h),n,yline)
        if cells:
            first_x=cells[0][0]; last_x=cells[-1][0]
            p.fillRect(QRectF(first_x,yline,max(1,last_x-first_x),max(2,h*.004)),QColor(t['rule']))
        for j,item in enumerate(items):
            xx,head_rect,desc_rect=cells[j]; parts=item.split('|',1); head=parts[0].strip(); desc=parts[1].strip() if len(parts)>1 else ''
            p.setBrush(QColor(accent_for(j)));p.setPen(Qt.NoPen);p.drawEllipse(QRectF(xx-w*.008,yline-w*.008,w*.016,w*.016))
            hf=fit_font(head,sans,QFont.Normal,int(h*.022),int(h*.015),head_rect,2)
            df=fit_font(desc,sans,QFont.Normal,int(h*.017),int(h*.012),desc_rect,4)
            draw_text(p,head_rect,head,hf,t['fg'],Qt.AlignCenter|Qt.AlignVCenter|Qt.TextWordWrap)
            draw_text(p,desc_rect,desc,df,t['muted'],Qt.AlignCenter|Qt.AlignTop|Qt.TextWordWrap)
    elif layout=='QUOTE':
        # Editorial quote: oversized punctuation is deliberately low contrast so
        # the words remain the visual subject.
        draw_text(p,QRectF(x+m,y+h*.12,w*.18,h*.20),'“',QFont(sans,max(28,int(h*.16)),QFont.Black),t['rule'],Qt.AlignLeft|Qt.AlignTop)
        p.fillRect(QRectF(x+m,y+h*.28,max(2,w*.003),h*.43),QColor(t['accent']))
        q=QRectF(x+m+w*.045,y+h*.23,w*.76,h*.42); hero(q,title,.070,.034)
        draw_text(p,QRectF(q.x(),y+h*.70,q.width(),h*.08),body,QFont(sans,max(10,int(h*.022))),t['muted'])
    elif layout=='BIG NUMBER':
        f=fit_font(title,sans,QFont.Black,int(h*.23),int(h*.10),QRectF(x+m,y+h*.22,w-2*m,h*.38),1); draw_text(p,QRectF(x+m,y+h*.20,w-2*m,h*.42),title,f,t['accent'],Qt.AlignLeft|Qt.AlignVCenter)
        bodytxt(QRectF(x+m,y+h*.64,w*.72,h*.14),body)
    elif layout=='SECTION':
        # Strong chapter divider: optional numeric prefix becomes a large section marker.
        sm=re.match(r'^\s*(\d{1,2})[. :–—-]+(.*)$',title)
        if sm:
            draw_text(p,QRectF(x+m,y+h*.17,w*.18,h*.18),sm.group(1).zfill(2),QFont(sans,max(18,int(h*.10))),t['accent'],Qt.AlignLeft|Qt.AlignVCenter)
            section_title=sm.group(2).strip()
        else: section_title=title
        p.fillRect(QRectF(x+m,y+h*.48,w*.10,max(2,h*.006)),QColor(t['accent'])); hero(QRectF(x+m,y+h*.27,w*.78,h*.20),section_title,.07,.036); bodytxt(QRectF(x+m+w*.14,y+h*.51,w*.64,h*.12),body)
    elif layout=='END':
        hero(QRectF(x+m,y+h*.27,w-2*m,h*.22),title,.085,.04,Qt.AlignCenter|Qt.AlignVCenter|Qt.TextWordWrap); bodytxt(QRectF(x+m,y+h*.51,w-2*m,h*.16),body,Qt.AlignCenter|Qt.AlignVCenter|Qt.TextWordWrap)
        if not t.get('uddevalla'):
            p.fillRect(QRectF(x+w*.45,y+h*.74,w*.10,max(2,h*.006)),QColor(t['accent']))
    elif layout=='STATEMENT':
        hero(QRectF(x+m,y+h*.24,w-2*m,h*.38),title,.095,.045); p.fillRect(QRectF(x+m,y+h*.67,w*.10,max(2,h*.006)),QColor(t['accent'])); bodytxt(QRectF(x+m,y+h*.70,w*.75,h*.12),body)
    elif layout=='TEXT':
        hero(QRectF(x+m,y+h*.18,w*.78,h*.16),title,.06,.034); p.fillRect(QRectF(x+m,y+h*.36,w*.07,max(2,h*.004)),QColor(t['accent'])); bodytxt(QRectF(x+m,y+h*.40,w*.78,h*.38),body)
    else: # TITLE
        hero(QRectF(x+m,y+h*.25,w*.82,h*.25),title,.09,.04); p.fillRect(QRectF(x+m,y+h*.54,w*.10,max(2,h*.006)),QColor(t['accent'])); bodytxt(QRectF(x+m,y+h*.58,w*.75,h*.13),body)
        if img: draw_image(p,QRectF(x+w*.64,y+h*.18,w*.30,h*.62),img,mode,mono,fx,fy,.92,br,ct,ov,bl)
    if s.get('imageRequest') and not s.get('image'):
        f=QFont(mono_font,max(8,int(h*.014))); draw_text(p,QRectF(x+w*.60,y+h*.90,w*.34,h*.04),'IMAGE REQUEST  ·  '+s.get('imageRequest',''),f,t['muted'],Qt.AlignRight|Qt.AlignVCenter|Qt.TextSingleLine)
    if theme_name=='SATIE':
        p.setPen(QColor(t['rule'])); p.drawLine(int(x+w*.80),int(y+h*.09),int(x+w*.94),int(y+h*.09)); p.drawLine(int(x+w*.94),int(y+h*.09),int(x+w*.94),int(y+h*.18))
    if theme_name=='CARL LARSSON':
        # Warm domestic palette: Falun red, sage, blue-grey and ochre.
        # Keep it architectural and quiet: no decorative dots or enclosing frame.
        palette=t.get('palette',['#A33E32','#557A68','#66859A','#C39A45'])
        band_w=w*.022; band_h=max(2,h*.004)
        for j,c in enumerate(palette): p.fillRect(QRectF(x+m+j*band_w,y+h*.925,band_w*.72,band_h),QColor(c))
    if t.get('uddevalla'):
        # Uddevalla identity: one restrained accent edge; logo artwork is never recoloured.
        p.fillRect(QRectF(x,y,w*.010,h),QColor(t['accent']))
        logo_mode=str(s.get('logoMode','AUTO')).upper()
        auto_layout=layout in ('TITLE','SECTION','END')
        want_logo=show_logo and logo_mode!='OFF' and (logo_mode=='ON' or auto_layout)
        if want_logo:
            # Full-bleed imagery has unknown luminance: use the supplied white-disc logo.
            if layout=='FULL BLEED': logo_path=BRANDING/'uddevalla-white-disc.png'
            else:
                bg=QColor(t['bg']); lum=(0.2126*bg.redF()+0.7152*bg.greenF()+0.0722*bg.blueF())
                logo_path=BRANDING/('uddevalla-light.png' if lum >= 0.55 else 'uddevalla-dark.png')
            # AUTO placement keeps clear of the dominant content region for each layout.
            side=h*.135
            if layout=='END': lr=QRectF(x+w*.5-side*.5,y+h*.755,side,side)
            elif layout in ('SPLIT','IMAGE + QUOTE'): lr=QRectF(x+m,y+h*.80,side,side)
            elif layout in ('IMAGE','PHOTO','FULL BLEED'): lr=QRectF(x+w-side-w*.035,y+h*.035,side,side)
            elif layout=='TITLE' and img: lr=QRectF(x+m,y+h*.805,side,side)
            else: lr=QRectF(x+w-side-w*.038,y+h*.805,side,side)
            pm=_cached_pixmap(str(logo_path),QSize(max(64,int(lr.width())),max(64,int(lr.height()))))
            if not pm.isNull():
                scaled=pm.scaled(int(lr.width()),int(lr.height()),Qt.KeepAspectRatio,Qt.SmoothTransformation)
                dx=lr.x()+(lr.width()-scaled.width())/2; dy=lr.y()+(lr.height()-scaled.height())/2
                p.drawPixmap(int(dx),int(dy),scaled)
    if footer_text and layout!='FULL BLEED':
        # Footer is deck metadata: always subtle, but configurable. Keep it inside
        # a conservative safe area so long text cannot collide with edge branding.
        footer_align=normalize_footer_align(footer_align); footer_size=normalize_footer_size(footer_size)
        size_factor={'SMALL':.010,'MEDIUM':.012,'LARGE':.014}[footer_size]
        ff=QFont(sans,max(7,int(h*size_factor)))
        footer_flags={'LEFT':Qt.AlignLeft,'CENTER':Qt.AlignHCenter,'RIGHT':Qt.AlignRight}[footer_align]|Qt.AlignVCenter|Qt.TextSingleLine
        footer_rect=QRectF(x+m,y+h*.938,w-2*m,h*.035)
        fm=QFontMetrics(ff); shown=fm.elidedText(str(footer_text),Qt.ElideRight,max(1,int(footer_rect.width())))
        draw_text(p,footer_rect,shown,ff,t['muted'],footer_flags)
    p.restore()

