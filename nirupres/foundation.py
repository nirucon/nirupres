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
    QPicture,
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

from . import textfit

APP = "NIRUPRES"
VERSION = "2.3.0"
FORMAT = 9
MD_FORMAT = 2
TAGLINE = "A minimal, suckless presentation app by Nicklas Rudolfsson."
MB = 1024 * 1024
MAX_PROJECT_JSON = 5 * MB
MAX_ASSETS = 250
MAX_PACKAGE_UNPACKED = 500 * MB
MAX_IMAGE_PIXELS = 50_000_000
IMAGE_CACHE_BUDGET = 192 * MB
MAX_HISTORY_BYTES = 24 * MB
EXPORT_PROFILES = {"preview": (1920, 1080), "pptx": (2560, 1440)}
_IMAGE_CACHE = OrderedDict()
_IMAGE_CACHE_BYTES = 0
DATA = Path(os.environ.get("XDG_DATA_HOME") or (Path.home() / ".local/share")) / "nirupres"
AUTOSAVE = DATA / "autosave.json"
RECENTS = DATA / "recent.json"
DISPLAY_PREFS = DATA / "display.json"
ASSETS = Path(__file__).resolve().parent / "assets"
BRANDING = ASSETS / "branding"
FILE_PREFS = DATA / "file-workflow.json"
THEMES = {
    "B&W": {
        "bg": "#050505",
        "fg": "#F4F2ED",
        "muted": "#969696",
        "accent": "#F4F2ED",
        "rule": "#2A2A2A",
        "imageMono": True,
    },
    "W&B": {
        "bg": "#F4F2ED",
        "fg": "#090909",
        "muted": "#66635F",
        "accent": "#090909",
        "rule": "#D4D0C8",
        "imageMono": True,
    },
    "GRAY": {
        "bg": "#242424",
        "fg": "#ECECEC",
        "muted": "#A8A8A8",
        "accent": "#CFCFCF",
        "rule": "#505050",
        "imageMono": True,
    },
    "REDWINE": {
        "bg": "#210B11",
        "fg": "#F1E8E3",
        "muted": "#B89DA4",
        "accent": "#8F263B",
        "rule": "#54202C",
        "imageMono": False,
    },
    "SATIE": {
        "bg": "#EDE7D8",
        "fg": "#191815",
        "muted": "#6F695F",
        "accent": "#2E2923",
        "rule": "#C7BEAC",
        "imageMono": True,
    },
    "CARL LARSSON": {
        "bg": "#EFE1BF",
        "fg": "#26372F",
        "muted": "#78664F",
        "accent": "#A33E32",
        "rule": "#B8A47B",
        "imageMono": False,
        "palette": ["#A33E32", "#557A68", "#66859A", "#C39A45"],
    },
    "BEKSINSKI": {
        "bg": "#0A0807",
        "fg": "#E9E0D2",
        "muted": "#8F8274",
        "accent": "#7B3028",
        "rule": "#30251F",
        "imageMono": False,
    },
    "MARTIN": {
        "bg": "#0A0D0E",
        "fg": "#EEE8D9",
        "muted": "#92938D",
        "accent": "#B78C4A",
        "rule": "#293034",
        "imageMono": False,
    },
    "SFUMATO": {
        "bg": "#171713",
        "fg": "#E8E3D5",
        "muted": "#9C988C",
        "accent": "#8A8067",
        "rule": "#37362F",
        "imageMono": False,
    },
    # Uddevalla family. Each theme uses one official profile colour consistently.
    # Logo artwork itself is never recoloured; the renderer chooses the supplied
    # light/dark logo asset from the actual theme background luminance.
    "UDDEVALLA DARK": {
        "bg": "#07141D",
        "fg": "#ECEDED",
        "muted": "#A9BAC4",
        "accent": "#0065A6",
        "rule": "#24404F",
        "imageMono": False,
        "uddevalla": True,
    },
    "UDDEVALLA BLACK": {
        "bg": "#000000",
        "fg": "#FFFFFF",
        "muted": "#B8C0C5",
        "accent": "#0065A6",
        "rule": "#252525",
        "imageMono": False,
        "uddevalla": True,
    },
    "UDDEVALLA BLUE": {
        "bg": "#F8FAFB",
        "fg": "#173042",
        "muted": "#526774",
        "accent": "#0065A6",
        "rule": "#D8E2E8",
        "imageMono": False,
        "uddevalla": True,
    },
    "UDDEVALLA GREEN": {
        "bg": "#F8FAFB",
        "fg": "#173042",
        "muted": "#526774",
        "accent": "#79AC54",
        "rule": "#DCE7D5",
        "imageMono": False,
        "uddevalla": True,
    },
    "UDDEVALLA YELLOW": {
        "bg": "#F8FAFB",
        "fg": "#173042",
        "muted": "#526774",
        "accent": "#FFCC00",
        "rule": "#EEE5BF",
        "imageMono": False,
        "uddevalla": True,
    },
    "UDDEVALLA RED": {
        "bg": "#F8FAFB",
        "fg": "#173042",
        "muted": "#526774",
        "accent": "#B51726",
        "rule": "#E8D5D8",
        "imageMono": False,
        "uddevalla": True,
    },
    "UDDEVALLA PINK": {
        "bg": "#F8FAFB",
        "fg": "#173042",
        "muted": "#526774",
        "accent": "#E0437C",
        "rule": "#EED8E1",
        "imageMono": False,
        "uddevalla": True,
    },
    "UDDEVALLA PURPLE": {
        "bg": "#F8FAFB",
        "fg": "#173042",
        "muted": "#526774",
        "accent": "#672A61",
        "rule": "#E0D5DF",
        "imageMono": False,
        "uddevalla": True,
    },
}
LAYOUTS = [
    "TITLE",
    "STATEMENT",
    "TEXT",
    "BULLETS",
    "AGENDA",
    "IMAGE",
    "PHOTO",
    "HERO IMAGE",
    "SPLIT",
    "TWO COLUMN",
    "COMPARE",
    "IMAGE + QUOTE",
    "QUOTE",
    "BIG NUMBER",
    "DATA / KPI",
    "NUMBER GRID",
    "PROCESS",
    "TIMELINE",
    "MATRIX",
    "TABLE",
    "SECTION",
    "VIDEO",
    "VIDEO + TEXT",
    "FULL BLEED",
    "END",
    "CODE",
    "IMAGE + CAPTION",
    "THREE COLUMN",
]
UDDEVALLA_ACCENTS = {
    "BLUE": "#0065A6",
    "GREEN": "#79AC54",
    "YELLOW": "#FFCC00",
    "RED": "#B51726",
    "PINK": "#E0437C",
    "PURPLE": "#672A61",
}
DEFAULT = {
    "format": FORMAT,
    "appVersion": VERSION,
    "title": "Untitled",
    "theme": "B&W",
    "showNumbers": False,
    "showLogo": True,
    "fontFamily": "Noto Sans",
    "aspect": "16:9",
    "transition": "NONE",
    "reduceMotion": False,
    "cinematicOpen": True,
    "cinematicClose": True,
    "targetMinutes": 0,
    "footerText": "",
    "footerAlign": "LEFT",
    "footerSize": "MEDIUM",
    "slides": [
        {
            "layout": "TITLE",
            "title": "NIRUPRES",
            "body": "MINIMAL NOIR PRESENTATIONS",
            "image": "",
            "imageMode": "FILL",
            "mono": False,
            "notes": "",
            "focalX": 0.5,
            "focalY": 0.5,
            "imageTreatment": "NATURAL",
        }
    ],
}


def clone(x):
    return json.loads(json.dumps(x))


def system_documents_dir():
    p = QStandardPaths.writableLocation(QStandardPaths.StandardLocation.DocumentsLocation)
    candidate = Path(p).expanduser() if p else Path.home() / "Documents"
    return candidate if candidate.exists() else Path.home()


def load_file_prefs():
    defaults = {
        "defaultFolder": "",
        "rememberLastFolder": True,
        "lastFolder": "",
        "lastFilter": "",
        "assetQuality": "OPTIMIZED",
    }
    try:
        raw = json.loads(FILE_PREFS.read_text(encoding="utf-8")) if FILE_PREFS.exists() else {}
        if isinstance(raw, dict):
            defaults.update({k: raw.get(k, defaults[k]) for k in defaults})
    except Exception:
        pass
    defaults["rememberLastFolder"] = bool(defaults.get("rememberLastFolder", True))
    defaults["assetQuality"] = str(defaults.get("assetQuality", "OPTIMIZED")).upper()
    if defaults["assetQuality"] not in ("OPTIMIZED", "HIGH", "ORIGINAL"):
        defaults["assetQuality"] = "OPTIMIZED"
    for key in ("defaultFolder", "lastFolder"):
        value = str(defaults.get(key, "") or "")
        defaults[key] = value if value and Path(value).expanduser().is_dir() else ""
    return defaults


def save_file_prefs(prefs):
    DATA.mkdir(parents=True, exist_ok=True)
    payload = {
        k: prefs.get(k)
        for k in ("defaultFolder", "rememberLastFolder", "lastFolder", "lastFilter", "assetQuality")
    }
    tmp = FILE_PREFS.with_suffix(".tmp")
    tmp.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    tmp.replace(FILE_PREFS)


def preferred_folder(prefs, current_path=None, prefer_current=True):
    if prefer_current and current_path:
        try:
            parent = Path(current_path).expanduser().resolve().parent
            if parent.is_dir():
                return parent
        except Exception:
            pass
    if prefs.get("rememberLastFolder") and prefs.get("lastFolder"):
        p = Path(prefs["lastFolder"]).expanduser()
        if p.is_dir():
            return p
    if prefs.get("defaultFolder"):
        p = Path(prefs["defaultFolder"]).expanduser()
        if p.is_dir():
            return p
    return system_documents_dir()


