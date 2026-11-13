#!/usr/bin/env python3
"""Template stress render (2.3): every layout with short, normal, long and very
long text, at several output sizes and pixel densities.

Fails when any text line is painted outside its box or outside the slide
(overflow), and when short or normal text is clipped or items are hidden.
Long and very long text may shrink, clip with an ellipsis or show "+N more",
but never overflow. Run standalone (prints a summary, writes PNGs with
--out DIR) or through the release gate.
"""

import os
import sys
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

LONGWORD = "https://intranet.example.org/socialtjanst/rutiner/handlaggning/dokumentation"
IMAGE_LAYOUTS = ("IMAGE", "PHOTO", "HERO IMAGE", "SPLIT", "IMAGE + QUOTE", "FULL BLEED",
                 "IMAGE + CAPTION")


def words(n, seed=0):
    base = ("presentation slide content needs to remain readable when people add more text "
            "than the template designer expected in the first place so we test it").split()
    return " ".join(base[(i + seed) % len(base)] for i in range(n))


TEXT = {
    "short": dict(title="Q3", line=lambda i: words(2, i), n=2),
    "normal": dict(title="Quarterly results and next steps", line=lambda i: words(7, i), n=4),
    "long": dict(
        title="Quarterly results, what we learned and the next steps for the whole organisation",
        line=lambda i: words(16, i), n=7),
    "vlong": dict(
        title="Quarterly results, what we learned, what went wrong and the next steps for the "
              "whole organisation in 2027 and beyond " + LONGWORD,
        line=lambda i: words(32, i) + (" " + LONGWORD if i == 0 else ""), n=12),
}


def body_for(layout, lv):
    d = TEXT[lv]
    n, line = d["n"], d["line"]
    if layout in ("DATA / KPI", "NUMBER GRID"):
        vals = {"short": "7", "normal": "42 %", "long": "1 234 567", "vlong": "1 234 567 890 kr"}[lv]
        return "\n".join(f"{vals} | {line(i)}" for i in range(n))
    if layout in ("PROCESS", "TIMELINE", "MATRIX"):
        step = {"short": "Plan", "normal": "Discovery", "long": "Discovery and planning",
                "vlong": "Discovery, planning and stakeholder alignment"}[lv]
        return "\n".join(f"{step} {i+1} | {line(i)}" for i in range(n))
    if layout == "TABLE":
        cols = {"short": 2, "normal": 3, "long": 4, "vlong": 6}[lv]
        rows = [" | ".join(f"Header {c+1}" for c in range(cols))]
        rows += [" | ".join(words(2 if lv == "short" else (4 if lv == "normal" else 9), r + c)
                            for c in range(cols)) for r in range(n)]
        return "\n".join(rows)
    if layout in ("TWO COLUMN", "COMPARE"):
        left = "\n".join(line(i) for i in range(n))
        right = "\n".join(line(i + 3) for i in range(n))
        return left + "\n|||\n" + right
    if layout == "THREE COLUMN":
        return "\n|||\n".join("\n".join(line(i + k) for i in range(max(1, n - 1))) for k in range(3))
    if layout == "CODE":
        count = {"short": 3, "normal": 12, "long": 24, "vlong": 60}[lv]
        width = {"short": 20, "normal": 50, "long": 80, "vlong": 140}[lv]
        return "\n".join(("    " * (i % 3) + f"value_{i} = compute({words(3, i)!r})" + " # x" * width)[:width + 8]
                         for i in range(count))
    if layout in ("BULLETS", "AGENDA"):
        return "\n".join(line(i) for i in range(n))
    if layout in ("TITLE", "SECTION", "END", "STATEMENT", "QUOTE"):
        return {"short": "Hi", "normal": "A subtitle line for the slide", "long": words(25),
                "vlong": words(70) + " " + LONGWORD}[lv]
    if layout == "BIG NUMBER":
        return line(0)
    if layout in IMAGE_LAYOUTS or layout.startswith("VIDEO"):
        return "\n".join(line(i) for i in range(min(n, 3)))
    return "\n".join(line(i) for i in range(n))


