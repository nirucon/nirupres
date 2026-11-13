"""Pixel-based text fitting for slide templates (2.3).

Every slide text goes through ``fit_draw``. It lays the text out with
QTextLayout at a pixel size, so the measurement and the painting use exactly
the same lines on every paint device (screen, QImage, PDF, thumbnails). The
fit honours ``max_lines`` and the box height, shrinks to the layout minimum and
then clips with an ellipsis inside the box. It never paints outside the box.

Overflow is reported to an active report list (see ``collect``) so Deck health
can tell the user "slide 7: text does not fit".
"""

import re
from collections import OrderedDict
from contextlib import contextmanager

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import QFont, QFontMetricsF, QTextCharFormat, QTextLayout, QTextOption

ELLIPSIS = "…"
_MARK = re.compile(
    r"\*\*\*([^*\n]+?)\*\*\*|___([^_\n]+?)___|\*\*(.+?)\*\*|__(.+?)__"
    r"|(?<!\*)\*([^*\n]+?)\*(?!\*)|(?<!_)_([^_\n]+?)_(?!_)"
)
_REPORT = None  # list of dicts while a report is being collected
_TRACE = None  # list of (box, painted_rect, text) for tests
_CACHE = OrderedDict()
_CACHE_MAX = 6000


@contextmanager
def collect(report=None, trace=None):
    """Collect fit issues (and optionally painted line rects) while rendering."""
    global _REPORT, _TRACE
    old = (_REPORT, _TRACE)
    _REPORT, _TRACE = report, trace
    try:
        yield report
    finally:
        _REPORT, _TRACE = old


def note(kind, **info):
    """Record a fit issue (``clipped`` or ``hidden``) for the active report."""
    if _REPORT is not None:
        _REPORT.append(dict(info, kind=kind))


def px_font(family, px, weight=QFont.Normal, italic=False):
    f = QFont(family)
    f.setPixelSize(max(1, int(round(px))))
    f.setWeight(weight)
    if italic:
        f.setItalic(True)
    return f


def markup_runs(text, bold=False, italic=False):
    """Split inline Markdown emphasis into (segment, bold, italic) runs."""
    out = []
    pos = 0
    for m in _MARK.finditer(text):
        if m.start() > pos:
            out.append((text[pos : m.start()], bold, italic))
        g = m.lastindex
        inner = m.group(g)
        if g in (1, 2):
            out += markup_runs(inner, True, True)
        elif g in (3, 4):
            out += markup_runs(inner, True, italic)
        else:
            out += markup_runs(inner, bold, True)
        pos = m.end()
    if pos < len(text):
        out.append((text[pos:], bold, italic))
    return out


def plain_and_formats(text, markup=True):
    text = str(text or "").replace("\r\n", "\n").replace("\r", "\n").replace("\t", "    ")
    if not markup:
        return text, []
    runs = markup_runs(text)
    plain = []
    fmts = []
    pos = 0
    for seg, b, i in runs:
        if b or i:
            fmts.append((pos, len(seg), b, i))
        plain.append(seg)
        pos += len(seg)
    return "".join(plain), fmts


class _Laid:
    __slots__ = ("layouts", "lines", "height", "width_ok", "fm")


def _layout(plain, fmts, font, width, wrap=True, align=Qt.AlignLeft):
    """Lay out ``plain`` (paragraphs split on newlines) at ``font`` into ``width``."""
    out = _Laid()
    out.layouts = []
    out.lines = []  # (layout, line, paragraph_start_in_plain)
    fm = QFontMetricsF(font)
    out.fm = fm
    step = fm.lineSpacing()
    y = 0.0
    out.width_ok = True
    opt = QTextOption()
    opt.setWrapMode(QTextOption.WrapAtWordBoundaryOrAnywhere if wrap else QTextOption.NoWrap)
    opt.setAlignment(Qt.Alignment(int(align) & int(Qt.AlignHorizontal_Mask)))
    start = 0
    for para in plain.split("\n"):
        tl = QTextLayout(para, font)
        tl.setTextOption(opt)
        if fmts:
            ranges = []
            for fs, fl, b, i in fmts:
                a = max(fs, start)
                e = min(fs + fl, start + len(para))
                if e > a:
                    r = QTextLayout.FormatRange()
                    r.start = a - start
                    r.length = e - a
                    cf = QTextCharFormat()
                    if b:
                        cf.setFontWeight(QFont.Bold if font.weight() < QFont.Bold else QFont.Black)
                    if i:
                        cf.setFontItalic(True)
                    r.format = cf
                    ranges.append(r)
            if ranges:
                tl.setFormats(ranges)
        tl.beginLayout()
        while True:
            line = tl.createLine()
            if not line.isValid():
                break
            line.setLineWidth(max(1.0, width))
            line.setPosition(QPointF(0, y))
            if line.naturalTextWidth() > width + 0.5:
                out.width_ok = False
            y += max(step, line.height())
            out.lines.append((tl, line, start))
        tl.endLayout()
        out.layouts.append(tl)
        start += len(para) + 1
    out.height = y
    return out


def _fits(laid, rect, max_lines):
    return (
        laid.width_ok
        and laid.height <= rect.height() + 0.5
        and (not max_lines or len(laid.lines) <= max_lines)
    )