def remember_file_folder(prefs, path, selected_filter=""):
    try:
        p = Path(path).expanduser()
        folder = p if p.is_dir() else p.parent
        if prefs.get("rememberLastFolder", True) and folder.is_dir():
            prefs["lastFolder"] = str(folder.resolve())
        if selected_filter:
            prefs["lastFilter"] = selected_filter
        save_file_prefs(prefs)
    except Exception:
        pass


TRANSITIONS = ("NONE", "FADE", "MORPH", "REVEAL")


def normalize_transition(value, allow_deck=False):
    value = str(value or ("DECK" if allow_deck else "NONE")).strip().upper()
    if value == "CUT":
        value = "NONE"
    allowed = (("DECK",) + TRANSITIONS) if allow_deck else TRANSITIONS
    return value if value in allowed else ("DECK" if allow_deck else "NONE")


def slide_defaults(s=None):
    d = {
        "id": str(uuid.uuid4()),
        "layout": "TITLE",
        "title": "",
        "body": "",
        "image": "",
        "imageMode": "FILL",
        "mono": False,
        "notes": "",
        "focalX": 0.5,
        "focalY": 0.5,
        "imageTreatment": "NATURAL",
        "listStyle": "NUMBERS",
        "backgroundImage": "",
        "backgroundDim": 75,
        "backgroundBlur": 0,
        "backgroundMono": False,
        "backgroundFocalX": 0.5,
        "backgroundFocalY": 0.5,
        "themeOverride": "DECK",
        "fontScale": 1.0,
        "align": "LEFT",
        "weight": "AUTO",
        "brightness": 0,
        "contrast": 0,
        "overlay": 0,
        "blur": 0,
        "fontFamily": "DECK",
        "hidden": False,
        "caption": "",
        "imageRequest": "",
        "logoMode": "AUTO",
        "accentOverride": "AUTO",
        "transition": "DECK",
        "leftColumn": None,
        "rightColumn": None,
        "twoColumnStructured": False,
        "sideStructured": False,
        "backgroundMotion": "NONE",
        "videoSource": "",
        "videoPoster": "",
        "videoAutoplay": False,
        "videoMuted": False,
    }
    d.update(s or {})
    d["imageTreatment"] = str(
        ("MONO" if d.get("mono") else "NATURAL")
        if (s is not None and "imageTreatment" not in s)
        else d.get("imageTreatment", "NATURAL")
    ).upper()
    d["imageTreatment"] = (
        d["imageTreatment"]
        if d["imageTreatment"] in ("NATURAL", "MONO", "DIM", "CONTRAST")
        else "NATURAL"
    )
    d["mono"] = d["imageTreatment"] == "MONO"
    d["listStyle"] = (
        str(d.get("listStyle", "NUMBERS")).upper()
        if str(d.get("listStyle", "NUMBERS")).upper() in ("NUMBERS", "DOTS", "DASHES", "NONE")
        else "NUMBERS"
    )
    d["backgroundDim"] = max(0, min(90, int(d.get("backgroundDim", 75) or 0)))
    d["backgroundBlur"] = max(0, min(20, int(d.get("backgroundBlur", 0) or 0)))
    d["backgroundMono"] = bool(d.get("backgroundMono", False))
    d["backgroundFocalX"] = max(0.0, min(1.0, float(d.get("backgroundFocalX", 0.5) or 0.5)))
    d["backgroundFocalY"] = max(0.0, min(1.0, float(d.get("backgroundFocalY", 0.5) or 0.5)))
    d["transition"] = normalize_transition(d.get("transition", "DECK"), True)
    d["backgroundMotion"] = (
        str(d.get("backgroundMotion", "NONE")).upper()
        if str(d.get("backgroundMotion", "NONE")).upper() in ("NONE", "KEN BURNS")
        else "NONE"
    )
    d["logoMode"] = (
        str(d.get("logoMode", "AUTO")).upper()
        if str(d.get("logoMode", "AUTO")).upper() in ("AUTO", "ON", "OFF")
        else "AUTO"
    )
    d["accentOverride"] = (
        str(d.get("accentOverride", "AUTO")).upper()
        if str(d.get("accentOverride", "AUTO")).upper() in ("AUTO", *UDDEVALLA_ACCENTS.keys())
        else "AUTO"
    )
    return d


def normalize_footer_align(value):
    value = str(value or "LEFT").strip().upper()
    return value if value in ("LEFT", "CENTER", "RIGHT") else "LEFT"


def normalize_footer_size(value):
    value = str(value or "MEDIUM").strip().upper()
    return value if value in ("SMALL", "MEDIUM", "LARGE") else "MEDIUM"


def normalize_theme(name):
    # Compatibility aliases stay readable while duplicate themes stay out of the UI.
    name = str(name or "B&W").upper()
    if name in ("UDDEVALLA", "UDDEVALLA LIGHT"):
        return "UDDEVALLA BLUE"
    if name == "CL":
        return "CARL LARSSON"
    return name


def normalize_themes(data):
    data["theme"] = normalize_theme(data.get("theme", "B&W"))
    if data["theme"] not in THEMES:
        data["theme"] = "B&W"
    for slide in data.get("slides", []):
        ov = normalize_theme(slide.get("themeOverride", "DECK"))
        slide["themeOverride"] = ov if ov == "DECK" or ov in THEMES else "DECK"
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
    text_width = button.fontMetrics().horizontalAdvance(button.text().replace("&", ""))
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
    box = QMessageBox(parent)
    box.setWindowTitle(title)
    box.setText(text)
    box.setIcon(QMessageBox.NoIcon)
    box.setStandardButtons(buttons)
    if default is not None:
        try:
            box.setDefaultButton(default)
        except TypeError:
            pass
    neutralize_button_box(box)
    # QMessageBox may otherwise keep a compact platform-derived width and squeeze
    # long standard-button labels. Reserve enough width for every action + gaps.
    action_width = (
        sum(b.minimumWidth() for b in box.buttons()) + max(0, len(box.buttons()) - 1) * 8 + 32
    )
    box.setMinimumWidth(max(380, action_width))
    return box


def message_info(parent, title, text):
    return make_message_box(parent, title, text, QMessageBox.Ok).exec()


def message_warning(parent, title, text):
    return make_message_box(parent, title, text, QMessageBox.Ok).exec()


def message_critical(parent, title, text):
    return make_message_box(parent, title, text, QMessageBox.Ok).exec()


def message_question(parent, title, text, buttons, default=None):
    return make_message_box(parent, title, text, buttons, default).exec()


def _safe_zip_name(name):
    name = str(name).replace("\\", "/")
    if not name or name.startswith("/") or re.match(r"^[A-Za-z]:", name):
        return False
    parts = Path(name).parts
    return ".." not in parts and all(x not in ("", ".") for x in parts)


def _validate_project_data(data):
    if not isinstance(data, dict):
        raise ValueError("Invalid project: project.json must contain an object.")
    slides = data.get("slides", [])
    if not isinstance(slides, list):
        raise ValueError("Invalid project: slides must be a list.")
    if len(slides) > 2000:
        raise ValueError("Project contains too many slides (maximum 2000).")
    for i, slide in enumerate(slides):
        if not isinstance(slide, dict):
            raise ValueError(f"Invalid slide {i+1}.")
        for key in (
            "title",
            "body",
            "notes",
            "caption",
            "imageRequest",
            "image",
            "backgroundImage",
            "videoSource",
            "videoPoster",
        ):
            value = slide.get(key, "")
            if not isinstance(value, str):
                raise ValueError(f"Invalid {key} on slide {i+1}.")
            if len(value) > 1_000_000:
                raise ValueError(f"Slide {i+1} contains an excessively large text field.")
    return data