def title_for(layout, lv):
    if layout == "BIG NUMBER":
        return {"short": "7", "normal": "42 %", "long": "1 234 567",
                "vlong": "1 234 567 890 kr (prognos)"}[lv]
    return TEXT[lv]["title"]


def test_image(path):
    from PySide6.QtGui import QColor, QImage, QLinearGradient, QPainter

    im = QImage(1600, 1000, QImage.Format_RGB32)
    pp = QPainter(im)
    g = QLinearGradient(0, 0, 1600, 1000)
    g.setColorAt(0, QColor("#335577"))
    g.setColorAt(1, QColor("#aa7733"))
    pp.fillRect(0, 0, 1600, 1000, g)
    pp.end()
    im.save(str(path))
    return str(path)


def run(m=None, out_dir=None, verbose=False):
    from PySide6.QtCore import QRectF
    from PySide6.QtGui import QImage, QPainter
    from PySide6.QtWidgets import QApplication

    QApplication.instance() or QApplication([])
    import tempfile

    from nirupres import foundation as F
    from nirupres import textfit

    tmp = tempfile.TemporaryDirectory(prefix="nirupres-fit-")
    img = test_image(Path(tmp.name) / "test.png")
    failures, notes = [], []
    sizes = [(1920, 1080, 96), (1280, 720, 144), (2560, 1440, 96)]
    for (W, H, dpi) in sizes:
        for layout in F.LAYOUTS:
            for lv in TEXT:
                s = F.slide_defaults({
                    "layout": layout, "title": title_for(layout, lv), "body": body_for(layout, lv),
                    "notes": "secret notes",
                    "caption": TEXT[lv]["title"] if layout in ("IMAGE", "PHOTO") else "",
                })
                if layout in IMAGE_LAYOUTS or layout == "TITLE":
                    s["image"] = img
                image = QImage(W, H, QImage.Format_ARGB32)
                image.fill(0)
                dpm = int(dpi / 0.0254)
                image.setDotsPerMeterX(dpm)
                image.setDotsPerMeterY(dpm)
                report, trace = [], []
                p = QPainter(image)
                slide = QRectF(0, 0, W, H)
                with textfit.collect(report, trace):
                    F.render_slide(p, slide, s, "B&W", 2, 10, True, "Noto Sans", False,
                                   "Footer text · Organisation", "LEFT", "MEDIUM")
                p.end()
                tag = f"{layout} {lv} {W}x{H}@{dpi}"
                for box, line, txt in trace:
                    inside = (line.left() >= box.left() - 1 and line.right() <= box.right() + 1
                              and line.top() >= box.top() - 1 and line.bottom() <= box.bottom() + 1)
                    on_slide = slide.adjusted(-1, -1, 1, 1).contains(box)
                    if not inside or not on_slide:
                        failures.append(f"OVERFLOW {tag}: {txt[:40]!r} line={line.getRect()} "
                                        f"box={box.getRect()}")
                bad = [r for r in report if r.get("label") not in ("footer", "label", "request")]
                if bad and lv in ("short", "normal"):
                    failures.append(f"NOFIT {tag}: {bad}")
                elif bad:
                    notes.append(f"{tag}: " + ", ".join(
                        f"{r['kind']} {r.get('label')}" + (f" {r['count']}" if 'count' in r else "")
                        for r in bad))
                if out_dir and (W, H) == (1920, 1080):
                    Path(out_dir).mkdir(parents=True, exist_ok=True)
                    safe = layout.replace(" ", "_").replace("/", "-").replace("+", "plus")
                    image.save(str(Path(out_dir) / f"{safe}__{lv}.png"))
    tmp.cleanup()
    if verbose:
        for n in notes:
            print("note:", n)
    return failures, notes


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
    out = None
    if "--out" in sys.argv:
        out = sys.argv[sys.argv.index("--out") + 1]
    fails, notes = run(out_dir=out, verbose="-v" in sys.argv)
    for f in fails:
        print(f)
    print(f"template fit: {len(fails)} failure(s), {len(notes)} long-text note(s)")
    sys.exit(1 if fails else 0)