def fit_size(text, family, weight, max_px, min_px, rect, max_lines=None, markup=True, wrap=True):
    """Largest integer pixel size in [min_px, max_px] where the text fits, or None."""
    plain, fmts = plain_and_formats(text, markup)
    max_px = max(1, int(max_px))
    min_px = max(1, min(int(min_px), max_px))
    key = (plain, tuple(fmts), family, int(weight), max_px, min_px, round(rect.width(), 1),
           round(rect.height(), 1), max_lines, wrap)
    if key in _CACHE:
        _CACHE.move_to_end(key)
        return _CACHE[key]

    def ok(px):
        return _fits(_layout(plain, fmts, px_font(family, px, weight), rect.width(), wrap), rect,
                     max_lines)

    result = None
    if not plain.strip():
        result = max_px
    elif ok(max_px):
        result = max_px
    elif ok(min_px):
        lo, hi = min_px, max_px  # lo fits, hi does not
        while hi - lo > 1:
            mid = (lo + hi) // 2
            if ok(mid):
                lo = mid
            else:
                hi = mid
        result = lo
    _CACHE[key] = result
    if len(_CACHE) > _CACHE_MAX:
        _CACHE.popitem(last=False)
    return result


def _trace(box, rect, text):
    if _TRACE is not None:
        _TRACE.append((QRectF(box), QRectF(rect), text))


def fit_draw(
    p,
    rect,
    text,
    family,
    weight,
    max_px,
    min_px,
    color,
    align=Qt.AlignLeft | Qt.AlignVCenter,
    max_lines=None,
    label="text",
    markup=True,
    wrap=True,
    px=None,
):
    """Fit ``text`` into ``rect`` and paint it. Returns the pixel size used.

    ``px`` forces a size (paired columns share one size); the text is still
    clipped with an ellipsis if it does not fit at that size.
    """
    rect = QRectF(rect)
    plain, fmts = plain_and_formats(text, markup)
    if not plain.strip() or rect.width() < 2 or rect.height() < 2:
        return int(px or max_px)
    if px is None:
        size = fit_size(text, family, weight, max_px, min_px, rect, max_lines, markup, wrap)
        clipped = size is None
        size = size or max(1, int(min(min_px, max_px)))
    else:
        size = max(1, int(px))
        clipped = False
    font = px_font(family, size, weight)
    laid = _layout(plain, fmts, font, rect.width(), wrap, align)
    if not wrap and not laid.width_ok:
        # Code that is still too wide at the minimum wraps rather than overflowing.
        wrap = True
        clipped = False
        laid = _layout(plain, fmts, font, rect.width(), True, align)
    if not _fits(laid, rect, max_lines):
        clipped = True
    step = laid.fm.lineSpacing()
    if step > rect.height():
        # Even one line is taller than the box: shrink below the minimum so
        # one line still fits inside the box.
        size = max(1, int(size * rect.height() / step))
        font = px_font(family, size, weight)
        laid = _layout(plain, fmts, font, rect.width(), wrap, align)
        step = laid.fm.lineSpacing()
        clipped = True
    lines = laid.lines
    shown = len(lines)
    if clipped:
        cap = max(1, int((rect.height() + 0.5) // max(1.0, step)))
        if max_lines:
            cap = min(cap, max_lines)
        shown = min(shown, cap)
        while shown > 1 and lines[shown - 1][1].y() + lines[shown - 1][1].height() > rect.height() + 0.5:
            shown -= 1
    truncated = shown < len(lines) or not laid.width_ok
    used_h = 0.0
    if shown:
        last = lines[shown - 1][1]
        used_h = last.y() + max(step, last.height())
    y0 = rect.y()
    if align & Qt.AlignVCenter:
        y0 += max(0.0, (rect.height() - used_h) / 2)
    elif align & Qt.AlignBottom:
        y0 += max(0.0, rect.height() - used_h)
    p.save()
    p.setClipRect(rect, Qt.IntersectClip)
    p.setPen(color)
    p.setFont(font)
    for n in range(shown):
        tl, line, pstart = lines[n]
        ly = y0 + line.y()
        if truncated and n == shown - 1:
            rest = plain[pstart + line.textStart():].replace("\n", " ")
            elided = laid.fm.elidedText(rest, Qt.ElideRight, rect.width() - 1)
            if not elided.endswith(ELLIPSIS):
                elided = elided.rstrip() + ELLIPSIS
                while elided != ELLIPSIS and laid.fm.horizontalAdvance(elided) > rect.width() - 1:
                    elided = elided[:-2] + ELLIPSIS
            lr = QRectF(rect.x(), ly, rect.width(), step)
            p.drawText(lr, int(align & Qt.AlignHorizontal_Mask) | int(Qt.AlignTop) | int(Qt.TextSingleLine), elided)
            wid = laid.fm.horizontalAdvance(elided)
            hx = rect.x() if not (align & (Qt.AlignHCenter | Qt.AlignRight)) else (
                rect.right() - wid if align & Qt.AlignRight else rect.center().x() - wid / 2)
            _trace(rect, QRectF(hx, ly, wid, step), elided)
        else:
            line.draw(p, QPointF(rect.x(), y0))
            _trace(rect, QRectF(rect.x() + line.naturalTextRect().x(), ly,
                                line.naturalTextWidth(), max(step, line.height())),
                   plain[pstart + line.textStart(): pstart + line.textStart() + line.textLength()])
    p.restore()
    if clipped or truncated:
        note("clipped", label=label, text=plain[:60], hidden_lines=len(lines) - shown)
    return size


def measure(text, family, weight, px, width, markup=True, wrap=True):
    """(line count, line step, height) of ``text`` at ``px`` in ``width``."""
    plain, fmts = plain_and_formats(text, markup)
    if not plain.strip():
        return 0, QFontMetricsF(px_font(family, px, weight)).lineSpacing(), 0.0
    laid = _layout(plain, fmts, px_font(family, px, weight), width, wrap)
    return len(laid.lines), laid.fm.lineSpacing(), laid.height


def more_label(count):
    return f"+{count} more"