def load_project(path, extract_dir):
    p = Path(path)
    if zipfile.is_zipfile(p):
        root = Path(extract_dir).resolve()
        assets = 0
        total = 0
        project_bytes = None
        with zipfile.ZipFile(p) as z:
            infos = z.infolist()
            if len(infos) > MAX_ASSETS + 8:
                raise ValueError("Project package contains too many files.")
            for info in infos:
                name = info.filename.replace("\\", "/")
                if not _safe_zip_name(name):
                    raise ValueError(f"Unsafe path in project package: {name}")
                mode = (info.external_attr >> 16) & 0xFFFF
                ftype = stat.S_IFMT(mode)
                if stat.S_ISLNK(mode) or (ftype and ftype not in (stat.S_IFREG, stat.S_IFDIR)):
                    raise ValueError(f"Unsupported file type in package: {name}")
                if name != "project.json" and not name.startswith("assets/"):
                    if not name.endswith("/"):
                        raise ValueError(f"Unexpected file in project package: {name}")
                if name.startswith("assets/") and not name.endswith("/"):
                    assets += 1
                total += info.file_size
                if total > MAX_PACKAGE_UNPACKED:
                    raise ValueError("Project package is too large when unpacked.")
                if name == "project.json":
                    if info.file_size > MAX_PROJECT_JSON:
                        raise ValueError("project.json is too large.")
                    project_bytes = z.read(info)
            if project_bytes is None:
                raise ValueError("Invalid NIRUPRES package: project.json is missing.")
            if assets > MAX_ASSETS:
                raise ValueError(f"Project contains too many assets (maximum {MAX_ASSETS}).")
            data = json.loads(project_bytes.decode("utf-8"))
            _validate_project_data(data)
            for info in infos:
                name = info.filename.replace("\\", "/")
                if not name.startswith("assets/") or name.endswith("/"):
                    continue
                target = (root / name).resolve()
                if root not in target.parents:
                    raise ValueError(f"Unsafe asset path: {name}")
                target.parent.mkdir(parents=True, exist_ok=True)
                with z.open(info) as src, target.open("wb") as dst:
                    shutil.copyfileobj(src, dst, 1024 * 1024)
        for slide in data.get("slides", []):
            for field in ("image", "backgroundImage", "videoPoster", "videoSource"):
                image = str(slide.get(field, ""))
                if image.startswith("asset:"):
                    rel = image[6:].replace("\\", "/")
                    if not _safe_zip_name(rel) or "/" in rel:
                        slide[field] = ""
                    else:
                        slide[field] = str(root / "assets" / rel)
    else:
        if p.stat().st_size > MAX_PROJECT_JSON:
            raise ValueError("Project JSON is too large.")
        data = _validate_project_data(json.loads(p.read_text(encoding="utf-8")))
    data.setdefault("title", p.stem)
    data.setdefault("theme", "B&W")
    data.setdefault("showNumbers", False)
    data.setdefault("showLogo", True)
    data.setdefault("footerText", "")
    data.setdefault("footerAlign", "LEFT")
    data.setdefault("footerSize", "MEDIUM")
    data["footerAlign"] = normalize_footer_align(data.get("footerAlign"))
    data["footerSize"] = normalize_footer_size(data.get("footerSize"))
    data.setdefault("fontFamily", "Noto Sans")
    data.setdefault("aspect", "16:9")
    data.setdefault("transition", "NONE")
    data.setdefault("reduceMotion", False)
    data.setdefault("cinematicOpen", True)
    data.setdefault("cinematicClose", True)
    data["transition"] = normalize_transition(data.get("transition", "NONE"))
    data["slides"] = [slide_defaults(x) for x in data.get("slides", [])] or [
        slide_defaults(DEFAULT["slides"][0])
    ]
    normalize_themes(data)
    data["format"] = FORMAT
    data["appVersion"] = VERSION
    return data


def _cached_pixmap(path, target_size):
    global _IMAGE_CACHE_BYTES
    try:
        p = Path(path)
        st = p.stat()
        tw = max(64, int(target_size.width()))
        th = max(64, int(target_size.height()))
        bucket = (min(4096, ((tw + 255) // 256) * 256), min(4096, ((th + 255) // 256) * 256))
        key = (str(p.resolve()), st.st_mtime_ns, st.st_size, bucket)
        if key in _IMAGE_CACHE:
            pm, cost = _IMAGE_CACHE.pop(key)
            _IMAGE_CACHE[key] = (pm, cost)
            return pm
        reader = QImageReader(str(p))
        reader.setAutoTransform(True)
        sz = reader.size()
        if not sz.isValid() or sz.width() <= 0 or sz.height() <= 0:
            return QPixmap()
        if sz.width() * sz.height() > MAX_IMAGE_PIXELS:
            return QPixmap()
        scale = min(1.0, max(bucket[0] / sz.width(), bucket[1] / sz.height()) * 1.15)
        if scale < 1.0:
            reader.setScaledSize(
                QSize(max(1, int(sz.width() * scale)), max(1, int(sz.height() * scale)))
            )
        img = reader.read()
        if img.isNull():
            return QPixmap()
        pm = QPixmap.fromImage(img)
        cost = max(1, pm.width() * pm.height() * 4)
        _IMAGE_CACHE[key] = (pm, cost)
        _IMAGE_CACHE_BYTES += cost
        while _IMAGE_CACHE and _IMAGE_CACHE_BYTES > IMAGE_CACHE_BUDGET:
            _, (_, oldcost) = _IMAGE_CACHE.popitem(last=False)
            _IMAGE_CACHE_BYTES -= oldcost
        return pm
    except Exception:
        return QPixmap()


def _image_dimensions(path):
    try:
        reader = QImageReader(str(path))
        reader.setAutoTransform(True)
        sz = reader.size()
        return (sz.width(), sz.height()) if sz.isValid() else (0, 0)
    except Exception:
        return (0, 0)


def _optimize_asset(src, out_dir, quality_mode="OPTIMIZED"):
    """Create an embedded presentation asset without touching the source file.

    OPTIMIZED targets normal presentation/export use, HIGH keeps extra headroom for
    4K displays, and ORIGINAL is byte-for-byte. The full frame is retained so a
    later focal-point change never loses source content.
    """
    src = Path(src)
    quality_mode = str(quality_mode or "OPTIMIZED").upper()
    if quality_mode == "ORIGINAL":
        data = src.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        ext = src.suffix.lower() or ".bin"
        dst = Path(out_dir) / (digest[:20] + ext)
        dst.write_bytes(data)
        return dst, len(data), len(data), _image_dimensions(src), _image_dimensions(src)
    reader = QImageReader(str(src))
    reader.setAutoTransform(True)
    sz = reader.size()
    if (
        not sz.isValid()
        or sz.width() <= 0
        or sz.height() <= 0
        or sz.width() * sz.height() > MAX_IMAGE_PIXELS
    ):
        raise ValueError(f"Unsupported or excessively large image: {src.name}")
    img = reader.read()
    if img.isNull():
        raise ValueError(f"Could not decode image: {src.name}")
    before = (img.width(), img.height())
    max_edge = 4096 if quality_mode == "HIGH" else 3200
    if max(before) > max_edge:
        img = img.scaled(max_edge, max_edge, Qt.KeepAspectRatio, Qt.SmoothTransformation)
    suffix = src.suffix.lower()
    has_alpha = img.hasAlphaChannel()
    # Preserve PNG/transparency and line-art formats; photographic JPEG/WebP is
    # encoded as high-quality JPEG for a much smaller portable deck.
    if has_alpha or suffix in (".png", ".bmp"):
        ext = ".png"
        fmt = "PNG"
        q = 95
    else:
        ext = ".jpg"
        fmt = "JPEG"
        q = 94 if quality_mode == "HIGH" else 88
    tmp = Path(out_dir) / ("asset-tmp" + ext)
    if not img.save(str(tmp), fmt, q):
        raise ValueError(f"Could not encode image: {src.name}")
    data = tmp.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    dst = Path(out_dir) / (digest[:20] + ext)
    if dst.exists():
        tmp.unlink()
    else:
        tmp.replace(dst)
    return dst, src.stat().st_size, dst.stat().st_size, before, (img.width(), img.height())


def _paired_font(texts, family, weight, max_px, min_px, rects):
    """Fit equivalent regions as one typographic system, not independently.

    2.3: pixel sizes (QFont.pixelSize), measured with the same layout the
    renderer paints, so the result does not depend on the screen DPI.
    """
    sizes = [
        textfit.fit_size(text, family, weight, max_px, min_px, rect)
        for text, rect in zip(texts, rects)
        if str(text or "").strip()
    ]
    if any(z is None for z in sizes):
        size = min_px
    else:
        size = min(sizes) if sizes else max_px
    return textfit.px_font(family, size, weight)


def _parse_two_column_body(body):
    """Split TWO COLUMN into two literal text columns.

    `|||` is the portable/Markdown column delimiter. Blank lines are content
    and therefore stay inside their column. For legacy decks without an
    explicit delimiter only, the historical first-blank split is retained so
    they can still be opened and then normalized by the structured editor.
    No line is ever interpreted as a heading by TWO COLUMN.
    """
    raw = str(body or "").replace("\r\n", "\n").replace("\r", "\n")
    explicit = False
    m = re.search(r"^\s*\|\|\|\s*$", raw, re.M)
    if m:
        left, right = raw[: m.start()], raw[m.end() :]
        explicit = True
    else:
        m = re.search(r"^\s*\|\|\s*$", raw, re.M)
        if m:
            left, right = raw[: m.start()], raw[m.end() :]
            explicit = True
        else:
            parts = re.split(r"\n\s*\n", raw, maxsplit=1)
            left = parts[0] if parts else ""
            right = parts[1] if len(parts) > 1 else ""

    def column(text):
        # Preserve paragraph breaks exactly. Strip only outer whitespace so
        # editor -> save -> render is deterministic and WYSIWYM-like.
        return {"heading": "", "groups": [], "text": str(text or "").strip(), "grouped": False}

    return [column(left), column(right)], explicit


def grayscale(pm):
    """Return a grayscale copy using the Qt 6 enum API. Never mutate the source pixmap."""
    if pm.isNull():
        return pm
    img = pm.toImage()
    fmt = getattr(QImage.Format, "Format_Grayscale8", None)
    if fmt is None:
        return pm
    converted = img.convertToFormat(fmt)
    out = QPixmap.fromImage(converted)
    return out if not out.isNull() else pm


def _plain_markup(text):
    return re.sub(r"(?<!\\)(\*\*|__|\*|_)", "", str(text or ""))


def _inline_html(text):
    import html

    value = html.escape(str(text or ""))
    # Combined emphasis must be parsed before bold/italic so ***text*** is
    # deterministic rather than leaving a stray marker after the bold pass.
    value = re.sub(r"\*\*\*([^*\n]+?)\*\*\*", r"<b><i>\1</i></b>", value)
    value = re.sub(r"___([^_\n]+?)___", r"<b><i>\1</i></b>", value)
    value = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", value)
    value = re.sub(r"__(.+?)__", r"<b>\1</b>", value)
    value = re.sub(r"(?<!\*)\*([^*\n]+?)\*(?!\*)", r"<i>\1</i>", value)
    value = re.sub(r"(?<!_)_([^_\n]+?)_(?!_)", r"<i>\1</i>", value)
    return value.replace("\n", "<br>")


def _format_markdown_selection(text, kind):
    """Apply/toggle inline Markdown emphasis without crossing line breaks.

    QTextEdit selections can include the paragraph separator at the end of a
    line. Wrapping that entire selection used to create dangling `*` markers on
    the following line. Each selected line is therefore formatted separately.
    """
    text = str(text or "").replace("\u2029", "\n")
    if kind == "normal":
        return _plain_markup(text)
    marker = "**" if kind == "bold" else "*"
    out = []
    for line in text.split("\n"):
        if not line.strip():
            out.append(line)
            continue
        leading = line[: len(line) - len(line.lstrip())]
        trailing = line[len(line.rstrip()) :]
        core = line.strip()
        if kind == "bold" and core.startswith("**") and core.endswith("**") and len(core) >= 4:
            core = core[2:-2]
        elif (
            kind == "italic"
            and core.startswith("*")
            and core.endswith("*")
            and not core.startswith("**")
            and not core.endswith("**")
            and len(core) >= 2
        ):
            core = core[1:-1]
        else:
            core = marker + core + marker
        out.append(leading + core + trailing)
    return "\n".join(out)


def draw_text(p, rect, text, font, color, flags=Qt.AlignLeft | Qt.AlignVCenter | Qt.TextWordWrap):
    text = str(text or "")
    if not re.search(
        r"(\*\*\*.+?\*\*\*|\*\*.+?\*\*|__.+?__|(?<!\*)\*[^*\n]+?\*(?!\*)|(?<!_)_[^_\n]+?_(?!_))",
        text,
    ):
        p.setFont(font)
        p.setPen(QColor(color))
        p.drawText(rect, flags, text)
        return
    doc = QTextDocument()
    doc.setDocumentMargin(0)
    doc.setDefaultFont(font)
    align = "center" if flags & Qt.AlignHCenter else ("right" if flags & Qt.AlignRight else "left")
    doc.setHtml(
        f'<div style="color:{color}; text-align:{align}; white-space:pre-wrap">{_inline_html(text)}</div>'
    )
    doc.setTextWidth(rect.width())
    natural = doc.size().height()
    y = rect.y()
    if flags & Qt.AlignVCenter:
        y += max(0, (rect.height() - natural) / 2)
    elif flags & Qt.AlignBottom:
        y += max(0, rect.height() - natural)
    p.save()
    p.translate(rect.x(), y)
    p.setClipRect(QRectF(0, 0, rect.width(), rect.height()))
    ctx = QAbstractTextDocumentLayout.PaintContext()
    doc.documentLayout().draw(p, ctx)
    p.restore()


def fit_font(text, family, weight, max_px, min_px, rect, max_lines=3):
    """Largest pixel-sized font in [min_px, max_px] that fits ``max_lines`` in ``rect``."""
    size = textfit.fit_size(text, family, weight, max_px, min_px, rect, max_lines)
    return textfit.px_font(family, size or min_px, weight)


def draw_image(
    p,
    rect,
    path,
    mode="FILL",
    mono=False,
    fx=0.5,
    fy=0.5,
    opacity=1.0,
    brightness=0,
    contrast=0,
    overlay=0,
    blur=0,
    zoom=1.0,
):
    """Render an image defensively. A bad transform must never break the outer paint cycle."""
    if not path or not Path(path).exists():
        return False
    target = rect.toRect()
    if target.width() <= 0 or target.height() <= 0:
        return False
    pm = _cached_pixmap(path, target.size())
    if pm.isNull():
        return False
    try:
        if mono:
            pm = grayscale(pm)
        if blur:
            try:
                scene = QGraphicsScene()
                item = QGraphicsPixmapItem(pm)
                eff = QGraphicsBlurEffect()
                eff.setBlurRadius(float(blur))
                item.setGraphicsEffect(eff)
                scene.addItem(item)
                out = QPixmap(pm.size())
                out.fill(Qt.transparent)
                qp = QPainter(out)
                try:
                    scene.render(qp, QRectF(out.rect()), QRectF(pm.rect()))
                finally:
                    if qp.isActive():
                        qp.end()
                if not out.isNull():
                    pm = out
            except Exception:
                pass  # keep the unblurred pixmap
        aspect = Qt.KeepAspectRatioByExpanding if mode == "FILL" else Qt.KeepAspectRatio
        zoom = max(1.0, min(1.18, float(zoom or 1.0))) if mode == "FILL" else 1.0
        scale_size = QSize(max(1, int(target.width() * zoom)), max(1, int(target.height() * zoom)))
        scaled = pm.scaled(scale_size, aspect, Qt.SmoothTransformation)
        if scaled.isNull():
            return False
        if mode == "FILL":
            maxx = max(0, scaled.width() - target.width())
            maxy = max(0, scaled.height() - target.height())
            sx = int(maxx * max(0, min(1, fx)))
            sy = int(maxy * max(0, min(1, fy)))
            scaled = scaled.copy(
                sx,
                sy,
                min(target.width(), scaled.width() - sx),
                min(target.height(), scaled.height() - sy),
            )
            dest = target
        else:
            dest = QRectF(
                target.x() + (target.width() - scaled.width()) / 2,
                target.y() + (target.height() - scaled.height()) / 2,
                scaled.width(),
                scaled.height(),
            ).toRect()
        p.save()
        try:
            p.setOpacity(max(0.0, min(1.0, float(opacity))))
            p.drawPixmap(dest, scaled)
            p.setOpacity(1.0)
            if brightness:
                p.fillRect(
                    dest,
                    (
                        QColor(255, 255, 255, min(150, int(brightness * 1.5)))
                        if brightness > 0
                        else QColor(0, 0, 0, min(150, int(-brightness * 1.5)))
                    ),
                )
            if contrast > 0:
                p.setCompositionMode(QPainter.CompositionMode_Overlay)
                p.fillRect(dest, QColor(255, 255, 255, min(100, int(contrast))))
                p.setCompositionMode(QPainter.CompositionMode_SourceOver)
            if overlay:
                p.fillRect(dest, QColor(0, 0, 0, min(220, int(overlay * 2.2))))
        finally:
            p.setOpacity(1.0)
            p.setCompositionMode(QPainter.CompositionMode_SourceOver)
            p.restore()
        return True
    except Exception as exc:
        print(f"NIRUPRES image render warning: {exc}", file=sys.stderr)
        return False


def timeline_cells(safe_rect, count, yline):
    """Return bounded TIMELINE node/text geometry for 1–5 milestones.

    Geometry is deliberately independent of slide content so old decks benefit
    automatically from renderer improvements without migration.
    """
    count = max(1, min(5, int(count or 1)))
    cell_w = safe_rect.width() / count
    pad = min(cell_w * 0.08, safe_rect.width() * 0.012)
    head_h = safe_rect.height() * 0.075
    desc_h = safe_rect.height() * 0.125
    out = []
    for i in range(count):
        left = safe_rect.x() + i * cell_w
        inner = QRectF(left + pad, safe_rect.y(), max(1, cell_w - 2 * pad), safe_rect.height())
        xx = left + cell_w * 0.5
        head = QRectF(inner.x(), yline - safe_rect.height() * 0.17, inner.width(), head_h)
        desc = QRectF(inner.x(), yline + safe_rect.height() * 0.055, inner.width(), desc_h)
        out.append((xx, head, desc))
    return out


def _clean_items(body, strip_markers=True):
    out = []
    for q in str(body or "").splitlines():
        q = q.strip()
        if not q:
            continue
        if strip_markers:
            q = re.sub(r"^(?:[-•–—]\s+|\*\s+)", "", q)
        out.append(q)
    return out


def _take(items, cap, label):
    """Show at most ``cap`` items; when more exist the last slot says "+N more"."""
    if len(items) <= cap:
        return items, 0
    shown = items[: max(0, cap - 1)]
    hidden = len(items) - len(shown)
    textfit.note("hidden", label=label, count=hidden)
    return shown, hidden


def _parse_columns(body, count):
    """Split a body into ``count`` literal columns on lines that are only `|||`."""
    raw = str(body or "").replace("\r\n", "\n").replace("\r", "\n")
    parts = re.split(r"^\s*\|\|\|?\s*$", raw, flags=re.M)
    parts = [x.strip() for x in parts]
    if len(parts) == 1 and count > 1:
        parts = [x.strip() for x in re.split(r"\n\s*\n", raw.strip())]
    if len(parts) > count:
        parts = parts[: count - 1] + ["\n\n".join(parts[count - 1 :])]
    return parts + [""] * (count - len(parts))


def _table_rows(body):
    rows = []
    for q in str(body or "").splitlines():
        q = q.strip()
        if not q or re.fullmatch(r"[\s|:\-+]+", q):
            continue
        if q.startswith("|"):
            q = q[1:]
        if q.endswith("|") and not q.endswith("\\|"):
            q = q[:-1]
        rows.append([c.strip() for c in q.split("|")])
    return rows


PT_TO_PX = 96.0 / 72.0


def render_slide(
    p,
    rect,
    s,
    theme_name,
    index=0,
    total=1,
    show_numbers=True,
    deck_font="Noto Sans",
    show_logo=True,
    footer_text="",
    footer_align="LEFT",
    footer_size="MEDIUM",
    report=None,
):
    if report is not None:
        with textfit.collect(report, textfit._TRACE):
            return render_slide(
                p, rect, s, theme_name, index, total, show_numbers, deck_font, show_logo,
                footer_text, footer_align, footer_size,
            )
    theme_name = (
        s.get("themeOverride", "DECK") if s.get("themeOverride", "DECK") != "DECK" else theme_name
    )
    theme_name = normalize_theme(theme_name)
    t = dict(THEMES.get(theme_name, THEMES["B&W"]))
    accent_override = str(s.get("accentOverride", "AUTO")).upper()
    t["accent"] = UDDEVALLA_ACCENTS.get(accent_override, t["accent"])
    p.save()
    p.setRenderHint(QPainter.Antialiasing)
    p.setRenderHint(QPainter.SmoothPixmapTransform)
    p.setRenderHint(QPainter.TextAntialiasing)
    p.fillRect(rect, QColor(t["bg"]))
    x, y, w, h = rect.x(), rect.y(), rect.width(), rect.height()
    m = w * 0.055
    top = h * 0.055
    bgimg = str(s.get("backgroundImage", "") or "")
    if bgimg:
        bgfx = float(s.get("backgroundFocalX", 0.5))
        bgfy = float(s.get("backgroundFocalY", 0.5))
        bgdim = max(0, min(90, int(s.get("backgroundDim", 75))))
        bgblur = max(0, min(20, int(s.get("backgroundBlur", 0))))
        draw_image(
            p, rect, bgimg, "FILL", bool(s.get("backgroundMono", False)), bgfx, bgfy, 1.0,
            0, 0, 0, bgblur, float(s.get("_backgroundZoom", 1.0) or 1.0),
        )
        veil = QColor(t["bg"])
        veil.setAlpha(int(255 * bgdim / 100))
        p.fillRect(rect, veil)
    layout = s.get("layout", "TITLE")
    title = s.get("title", "")
    body = s.get("body", "")
    img = s.get("image", "")
    treatment = str(s.get("imageTreatment", "MONO" if s.get("mono") else "NATURAL")).upper()
    mono = (treatment == "MONO") or t["imageMono"]
    mode = s.get("imageMode", "FILL")
    fx = s.get("focalX", 0.5)
    fy = s.get("focalY", 0.5)
    br = s.get("brightness", 0)
    ct = s.get("contrast", 0)
    ov = s.get("overlay", 0)
    bl = s.get("blur", 0)
    br = br - 10 if treatment == "DIM" else br
    ov = max(ov, 18) if treatment == "DIM" else ov
    ct = max(ct, 24) if treatment == "CONTRAST" else ct
    sans = s.get("fontFamily", "DECK")
    sans = deck_font if sans == "DECK" else sans
    mono_font = "Noto Sans Mono"
    try:
        scale = max(0.5, min(2.0, float(s.get("fontScale", 1.0) or 1.0)))
    except (TypeError, ValueError):
        scale = 1.0
    align_name = s.get("align", "LEFT")
    base_align = {"LEFT": Qt.AlignLeft, "CENTER": Qt.AlignHCenter, "RIGHT": Qt.AlignRight}.get(
        align_name, Qt.AlignLeft
    )
    L, T, VC, HC, R, B = Qt.AlignLeft, Qt.AlignTop, Qt.AlignVCenter, Qt.AlignHCenter, Qt.AlignRight, Qt.AlignBottom

    def px(frac, floor=1):
        # Font sizes are pixels. Templates were designed in points on a 96 dpi
        # canvas (1 pt = 4/3 px); keep that look on every device and DPI.
        return max(floor, h * frac * PT_TO_PX)

    def text(r, txt, maxp, minp, color=None, weight=QFont.Normal, align=L | VC, lines=None,
             label="text", family=None, markup=True, wrap=True, size=None, scaled=True):
        k = scale if scaled else 1.0
        return textfit.fit_draw(
            p, r, txt, family or sans, weight, px(maxp * k), px(minp * k, 1),
            QColor(color or t["fg"]), align, lines, label, markup, wrap, size,
        )

    def label_text(r, txt, frac, color, family=None, weight=QFont.Normal, align=L | VC):
        # Single-line metadata (markers, numbers): shrink a little, then elide.
        return textfit.fit_draw(
            p, r, txt, family or mono_font, weight, px(frac, 6), px(frac * 0.6, 5),
            QColor(color), align, 1, "label", False,
        )

    hero_weight = {
        "LIGHT": QFont.Light,
        "REGULAR": QFont.Normal,
        "BOLD": QFont.Bold,
        "AUTO": QFont.Black,
    }.get(s.get("weight", "AUTO"), QFont.Black)

    def hero(r, txt, maxp=0.082, minp=0.038, align=None, lines=3):
        a = align if align is not None else (base_align | VC)
        return text(r, txt, maxp, minp, t["fg"], hero_weight, a, lines, "title")

    def bodytxt(r, txt, align=None, maxp=0.027, minp=0.016, color=None, lines=None, label="body"):
        a = align if align is not None else (L | VC)
        if align is None and base_align != Qt.AlignLeft:
            a = base_align | VC
        return text(r, txt, maxp, minp, color or t["muted"], QFont.Normal, a, lines, label)

    def more(r, hidden, align=L | VC):
        if hidden:
            text(r, textfit.more_label(hidden), 0.020, 0.012, t["muted"], QFont.Medium, align, 1,
                 "more", markup=False, scaled=False)

    def paired(texts, rects, maxp, minp, weight=QFont.Normal, lines=None):
        sizes = [
            textfit.fit_size(tx, sans, weight, px(maxp * scale), px(minp * scale), r, lines)
            for tx, r in zip(texts, rects)
            if str(tx or "").strip()
        ]
        if not sizes:
            return int(px(maxp * scale))
        if any(z is None for z in sizes):
            return int(px(minp * scale))
        return min(sizes)

    def accent_for(n=0):
        colors = t.get("profile") or [t["accent"]]
        return colors[n % len(colors)]

    def image(r, path, img_mode=None, opacity=1.0):
        return draw_image(p, r, path, img_mode or mode, mono, fx, fy, opacity, br, ct, ov, bl)

    if show_numbers and layout not in ("FULL BLEED",):
        label_text(QRectF(x + m, y + top, w * 0.3, h * 0.035), f"{index+1:02d} / {total:02d}",
                   0.013, t["muted"])

    if layout in ("VIDEO", "VIDEO + TEXT"):
        hero(QRectF(x + m, y + h * 0.08, w * 0.82, h * 0.12), title, 0.045, 0.024, lines=2)
        vr = (
            QRectF(x + m, y + h * 0.23, w - 2 * m, h * 0.58)
            if layout == "VIDEO"
            else QRectF(x + m, y + h * 0.25, w * 0.56, h * 0.52)
        )
        poster = s.get("videoPoster") or img
        if poster:
            draw_image(p, vr, poster, "FILL", mono, fx, fy, 1.0, br, ct, ov, bl)
        else:
            p.fillRect(vr, QColor(t["rule"]))
        p.setPen(QColor(t["fg"]))
        p.setBrush(Qt.NoBrush)
        p.drawRect(vr)
        text(QRectF(vr.center().x() - w * 0.04, vr.center().y() - h * 0.05, w * 0.08, h * 0.10),
             "▶", 0.055, 0.03, t["fg"], QFont.Bold, HC | VC, 1, "label", markup=False, scaled=False)
        if layout == "VIDEO + TEXT":
            bodytxt(QRectF(x + w * 0.66, y + h * 0.25, w * 0.285, h * 0.52), body, L | T,
                    0.024, 0.014, t["fg"])
        elif body:
            bodytxt(QRectF(x + m, y + h * 0.825, w - 2 * m, h * 0.10), body, L | VC, 0.020,
                    0.012, lines=3)
    elif layout == "FULL BLEED":
        image(rect, img, "FILL")
        p.fillRect(rect, QColor(0, 0, 0, 105))
        hero(QRectF(x + m, y + h * 0.45, w - 2 * m, h * 0.34), title, 0.085, 0.036,
             base_align | B, 3)
        if body:
            text(QRectF(x + m, y + h * 0.81, w - 2 * m, h * 0.11), body, 0.025, 0.014,
                 "#E8E8E8", QFont.Normal, base_align | T, 3, "body")
    elif layout in ("AGENDA", "BULLETS"):
        agenda = layout == "AGENDA"
        if agenda:
            hero(QRectF(x + m, y + h * 0.10, w * 0.80, h * 0.15), title, 0.052, 0.028, lines=2)
            top_y, area_h, cap = y + h * 0.30, h * 0.58, 6
        else:
            hero(QRectF(x + m, y + h * 0.11, w * 0.82, h * 0.17), title, 0.058, 0.028, lines=2)
            p.fillRect(QRectF(x + m, y + h * 0.30, w * 0.055, max(2, h * 0.0035)),
                       QColor(t["accent"]))
            top_y, area_h, cap = y + h * 0.345, h * 0.545, 7
        items, hidden = _take(_clean_items(body), cap, "items" if agenda else "bullets")
        slots = len(items) + (1 if hidden else 0)
        step = min(h * (0.092 if agenda else 0.079), area_h / max(1, slots))
        style = str(s.get("listStyle", "NUMBERS")).upper()
        marker_w = w * (0.07 if agenda else 0.045)
        has_marker = style != "NONE"
        tx = x + m + ((w * 0.13 if agenda else w * 0.055) if has_marker else 0)
        tw = (x + w - m) - tx - w * (0.08 if agenda else 0.06)
        boxes = [QRectF(tx, top_y + n * step, tw, step * 0.86) for n in range(len(items))]
        size = paired(items, boxes, 0.032, 0.017, QFont.Medium if agenda else QFont.Normal, 3)
        for n, item in enumerate(items):
            yy = top_y + n * step
            marker = {"NUMBERS": f"{n+1:02d}", "DOTS": "•", "DASHES": "—", "NONE": ""}.get(
                style, f"{n+1:02d}"
            )
            if marker:
                label_text(QRectF(x + m, yy, marker_w, step * 0.86), marker,
                           0.025 if agenda else 0.016, accent_for(n))
                if agenda:
                    p.fillRect(
                        QRectF(x + m + w * 0.075, yy + step * 0.43, w * 0.035, max(1, h * 0.002)),
                        QColor(t["rule"]),
                    )
            text(boxes[n], item, 0.032, 0.017, t["fg"], QFont.Medium if agenda else QFont.Normal,
                 L | VC, 3, "item", size=size)
        if hidden:
            more(QRectF(tx, top_y + len(items) * step, tw, step * 0.86), hidden)
    elif layout == "HERO IMAGE":
        ir = QRectF(x + w * 0.40, y, w * 0.60, h)
        if image(ir, img, "FILL"):
            p.fillRect(QRectF(x + w * 0.40, y, w * 0.10, h), QColor(t["bg"]))
        hero(QRectF(x + m, y + h * 0.16, w * 0.39, h * 0.38), title, 0.075, 0.032,
             base_align | B, 4)
        p.fillRect(QRectF(x + m, y + h * 0.57, w * 0.08, max(2, h * 0.005)), QColor(t["accent"]))
        bodytxt(QRectF(x + m, y + h * 0.61, w * 0.31, h * 0.27), body, L | T, 0.024, 0.014)
    elif layout in ("COMPARE", "TWO COLUMN", "THREE COLUMN"):
        three = layout == "THREE COLUMN"
        hero(QRectF(x + m, y + h * 0.10, w * 0.82, h * 0.15), title, 0.052, 0.028, lines=2)
        if three:
            texts = _parse_columns(body, 3)
        elif (
            bool(s.get("sideStructured") or s.get("twoColumnStructured"))
            and s.get("leftColumn") is not None
            and s.get("rightColumn") is not None
        ):
            texts = [str(s.get("leftColumn") or "").strip(), str(s.get("rightColumn") or "").strip()]
        else:
            cols, _ = _parse_two_column_body(body)
            texts = [cols[0]["text"], cols[1]["text"]]
        n = len(texts)
        gutter = w * (0.045 if three else (0.060 if layout == "COMPARE" else 0.070))
        cw = (w - 2 * m - gutter * (n - 1)) / n
        ctop = y + h * (0.33 if layout != "TWO COLUMN" else 0.345)
        ch = h * 0.55
        rects = [QRectF(x + m + i * (cw + gutter), ctop, cw, ch) for i in range(n)]
        size = paired(texts, rects, 0.027, 0.0145)
        for i, (tx_, r) in enumerate(zip(texts, rects)):
            text(r, tx_, 0.027, 0.0145, t["fg"], QFont.Normal, L | T, None, "column", size=size)
        if layout == "TWO COLUMN":
            p.fillRect(QRectF(x + m, y + h * 0.285, w * 0.065, max(2, h * 0.0035)),
                       QColor(t["accent"]))
            p.fillRect(QRectF(x + w * 0.5 - w * 0.0006, ctop, max(1, w * 0.0012), ch),
                       QColor(t["rule"]))
        else:
            for i, r in enumerate(rects):
                p.fillRect(QRectF(r.x(), ctop - h * 0.022, r.width(), max(2, h * 0.004)),
                           QColor(accent_for(i) if three else t["accent"]))
    elif layout == "DATA / KPI":
        hero(QRectF(x + m, y + h * 0.08, w * 0.80, h * 0.12), title, 0.043, 0.024, lines=2)
        items, hidden = _take([q.strip() for q in body.splitlines() if q.strip()], 4, "metrics")
        primary = items[0].split("|", 1) if items else ["—", "Primary metric"]
        num = primary[0].strip()
        lab = primary[1].strip() if len(primary) > 1 else ""
        text(QRectF(x + m, y + h * 0.24, w * 0.48, h * 0.25), num, 0.20, 0.035, t["accent"],
             QFont.Black, L | VC, 1, "number", markup=False, wrap=False)
        bodytxt(QRectF(x + m, y + h * 0.50, w * 0.45, h * 0.18), lab, L | T, 0.025, 0.014,
                t["fg"], 4)
        rest = items[1:]
        for n, item in enumerate(rest):
            a = item.split("|", 1)
            val = a[0].strip()
            label = a[1].strip() if len(a) > 1 else ""
            ry = y + h * (0.25 + n * 0.17)
            text(QRectF(x + w * 0.60, ry, w * 0.16, h * 0.12), val, 0.036, 0.016, t["fg"],
                 QFont.Bold, L | VC, 1, "number", markup=False, wrap=False)
            bodytxt(QRectF(x + w * 0.775, ry, w * 0.17, h * 0.12), label, L | VC, 0.017, 0.0115,
                    lines=4)
            if n < len(rest) - 1 or hidden:
                p.fillRect(
                    QRectF(x + w * 0.60, ry + h * 0.14, w * 0.345, max(1, h * 0.002)),
                    QColor(t["rule"]),
                )
        if hidden:
            more(QRectF(x + w * 0.60, y + h * (0.25 + len(rest) * 0.17), w * 0.345, h * 0.06),
                 hidden)
    elif layout == "PROCESS":
        hero(QRectF(x + m, y + h * 0.09, w * 0.80, h * 0.15), title, 0.050, 0.026, lines=2)
        items, hidden = _take([q.strip() for q in body.splitlines() if q.strip()], 5, "steps")
        n = max(1, len(items))
        gap = w * 0.018
        cell = (w - 2 * m - gap * (n - 1)) / n
        y0 = y + h * 0.33
        heads, descs = [], []
        for item in items:
            a = item.split("|", 1)
            heads.append(a[0].strip())
            descs.append(a[1].strip() if len(a) > 1 else "")
        hrects = [QRectF(x + m + j * (cell + gap), y0 + h * 0.11, cell, h * 0.12) for j in range(len(items))]
        drects = [QRectF(r.x(), y0 + h * 0.24, cell, h * 0.30) for r in hrects]
        hs = paired(heads, hrects, 0.030, 0.016, QFont.Bold, 3)
        ds = paired(descs, drects, 0.019, 0.0115)
        for j in range(len(items)):
            rx = hrects[j].x()
            label_text(QRectF(rx, y0, cell, h * 0.06), f"{j+1:02d}", 0.018, accent_for(j))
            p.fillRect(QRectF(rx, y0 + h * 0.08, cell, max(2, h * 0.004)), QColor(accent_for(j)))
            text(hrects[j], heads[j], 0.030, 0.016, t["fg"], QFont.Bold, L | T, 3, "step", size=hs)
            text(drects[j], descs[j], 0.019, 0.0115, t["muted"], QFont.Normal, L | T, None,
                 "step", size=ds)
        if hidden:
            more(QRectF(x + m, y + h * 0.86, w - 2 * m, h * 0.05), hidden, R | VC)
    elif layout == "MATRIX":
        hero(QRectF(x + m, y + h * 0.07, w * 0.80, h * 0.14), title, 0.046, 0.024, lines=2)
        items, hidden = _take([q.strip() for q in body.splitlines() if q.strip()], 4, "cells")
        for n, item in enumerate(items):
            a = item.split("|", 1)
            head = a[0].strip()
            desc = a[1].strip() if len(a) > 1 else ""
            col, row = n % 2, n // 2
            rx = x + m + col * w * 0.445
            ry = y + h * (0.25 + row * 0.33)
            rw = w * 0.40
            p.fillRect(QRectF(rx, ry, w * 0.035, max(2, h * 0.004)), QColor(accent_for(n)))
            text(QRectF(rx, ry + h * 0.02, rw, h * 0.085), head, 0.025, 0.015, t["fg"],
                 QFont.Bold, L | VC, 2, "cell")
            bodytxt(QRectF(rx, ry + h * 0.11, rw, h * 0.19), desc, L | T, 0.020, 0.012)
        if hidden:
            more(QRectF(x + m, y + h * 0.89, w - 2 * m, h * 0.04), hidden, R | VC)
    elif layout == "TABLE":
        hero(QRectF(x + m, y + h * 0.07, w * 0.80, h * 0.14), title, 0.046, 0.024, lines=2)
        rows = _table_rows(body)
        cols = min(6, max([len(r) for r in rows], default=1))
        rows = [(r + [""] * cols)[:cols] for r in rows]
        tx0, tw = x + m, w - 2 * m
        ttop, tbot = y + h * 0.24, y + h * 0.905
        # Column widths follow the content (clamped), so a label column stays narrow.
        weights = []
        for c in range(cols):
            longest = max([len(_plain_markup(r[c])) for r in rows] + [3])
            weights.append(min(40, max(6, longest)) ** 0.7)
        tot = sum(weights) or 1
        widths = [tw * wv / tot for wv in weights]
        pad_x, pad_y = w * 0.008, h * 0.010

        def row_heights(size, rr):
            out = []
            for ri, row in enumerate(rr):
                need = 0.0
                for c in range(cols):
                    n_lines, step, tall = textfit.measure(
                        row[c], sans, QFont.Bold if ri == 0 or c == 0 else QFont.Normal, size,
                        widths[c] - 2 * pad_x,
                    )
                    tall = tall if n_lines <= 3 else tall * 3 / n_lines
                    need = max(need, tall, step)
                out.append(need + 2 * pad_y + 1)
            return out

        avail = tbot - ttop
        max_size, min_size = int(px(0.022 * scale)), int(px(0.0125 * scale, 6))
        shown_rows, size, heights = rows, min_size, None
        if rows:
            for sz in range(max_size, min_size - 1, -1):
                hh = row_heights(sz, rows)
                if sum(hh) <= avail and all(
                    textfit.measure(r[c], sans, QFont.Normal, sz, widths[c] - 2 * pad_x)[0] <= 3
                    for r in rows for c in range(cols)
                ):
                    size, heights = sz, hh
                    break
            if heights is None:
                heights = row_heights(min_size, rows)
                keep = len(rows)
                more_h = h * 0.05
                while keep > 1 and sum(heights[:keep]) > avail - more_h:
                    keep -= 1
                if keep < len(rows):
                    textfit.note("hidden", label="rows", count=len(rows) - keep)
                shown_rows = rows[:keep]
                heights = heights[:keep]
        yy = ttop
        for ri, row in enumerate(shown_rows):
            rh = min(heights[ri], tbot - yy)
            if ri % 2 == 0 and ri > 0:
                band = QColor(t["rule"])
                band.setAlpha(70)
                p.fillRect(QRectF(tx0, yy, tw, rh), band)
            xx = tx0
            for c in range(cols):
                cell = QRectF(xx + pad_x, yy + pad_y, widths[c] - 2 * pad_x, max(1, rh - 2 * pad_y))
                text(cell, row[c], 0.022, 0.0125, t["fg"] if ri == 0 or c == 0 else t["muted"],
                     QFont.Bold if ri == 0 or c == 0 else QFont.Normal, L | VC, 3, "cell", size=size)
                xx += widths[c]
            p.fillRect(
                QRectF(tx0, yy + rh - max(1, h * 0.0015), tw, max(1, h * 0.0015)),
                QColor(t["accent"] if ri == 0 else t["rule"]),
            )
            yy += rh
        if len(shown_rows) < len(rows):
            more(QRectF(tx0, yy + h * 0.005, tw, h * 0.04), len(rows) - len(shown_rows), R | VC)
    elif layout == "IMAGE":
        has_title = bool(str(title).strip())
        if has_title:
            hero(QRectF(x + m, y + h * 0.065, w * 0.80, h * 0.10), title, 0.040, 0.022, lines=2)
        ir = QRectF(x + m, y + h * (0.18 if has_title else 0.10), w - 2 * m,
                    h * (0.62 if has_title else 0.70))
        image(ir, img)
        cap = str(s.get("caption") or "")
        bw = w - 2 * m if not cap else w * 0.58
        bodytxt(QRectF(x + m, y + h * 0.82, bw, h * 0.095), body, L | VC, 0.024, 0.013, lines=3)
        if cap:
            text(QRectF(x + m + w * 0.60, y + h * 0.82, w * 0.29, h * 0.095), cap, 0.016, 0.011,
                 t["muted"], QFont.Normal, R | VC, 3, "caption")
    elif layout == "IMAGE + CAPTION":
        ir = QRectF(x + m, y + h * 0.07, w - 2 * m, h * 0.66)
        image(ir, img, mode if mode != "FILL" else "FILL")
        p.fillRect(QRectF(x + m, y + h * 0.765, max(2, w * 0.003), h * 0.13), QColor(t["accent"]))
        cx = x + m + w * 0.018
        hero(QRectF(cx, y + h * 0.755, w * 0.80, h * 0.07), title, 0.034, 0.018, L | VC, 1)
        cap = "\n".join(q for q in (body, str(s.get("caption") or "")) if str(q).strip())
        bodytxt(QRectF(cx, y + h * 0.83, w * 0.80, h * 0.075), cap, L | T, 0.019, 0.0115, lines=3)
    elif layout == "SPLIT":
        left = QRectF(x + m, y + h * 0.14, w * 0.36, h * 0.74)
        right = QRectF(x + w * 0.50, y + h * 0.12, w * 0.445, h * 0.76)
        tr = QRectF(left.x(), left.y(), left.width(), left.height() * (0.54 if body else 1.0))
        hero(tr, title, 0.064, 0.030, (base_align | B) if body else None, 5)
        if body:
            bodytxt(QRectF(left.x(), tr.bottom() + h * 0.03, left.width(),
                           left.bottom() - tr.bottom() - h * 0.03), body, L | T, 0.025, 0.014)
        image(right, img)
        p.fillRect(QRectF(x + w * 0.465, y + h * 0.27, max(1, w * 0.0015), h * 0.46),
                   QColor(t["rule"]))
    elif layout == "PHOTO":
        ir = QRectF(x + m, y + h * 0.085, w - 2 * m, h * 0.715)
        image(ir, img)
        hero(QRectF(x + m, y + h * 0.815, w * 0.50, h * 0.075), title, 0.032, 0.018, L | VC, 2)
        bodytxt(QRectF(x + m + w * 0.53, y + h * 0.815, w * 0.36, h * 0.075), body, R | VC,
                0.019, 0.0115, lines=3)
        if s.get("caption"):
            text(QRectF(x + m, y + h * 0.893, w - 2 * m, h * 0.035), s.get("caption"), 0.013,
                 0.010, t["muted"], QFont.Normal, R | VC, 1, "caption")
    elif layout == "IMAGE + QUOTE":
        ir = QRectF(x + m, y + h * 0.11, w * 0.49, h * 0.75)
        image(ir, img)
        p.fillRect(QRectF(x + w * 0.555, y + h * 0.25, max(1, w * 0.0014), h * 0.43),
                   QColor(t["rule"]))
        hero(QRectF(x + w * 0.59, y + h * 0.13, w * 0.35, h * 0.44), title, 0.056, 0.026,
             base_align | B, 6)
        bodytxt(QRectF(x + w * 0.59, y + h * 0.60, w * 0.35, h * 0.26), body, L | T, 0.022, 0.013)
    elif layout == "NUMBER GRID":
        hero(QRectF(x + m, y + h * 0.08, w * 0.82, h * 0.14), title, 0.05, 0.026, lines=2)
        items, hidden = _take([q.strip() for q in body.splitlines() if q.strip()], 4, "numbers")
        for n, item in enumerate(items):
            parts = item.split("|", 1)
            num = parts[0].strip()
            label = parts[1].strip() if len(parts) > 1 else ""
            col, row = n % 2, n // 2
            rx = x + m + col * w * 0.445
            ry = y + h * (0.26 + row * 0.31)
            text(QRectF(rx, ry, w * 0.40, h * 0.14), num, 0.105, 0.03, accent_for(n), QFont.Black,
                 L | VC, 1, "number", markup=False, wrap=False)
            bodytxt(QRectF(rx, ry + h * 0.145, w * 0.40, h * 0.13), label, L | T, 0.022, 0.0125,
                    lines=3)
        if hidden:
            more(QRectF(x + m, y + h * 0.88, w - 2 * m, h * 0.04), hidden, R | VC)
    elif layout == "TIMELINE":
        hero(QRectF(x + m, y + h * 0.10, w * 0.82, h * 0.15), title, 0.052, 0.026, lines=2)
        items, hidden = _take([q.strip() for q in body.splitlines() if q.strip()], 5, "milestones")
        n = max(1, len(items))
        yline = y + h * 0.50
        cells = timeline_cells(QRectF(x + m, y, w - 2 * m, h), n, yline)
        if cells and items:
            p.fillRect(
                QRectF(cells[0][0], yline, max(1, cells[-1][0] - cells[0][0]), max(2, h * 0.004)),
                QColor(t["rule"]),
            )
        heads, descs = [], []
        for item in items:
            parts = item.split("|", 1)
            heads.append(parts[0].strip())
            descs.append(parts[1].strip() if len(parts) > 1 else "")
        hrects = [QRectF(c[1].x(), yline - h * 0.17, c[1].width(), h * 0.14) for c in cells[: len(items)]]
        drects = [QRectF(c[2].x(), yline + h * 0.045, c[2].width(), h * 0.33) for c in cells[: len(items)]]
        hs = paired(heads, hrects, 0.022, 0.013, QFont.Medium, 3)
        ds = paired(descs, drects, 0.017, 0.0115)
        for j in range(len(items)):
            xx = cells[j][0]
            p.setBrush(QColor(accent_for(j)))
            p.setPen(Qt.NoPen)
            p.drawEllipse(QRectF(xx - w * 0.008, yline - w * 0.008, w * 0.016, w * 0.016))
            text(hrects[j], heads[j], 0.022, 0.013, t["fg"], QFont.Medium, HC | B, 3, "milestone",
                 size=hs)
            text(drects[j], descs[j], 0.017, 0.0115, t["muted"], QFont.Normal, HC | T, None,
                 "milestone", size=ds)
        if hidden:
            more(QRectF(x + m, y + h * 0.88, w - 2 * m, h * 0.04), hidden, R | VC)
    elif layout == "QUOTE":
        text(QRectF(x + m, y + h * 0.10, w * 0.18, h * 0.20), "“", 0.16, 0.08, t["rule"],
             QFont.Black, L | T, 1, "label", markup=False, scaled=False)
        p.fillRect(QRectF(x + m, y + h * 0.26, max(2, w * 0.003), h * 0.45), QColor(t["accent"]))
        q = QRectF(x + m + w * 0.045, y + h * 0.20, w * 0.80, h * 0.48)
        hero(q, title, 0.068, 0.028, lines=6)
        bodytxt(QRectF(q.x(), y + h * 0.71, q.width(), h * 0.13), body, L | T, 0.022, 0.013,
                lines=3)
    elif layout == "BIG NUMBER":
        text(QRectF(x + m, y + h * 0.16, w - 2 * m, h * 0.44), title, 0.23, 0.04, t["accent"],
             QFont.Black, L | VC, 1, "number", wrap=False)
        bodytxt(QRectF(x + m, y + h * 0.63, w * 0.78, h * 0.24), body, L | T, 0.027, 0.014)
    elif layout == "SECTION":
        sm = re.match(r"^\s*(\d{1,2})[. :–—-]+(.*)$", title)
        if sm:
            text(QRectF(x + m, y + h * 0.10, w * 0.18, h * 0.16), sm.group(1).zfill(2), 0.10,
                 0.05, t["accent"], QFont.Normal, L | VC, 1, "label", markup=False)
            section_title = sm.group(2).strip()
        else:
            section_title = title
        p.fillRect(QRectF(x + m, y + h * 0.53, w * 0.10, max(2, h * 0.006)), QColor(t["accent"]))
        hero(QRectF(x + m, y + h * 0.27, w * 0.82, h * 0.24), section_title, 0.07, 0.032,
             base_align | B, 3)
        bodytxt(QRectF(x + m + w * 0.14, y + h * 0.57, w * 0.66, h * 0.27), body, L | T)
    elif layout == "END":
        hero(QRectF(x + m, y + h * 0.20, w - 2 * m, h * 0.30), title, 0.085, 0.036, HC | B, 3)
        bodytxt(QRectF(x + m, y + h * 0.52, w - 2 * m, h * 0.20), body, HC | T)
        if not t.get("uddevalla"):
            p.fillRect(
                QRectF(x + w * 0.45, y + h * 0.76, w * 0.10, max(2, h * 0.006)), QColor(t["accent"])
            )
    elif layout == "STATEMENT":
        hero(QRectF(x + m, y + h * 0.17, w - 2 * m, h * 0.47), title, 0.095, 0.036, lines=5)
        p.fillRect(QRectF(x + m, y + h * 0.67, w * 0.10, max(2, h * 0.006)), QColor(t["accent"]))
        bodytxt(QRectF(x + m, y + h * 0.70, w * 0.80, h * 0.20), body, L | T)
    elif layout == "TEXT":
        hero(QRectF(x + m, y + h * 0.12, w * 0.80, h * 0.20), title, 0.06, 0.028, lines=3)
        p.fillRect(QRectF(x + m, y + h * 0.35, w * 0.07, max(2, h * 0.004)), QColor(t["accent"]))
        bodytxt(QRectF(x + m, y + h * 0.39, w * 0.80, h * 0.51), body, L | T)
    elif layout == "CODE":
        has_title = bool(str(title).strip())
        if has_title:
            hero(QRectF(x + m, y + h * 0.07, w * 0.80, h * 0.11), title, 0.042, 0.022, lines=2)
        panel = QRectF(x + m, y + h * (0.20 if has_title else 0.09), w - 2 * m,
                       h * (0.70 if has_title else 0.81))
        bg = QColor(t["fg"])
        bg.setAlpha(18)
        p.fillRect(panel, bg)
        p.fillRect(QRectF(panel.x(), panel.y(), max(2, w * 0.003), panel.height()),
                   QColor(t["accent"]))
        code = str(body or "").replace("\r\n", "\n").expandtabs(4).strip("\n")
        inner = panel.adjusted(w * 0.022, h * 0.03, -w * 0.018, -h * 0.03)
        text(inner, code, 0.030, 0.0135, t["fg"], QFont.Normal, L | T, None, "code",
             family=mono_font, markup=False, wrap=False)
    else:  # TITLE
        tw = w * (0.53 if img else 0.82)
        hero(QRectF(x + m, y + h * 0.16, tw, h * 0.35), title, 0.09, 0.034, base_align | B, 4)
        p.fillRect(QRectF(x + m, y + h * 0.54, w * 0.10, max(2, h * 0.006)), QColor(t["accent"]))
        bodytxt(QRectF(x + m, y + h * 0.58, w * (0.53 if img else 0.75), h * 0.25), body, L | T)
        if img:
            draw_image(p, QRectF(x + w * 0.64, y + h * 0.18, w * 0.30, h * 0.62), img, mode, mono,
                       fx, fy, 0.92, br, ct, ov, bl)
    if s.get("imageRequest") and not s.get("image"):
        textfit.fit_draw(p, QRectF(x + w * 0.60, y + h * 0.90, w * 0.34, h * 0.035),
                         "IMAGE REQUEST  ·  " + s.get("imageRequest", ""), mono_font,
                         QFont.Normal, px(0.014, 6), px(0.014, 6), QColor(t["muted"]),
                         R | VC, 1, "request", False)
    if theme_name == "SATIE":
        p.setPen(QColor(t["rule"]))
        p.drawLine(int(x + w * 0.80), int(y + h * 0.09), int(x + w * 0.94), int(y + h * 0.09))
        p.drawLine(int(x + w * 0.94), int(y + h * 0.09), int(x + w * 0.94), int(y + h * 0.18))
    if theme_name == "CARL LARSSON":
        palette = t.get("palette", ["#A33E32", "#557A68", "#66859A", "#C39A45"])
        band_w = w * 0.022
        band_h = max(2, h * 0.004)
        for j, c in enumerate(palette):
            p.fillRect(QRectF(x + m + j * band_w, y + h * 0.925, band_w * 0.72, band_h), QColor(c))
    if t.get("uddevalla"):
        p.fillRect(QRectF(x, y, w * 0.010, h), QColor(t["accent"]))
        logo_mode = str(s.get("logoMode", "AUTO")).upper()
        auto_layout = layout in ("TITLE", "SECTION", "END")
        want_logo = show_logo and logo_mode != "OFF" and (logo_mode == "ON" or auto_layout)
        if want_logo:
            if layout == "FULL BLEED":
                logo_path = BRANDING / "uddevalla-white-disc.png"
            else:
                bg = QColor(t["bg"])
                lum = 0.2126 * bg.redF() + 0.7152 * bg.greenF() + 0.0722 * bg.blueF()
                logo_path = BRANDING / (
                    "uddevalla-light.png" if lum >= 0.55 else "uddevalla-dark.png"
                )
            side = h * 0.135
            if layout == "END":
                lr = QRectF(x + w * 0.5 - side * 0.5, y + h * 0.755, side, side)
            elif layout in ("SPLIT", "IMAGE + QUOTE"):
                lr = QRectF(x + m, y + h * 0.80, side, side)
            elif layout in ("IMAGE", "PHOTO", "FULL BLEED", "IMAGE + CAPTION"):
                lr = QRectF(x + w - side - w * 0.035, y + h * 0.035, side, side)
            elif layout == "TITLE" and img:
                lr = QRectF(x + m, y + h * 0.805, side, side)
            else:
                lr = QRectF(x + w - side - w * 0.038, y + h * 0.805, side, side)
            pm = _cached_pixmap(
                str(logo_path), QSize(max(64, int(lr.width())), max(64, int(lr.height())))
            )
            if not pm.isNull():
                scaled = pm.scaled(
                    int(lr.width()), int(lr.height()), Qt.KeepAspectRatio, Qt.SmoothTransformation
                )
                dx = lr.x() + (lr.width() - scaled.width()) / 2
                dy = lr.y() + (lr.height() - scaled.height()) / 2
                p.drawPixmap(int(dx), int(dy), scaled)
    if footer_text and layout != "FULL BLEED":
        footer_align = normalize_footer_align(footer_align)
        footer_size = normalize_footer_size(footer_size)
        size_factor = {"SMALL": 0.010, "MEDIUM": 0.012, "LARGE": 0.014}[footer_size]
        flags = {"LEFT": Qt.AlignLeft, "CENTER": Qt.AlignHCenter, "RIGHT": Qt.AlignRight}[
            footer_align
        ] | Qt.AlignVCenter
        fpx = px(size_factor, 7)
        textfit.fit_draw(p, QRectF(x + m, y + h * 0.938, w - 2 * m, h * 0.035), str(footer_text),
                         sans, QFont.Normal, fpx, fpx, QColor(t["muted"]), flags, 1, "footer",
                         False)
    p.restore()


def slide_fit_issues(s, data, index=0, width=1920, height=1080):
    """Render one slide offscreen and return its fit problems.

    Returns a list of human sentences such as "text does not fit (title)" or
    "3 items hidden". Results are cached by slide content.
    """
    key = json.dumps(
        [s, data.get("theme"), data.get("fontFamily"), data.get("footerText"),
         data.get("footerSize"), data.get("showNumbers"), width, height],
        sort_keys=True, default=str,
    )
    hit = _FIT_CACHE.get(key)
    if hit is not None:
        _FIT_CACHE.move_to_end(key)
        return list(hit)
    report = []
    pic = QPicture()
    qp = QPainter(pic)
    try:
        render_slide(
            qp, QRectF(0, 0, width, height), s, data.get("theme", "B&W"), index,
            len(data.get("slides", [])) or 1, data.get("showNumbers", False),
            data.get("fontFamily", "Noto Sans"), data.get("showLogo", True),
            data.get("footerText", ""), data.get("footerAlign", "LEFT"),
            data.get("footerSize", "MEDIUM"), report=report,
        )
    finally:
        if qp.isActive():
            qp.end()
    out = []
    clipped = sorted({r.get("label", "text") for r in report if r["kind"] == "clipped"
                      and r.get("label") not in ("footer", "request", "label", "more")})
    if clipped:
        out.append("text does not fit (" + ", ".join(clipped) + ")")
    for r in report:
        if r["kind"] == "hidden":
            n = r.get("count", 0)
            out.append(f"{n} {r.get('label', 'items')} hidden" if n != 1 else
                       f"1 {r.get('label', 'items').rstrip('s')} hidden")
    _FIT_CACHE[key] = list(out)
    if len(_FIT_CACHE) > 2000:
        _FIT_CACHE.popitem(last=False)
    return out


_FIT_CACHE = OrderedDict()
