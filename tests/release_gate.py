#!/usr/bin/env python3
"""NIRUPRES release-gate smoke tests. Runs against staged/installed package."""
import os, sys, tempfile, zipfile, json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication, QPushButton, QComboBox, QCheckBox, QLineEdit
from PySide6.QtCore import Qt
from PySide6.QtTest import QTest

# Qt 6.11 requires a QGuiApplication before nirupres.main is imported because
# module initialization reaches Qt font services. Create it before that import.
APP_INSTANCE = QApplication.instance() or QApplication([])

import nirupres
from nirupres import main as m

EXPECTED_VERSION = "2.0.0"

def check(cond, msg):
    if not cond:
        raise AssertionError(msg)

def main():
    # A QGuiApplication must exist before any rendering/font-database access.
    # The release gate renders the full theme × layout matrix, and Qt 6.11 can
    # abort the process (not raise Python) if font services are touched first.
    app = APP_INSTANCE

    # Exceptions raised from Qt signal/shortcut callbacks are reported through
    # sys.excepthook and do not necessarily fail the Python process. Capture
    # them so the release gate cannot print PASS after a broken key handler.
    qt_callback_errors=[]
    original_excepthook=sys.excepthook
    def gate_excepthook(exc_type, exc, tb):
        qt_callback_errors.append((exc_type, exc, tb))
        original_excepthook(exc_type, exc, tb)
    sys.excepthook=gate_excepthook

    check(nirupres.__version__ == EXPECTED_VERSION, "package version mismatch")
    check(m.VERSION == EXPECTED_VERSION, "main version mismatch")
    check(m.FORMAT == 9 and m.MD_FORMAT == 2, "format contract changed unexpectedly")
    # 1.7.1 emphasis contract: formatting never spans paragraph boundaries and
    # combined emphasis renders without leaking Markdown markers.
    check(m._format_markdown_selection("one\ntwo", "italic")=="*one*\n*two*", "multiline italic formatting failed")
    check(m._format_markdown_selection("one\ntwo", "bold")=="**one**\n**two**", "multiline bold formatting failed")
    check(m._format_markdown_selection("**one**", "bold")=="one", "bold toggle failed")
    check(m._format_markdown_selection("*one*", "normal")=="one", "normal emphasis reset failed")
    check("<b><i>both</i></b>" in m._inline_html("***both***"), "combined emphasis parser failed")
    # 1.4.0 file-workflow contract: preferences are user-local, persistent,
    # and folder resolution never falls back to the application cwd.
    original_prefs=m.FILE_PREFS
    with tempfile.TemporaryDirectory(prefix="nirupres-fileprefs-") as td:
        root=Path(td); default_dir=root/"default"; last_dir=root/"last"; deck_dir=root/"deck"
        default_dir.mkdir(); last_dir.mkdir(); deck_dir.mkdir()
        m.FILE_PREFS=root/"prefs.json"
        m.load_file_prefs.__globals__["FILE_PREFS"]=m.FILE_PREFS
        m.save_file_prefs.__globals__["FILE_PREFS"]=m.FILE_PREFS
        prefs={"defaultFolder":str(default_dir),"rememberLastFolder":True,"lastFolder":str(last_dir),"lastFilter":""}
        m.save_file_prefs(prefs); loaded=m.load_file_prefs()
        check(loaded["rememberLastFolder"] and Path(loaded["lastFolder"])==last_dir, "file preferences did not persist")
        check(m.preferred_folder(loaded,deck_dir/"demo.nirupres",True)==deck_dir, "current deck folder must win for related save/export")
        check(m.preferred_folder(loaded,None,False)==last_dir, "remembered folder must win for Open")
        loaded["rememberLastFolder"]=False; loaded["lastFolder"]=""
        check(m.preferred_folder(loaded,None,False)==default_dir, "default folder fallback failed")
    m.FILE_PREFS=original_prefs
    m.load_file_prefs.__globals__["FILE_PREFS"]=original_prefs
    m.save_file_prefs.__globals__["FILE_PREFS"]=original_prefs

    # 1.4.1 smart asset contract: defaults are safe, oversized images are
    # reduced non-destructively for embedding, and ORIGINAL remains byte-exact.
    check(m.load_file_prefs().get("assetQuality") in ("OPTIMIZED","HIGH","ORIGINAL"), "asset quality preference invalid")
    from PySide6.QtGui import QImage, QColor
    with tempfile.TemporaryDirectory(prefix="nirupres-assets-") as td:
        root=Path(td); src=root/"large.jpg"; out=root/"out"; out.mkdir()
        sample=QImage(3600,1800,QImage.Format.Format_RGB32); sample.fill(QColor("#777777")); check(sample.save(str(src),"JPEG",96), "asset test source write failed")
        opt,before,after,srcdim,outdim=m._optimize_asset(src,out,"OPTIMIZED")
        check(opt.is_file() and max(outdim)<=3200 and outdim[0]/outdim[1]==srcdim[0]/srcdim[1], "optimized asset geometry failed")
        check(after < before, "optimized photographic asset did not shrink")
        original,ob,oa,_,_=m._optimize_asset(src,out,"ORIGINAL")
        check(original.read_bytes()==src.read_bytes() and ob==oa, "ORIGINAL asset mode is not byte-exact")

    # Regression guard for 1.0.7: the monochrome dialog helper calls QIcon at
    # runtime, so the symbol must be imported into nirupres.main. compileall
    # alone cannot detect this class of NameError.
    check(hasattr(m, "QIcon"), "QIcon missing from nirupres.main imports")
    check(m.QIcon().isNull(), "QIcon runtime smoke test failed")
    for name in ("UDDEVALLA DARK","UDDEVALLA BLACK","UDDEVALLA BLUE","UDDEVALLA GREEN","UDDEVALLA YELLOW","UDDEVALLA RED","UDDEVALLA PINK","UDDEVALLA PURPLE"):
        check(name in m.THEMES and m.THEMES[name].get("uddevalla"), f"missing theme {name}")
    check("UDDEVALLA LIGHT" not in m.THEMES, "duplicate Uddevalla Light must stay out of UI theme registry")
    check(m.normalize_theme("UDDEVALLA LIGHT")=="UDDEVALLA BLUE" and m.normalize_theme("UDDEVALLA")=="UDDEVALLA BLUE", "legacy theme alias failed")
    for asset in ("uddevalla-dark.png","uddevalla-light.png","uddevalla-white-disc.png"):
        check((m.BRANDING/asset).is_file(), f"missing branding asset {asset}")

    # Render every layout × theme combination offscreen. Structured examples
    # exercise the parsers used by semantic 1.2 layouts, not just blank frames.
    required_layouts={"AGENDA","HERO IMAGE","COMPARE","DATA / KPI","PROCESS","MATRIX","TABLE"}
    check(required_layouts.issubset(set(m.LAYOUTS)) and len(m.LAYOUTS)==25, "1.2 layout catalogue incomplete")
    samples={
        "AGENDA":"Opening\nContext\nDecision\nNext steps",
        "COMPARE":"NOW\nManual\nFragmented\n\nTARGET\nAutomated\nConnected",
        "DATA / KPI":"80% | Faster\n12 | Systems\n47 | Risks\n2027 | Production",
        "PROCESS":"ANALYZE | Need\nPILOT | Test\nDELIVER | Launch\nFOLLOW UP | Improve",
        "TIMELINE":"SEP | Pilot\nOCT | Test\nNOV | Release",
        "MATRIX":"PEOPLE | Capability\nPROCESS | Workflow\nTECH | Tools\nGOVERNANCE | Control",
        "TABLE":"AREA | STATUS | OWNER\nSecurity | Ready | IT\nTraining | Active | HR",
        "NUMBER GRID":"80% | Faster\n12 | Systems\n47 | Risks\n2027 | Production",
        "TWO COLUMN":"Left narrative\n\nRight narrative",
    }
    from PySide6.QtGui import QImage, QPainter
    matrix_image=QImage(1280,720,QImage.Format.Format_ARGB32)
    for theme_name in m.THEMES:
        for layout_name in m.LAYOUTS:
            matrix_image.fill(0)
            painter=QPainter(matrix_image)
            slide=m.slide_defaults({"layout":layout_name,"title":"Release gate","body":samples.get(layout_name,"One\nTwo\nThree")})
            m.render_slide(painter,m.QRectF(0,0,1280,720),slide,theme_name,0,1,True,"Noto Sans",True,"Footer")
            check(painter.isActive(), f"renderer stopped for {theme_name} / {layout_name}")
            painter.end()

    # 1.2.1 TIMELINE regression: long edge-node content must stay inside the
    # same safe area used by the renderer. This catches the old fixed-width
    # first/last label overflow without relying on screenshots.
    safe=m.QRectF(96,0,1088,720)
    for count in range(1,6):
        cells=m.timeline_cells(safe,count,720*.54)
        check(len(cells)==count, f"timeline cell count failed: {count}")
        for _,head_rect,desc_rect in cells:
            check(safe.contains(head_rect), f"timeline heading escaped safe area: {count}")
            check(safe.contains(desc_rect), f"timeline description escaped safe area: {count}")
    long_timeline="Praktiskt | Lantbruk och praktiskt verksamhetsarbete\nTeknik & IT | Från teknikintresse till professionella IT-roller\nLedarskap | IT-chef, enhetschef och sektionschef\nIdag | Digitalisering, informationssäkerhet samt kris och beredskap"
    img=QImage(1280,720,QImage.Format.Format_ARGB32); img.fill(0); painter=QPainter(img)
    m.render_slide(painter,m.QRectF(0,0,1280,720),m.slide_defaults({"layout":"TIMELINE","title":"En bred arbetslivsresa","body":long_timeline}),"B&W",7,16,True,"Noto Sans",True,"")
    check(painter.isActive(), "long Swedish TIMELINE render failed"); painter.end()

    # 1.2.2 footer render contract: every alignment/size combination must
    # render at normal and long lengths without callback/runtime failure.
    for fa in ("LEFT","CENTER","RIGHT"):
        for fs in ("SMALL","MEDIUM","LARGE"):
            img=QImage(1280,720,QImage.Format.Format_ARGB32); img.fill(0); qp=QPainter(img)
            m.render_slide(qp,m.QRectF(0,0,1280,720),m.slide_defaults({"layout":"TITLE","title":"Footer gate"}),"B&W",0,1,True,"Noto Sans",False,"Nicklas Rudolfsson 2026 | Presentation skapad med NIRUPRES - en egenutvecklad app.",fa,fs)
            check(qp.isActive(), f"footer render failed: {fa}/{fs}"); qp.end()

    deck=m.clone(m.DEFAULT)
    deck.update({"title":"Gate","theme":"UDDEVALLA PURPLE","showNumbers":True,"showLogo":False,"fontFamily":"Noto Sans","aspect":"16:10","footerText":"Socialtjänsten | 2026","footerAlign":"RIGHT","footerSize":"LARGE","transition":"MORPH","reduceMotion":True})
    deck["slides"]=[m.slide_defaults({"layout":"SECTION","title":"01 · Test","body":"Body","notes":"Private","hidden":True,"logoMode":"ON","accentOverride":"GREEN","imageMode":"FIT","mono":True,"focalX":.25,"focalY":.75,"align":"CENTER","weight":"BOLD","fontScale":1.2,"brightness":5,"contrast":6,"overlay":7,"blur":2,"caption":"Caption","imageRequest":"forest","transition":"FADE"})]
    md=m.export_nirupres_markdown(deck)
    rt=m.import_nirupres_markdown(md)
    for key in ("theme","showNumbers","showLogo","fontFamily","aspect","footerText","footerAlign","footerSize","transition","reduceMotion"):
        check(rt[key] == deck[key], f"markdown deck round-trip failed: {key}")
    for key in ("layout","title","body","notes","hidden","logoMode","accentOverride","imageMode","mono","align","weight","caption","imageRequest","transition"):
        check(rt["slides"][0][key] == deck["slides"][0][key], f"markdown slide round-trip failed: {key}")
    check(not m._safe_zip_name("../escape") and m._safe_zip_name("assets/test.png"), "zip path guard failed")

    # Monochrome editor chrome contract. Presentation/theme colours may be vivid,
    # but application controls and dialogs must not inherit platform-coloured icons.
    from PySide6.QtWidgets import QMessageBox, QDialogButtonBox
    from PySide6.QtCore import Qt as _Qt
    probe=m.make_message_box(None,"Gate","Unsaved changes",QMessageBox.Save|QMessageBox.Discard|QMessageBox.Cancel)
    check(probe.icon()==QMessageBox.NoIcon, "message box uses a platform icon")
    check(all(b.icon().isNull() for b in probe.buttons()), "message box button has a platform-coloured icon")
    # 1.1.1 regression: every dialog action must fit its rendered label with
    # padding. This catches the clipped "Close without Saving" bug.
    for b in probe.buttons():
        required=b.fontMetrics().horizontalAdvance(b.text().replace('&','')) + 32
        check(b.minimumWidth() >= required, f"dialog button text can clip: {b.text()}")
        check(b.minimumHeight() >= 34, f"dialog button too short: {b.text()}")
    total_actions=sum(b.minimumWidth() for b in probe.buttons()) + max(0,len(probe.buttons())-1)*8 + 32
    check(probe.minimumWidth() >= total_actions, "message box can squeeze dialog actions")
    probe.close()
    bb=m.neutralize_button_box(QDialogButtonBox(QDialogButtonBox.Save|QDialogButtonBox.Cancel))
    check(all(b.icon().isNull() for b in bb.buttons()), "dialog button box has a platform-coloured icon")


    check((m.ASSETS/"nirupres.svg").is_file(), "application icon asset missing")
    check("REVEAL" in m.TRANSITIONS, "REVEAL transition missing")
    check(hasattr(m,"TransitionEngine"), "central transition engine missing")
    motion=m.clone(m.DEFAULT); motion["reduceMotion"]=True; motion["transition"]="MORPH"
    aud=m.Audience(motion,0,online=True); check(aud._transition_for(0)=="NONE", "reduce-motion did not suppress transitions"); aud.close()

    w=m.Main()
    check(hasattr(m,"FileWorkflowSettings"), "file workflow settings dialog missing")
    # 1.1.1 studio-layout contract: labels are concise and the deck controls
    # remain monochrome/semantic rather than gaining decorative colour UI.
    deck_labels=[x.text() for x in w.findChildren(m.QLabel) if x.objectName()=="deckLabel"]
    check("THEME" in deck_labels and "FONT" in deck_labels and "RATIO" in deck_labels and "TRANSITION" in deck_labels and "FOOTER" in deck_labels, "deck control labels missing")
    check("DECK THEME" not in deck_labels and "DECK FONT" not in deck_labels, "legacy deck labels returned")
    check(w.transition.minimumWidth() >= 120, "transition selector too narrow")
    # Visible interaction contract: these controls must exist and be enabled.
    buttons={b.text():b for b in w.findChildren(QPushButton)}
    # Top-level controls must be immediately usable. Controls inside collapsed
    # checkable inspector groups are intentionally disabled by Qt until their
    # parent group is expanded, so test those in their actual interactive state.
    for label in ("EXPORT","?","▶ PRESENT","|▶","▣","◉ ONLINE","▦","SLIDES","INSPECTOR","OUTLINE","DECK SETTINGS…","PRESETS"):
        check(label in buttons, f"missing UI control: {label}")
        check(buttons[label].isEnabled(), f"disabled top-level UI control: {label}")
    for label in ("▶ PREVIEW","APPLY STYLE TO SELECTED"):
        check(label in buttons, f"missing UI control: {label}")

    # Expand the owning inspector groups before checking child interactivity.
    # QGroupBox(checkable=True) disables descendants while unchecked by design.
    w.presentation_group.setChecked(True); app.processEvents()
    check(buttons["▶ PREVIEW"].isEnabled(), "transition preview not enabled when Presentation inspector is expanded")
    style_group=getattr(w,"style_group",None)
    if style_group is not None:
        style_group.setChecked(True); app.processEvents()
    else:
        # Locate the STYLE group without depending on a private attribute name.
        from PySide6.QtWidgets import QGroupBox
        groups=[g for g in w.findChildren(QGroupBox) if "STYLE" in g.title()]
        check(bool(groups), "STYLE inspector group missing")
        groups[0].setChecked(True); app.processEvents()
    check(buttons["APPLY STYLE TO SELECTED"].isEnabled(), "apply-style control not enabled when Style inspector is expanded")
    for attr in ("theme","deckfont","aspect","transition","numbers","guides","logo","footer","showleft","showright","outline_btn","slide_transition"):
        check(hasattr(w,attr), f"missing UI binding: {attr}")
    check(w.theme.count() >= len(m.THEMES), "theme selector incomplete")
    check(w.theme.findText("UDDEVALLA BLACK")>=0 and w.theme.findText("UDDEVALLA LIGHT")==-1, "theme selector compatibility/polish failed")
    check(all(w.theme.itemData(i,_Qt.ItemDataRole.DecorationRole) is None for i in range(w.theme.count())), "theme selector contains coloured UX decoration")
    check(w.aspect.count() >= 4, "ratio selector incomplete")
    check(w.transition.count() == 4 and w.transition.itemText(2)=="MORPH" and w.transition.itemText(3)=="REVEAL", "transition selector incomplete")
    check("NIRUPRES AI PRESENTATION BRIEF" in m.export_ai_brief(deck), "AI brief missing")
    repaired,report=m.import_nirupres_markdown_report(md.replace("layout: SECTION","layout: MADE_UP",1))
    check(repaired["slides"][0]["layout"]=="TEXT" and report, "AI markdown normalization/report failed")

    # 1.5.3 literal TWO COLUMN contract. Explicit left/right columns preserve
    # all internal blank lines and never infer headings/groups.
    left_literal=("Högt tempo\nJag behöver ibland bromsa så att alla hinner med\n\n"
                  "Mod att agera\nVåga köra – men också våga erkänna fel, lära och börja om\n\n"
                  "Höga ambitioner\nJag behöver vara vaksam på mina egna krav")
    right_literal=("Självständig\nJag behöver komma ihåg att involvera andra tidigt\n\n"
                   "Många idéer\nAllt behöver inte genomföras samtidigt\n\n"
                   "Dålig fikare\nEtt långvarigt förbättringsområde")
    structured=left_literal+"\n\n|||\n\n"+right_literal
    cols,explicit=m._parse_two_column_body(structured)
    check(explicit, "explicit TWO COLUMN boundary not detected")
    check(cols[0]["text"]==left_literal and cols[1]["text"]==right_literal, "TWO COLUMN literal text changed during parse")
    check(not cols[0]["groups"] and not cols[1]["groups"] and not cols[0]["heading"] and not cols[1]["heading"], "TWO COLUMN inferred forbidden semantics")
    check("\n\n" in cols[0]["text"] and "\n\n" in cols[1]["text"], "TWO COLUMN lost blank-line spacing")
    img=QImage(1280,720,QImage.Format.Format_ARGB32); img.fill(0); qp=QPainter(img)
    literal_slide=m.slide_defaults({"layout":"TWO COLUMN","title":"Literal gate","body":structured,"leftColumn":left_literal,"rightColumn":right_literal,"twoColumnStructured":True})
    m.render_slide(qp,m.QRectF(0,0,1280,720),literal_slide,"B&W",0,1,True,"Noto Sans",False,"")
    check(qp.isActive(), "literal TWO COLUMN render failed"); qp.end()

    # Legacy TWO COLUMN without an explicit delimiter remains readable.
    legacy_cols,legacy_explicit=m._parse_two_column_body("Left one\nLeft two\n\nRight one\nRight two")
    check(not legacy_explicit and legacy_cols[0]["text"]=="Left one\nLeft two" and legacy_cols[1]["text"]=="Right one\nRight two", "legacy TWO COLUMN compatibility failed")
    # Do not grep implementation text for words such as "groups"/"heading":
    # comments are not behavior. The literal parse/render assertions above are
    # the release contract and catch actual semantic inference regressions.

    # 1.4.1 paired typography contract: equivalent columns resolve to one
    # shared font size determined by the denser side.
    r1=m.QRectF(0,0,420,180); r2=m.QRectF(0,0,420,180)
    pf=m._paired_font(["Short", "A much longer paired column that must determine the shared scale across both sides"], "Noto Sans", m.QFont.Normal, 34, 14, [r1,r2])
    check(14 <= pf.pointSize() <= 34, "paired typography fit returned invalid size")

    # 1.4.0 composition/background contract
    check(hasattr(w,"list_style"), "list-style control missing")
    check([w.list_style.itemText(i) for i in range(w.list_style.count())] == ["NUMBERS","DOTS","DASHES","NONE"], "list styles incomplete")
    check(all(hasattr(w,a) for a in ("bgimg","bgmono","bgdim","bgblur","bgfx","bgfy")), "background image controls incomplete")
    bg=m.slide_defaults({"backgroundImage":"/tmp/bg.jpg","backgroundDim":999,"backgroundBlur":999,"listStyle":"DASHES"})
    check(bg["backgroundDim"]==90 and bg["backgroundBlur"]==20 and bg["listStyle"]=="DASHES", "background/list normalization failed")
    bgdeck=m.clone(m.DEFAULT); bgdeck["slides"]=[m.slide_defaults({"layout":"BULLETS","title":"Gate","body":"One\nTwo","listStyle":"DOTS","backgroundImage":"/tmp/bg.jpg","backgroundDim":82,"backgroundBlur":4,"backgroundMono":True,"backgroundFocalX":0.0,"backgroundFocalY":1.0})]
    bgmd=m.export_nirupres_markdown(bgdeck); bgrt=m.import_nirupres_markdown(bgmd)
    for key in ("listStyle","backgroundDim","backgroundBlur","backgroundMono","backgroundFocalX","backgroundFocalY"):
        check(bgrt["slides"][0][key]==bgdeck["slides"][0][key], f"background markdown round-trip failed: {key}")
    # Symmetric composition: TWO COLUMN and COMPARE render at normal size without error.
    for layout,body in (("TWO COLUMN","Left heading\nLeft detail\n\nRight heading\nRight detail"),("COMPARE","NOW\nOne\nTwo\n\nTARGET\nThree\nFour")):
        img=QImage(1280,720,QImage.Format.Format_ARGB32); img.fill(0); qp=QPainter(img)
        m.render_slide(qp,m.QRectF(0,0,1280,720),m.slide_defaults({"layout":layout,"title":"Symmetry","body":body}),"B&W",0,1,True,"Noto Sans",False,"")
        check(qp.isActive(), f"symmetric composition render failed: {layout}"); qp.end()

    # 1.3.0 composition/image contract
    check(hasattr(w,"treatment"), "image treatment control missing")
    check([w.treatment.itemText(i) for i in range(w.treatment.count())] == ["NATURAL","MONO","DIM","CONTRAST"], "image treatments incomplete")
    sample=m.slide_defaults({"layout":"SPLIT","imageTreatment":"DIM"})
    check(sample["imageTreatment"]=="DIM" and not sample["mono"], "DIM treatment normalization failed")
    legacy_mono=m.slide_defaults({"mono":True})
    check(legacy_mono["imageTreatment"]=="MONO" and legacy_mono["mono"], "legacy mono compatibility failed")
    check(w.footer.isEnabled(), "footer editor disabled")
    check(m.normalize_footer_align("center")=="CENTER" and m.normalize_footer_align("bad")=="LEFT", "footer alignment normalization failed")
    check(m.normalize_footer_size("large")=="LARGE" and m.normalize_footer_size("bad")=="MEDIUM", "footer size normalization failed")
    ds=m.DeckSettings(w,deck); check(ds.footer_align.currentText()=="RIGHT" and ds.footer_size.currentText()=="LARGE", "footer deck controls did not load"); ds.close()
    # Structured layouts depend on hard line breaks (TIMELINE, PROCESS, KPI,
    # TABLE, etc.). The canvas text editor must be a plain-text round trip;
    # QTextEdit(text) may pass through Qt text auto-detection and must not be
    # relied on for preserving structural newlines.
    structured="Praktiskt | Lantbruk m.m.\nMusik | Musiker, kompositör, ljudtekniker\nTeknik & IT | Från teknikintresse till professionella IT-roller\nIdag | Digitalisering, AI och informationssäkerhet"
    te=m.TextEditor(w,"En bred arbetslivsresa",structured,"TIMELINE"); app.processEvents()
    check(te.body.toPlainText()==structured, "canvas text editor collapsed structured-layout line breaks")
    check(not te.body.acceptRichText(), "canvas text editor must stay plain-text only")
    # 1.5.3: paired layouts expose left/right fields instead of delimiter syntax.
    structured_body="Left line 1\nLeft line 2\n\nLeft paragraph 2\n\n|||\n\nRight line 1\n\nRight paragraph 2"
    pair=m.TextEditor(w,"Paired",structured_body,"TWO COLUMN"); app.processEvents()
    check(pair.left_body is not None and pair.right_body is not None, "TWO COLUMN did not expose paired editors")
    check("|||" not in pair.left_body.toPlainText()+pair.right_body.toPlainText(), "serialization delimiter leaked into paired editor")
    check("|||" in pair.body_text(), "paired editor did not serialize explicit column boundary")
    check("no automatic headings" in next((x.text() for x in pair.findChildren(m.QLabel) if "automatic headings" in x.text()),""), "TWO COLUMN helper still advertises heading semantics")
    check(not pair.left_body.acceptRichText() and not pair.right_body.acceptRichText(), "paired editors must stay plain text"); pair.close()
    # 1.5.3 persistence regression: once structured, blank lines are paragraph
    # rhythm only and can never move content across columns on a reopen.
    left_src="Högt tempo\nJag behöver ibland bromsa\n\nMod att agera\nVåga köra och börja om"
    right_src="Höga ambitioner\nVar vaksam på egna krav\n\nSjälvständig\nInvolvera andra tidigt"
    persisted=m.slide_defaults({"layout":"TWO COLUMN","body":left_src+"\n\n|||\n\n"+right_src,"leftColumn":left_src,"rightColumn":right_src,"twoColumnStructured":True})
    pair2=m.TextEditor(w,"Stable",persisted["body"],"TWO COLUMN",persisted["leftColumn"],persisted["rightColumn"]); app.processEvents()
    check(pair2.left_body.toPlainText()==left_src and pair2.right_body.toPlainText()==right_src, "structured TWO COLUMN changed sides on reopen")
    img=QImage(1280,720,QImage.Format.Format_ARGB32); img.fill(0); qp=QPainter(img)
    m.render_slide(qp,m.QRectF(0,0,1280,720),persisted,"B&W",0,1,True,"Noto Sans",False,"")
    check(qp.isActive(), "persisted TWO COLUMN render failed"); qp.end(); pair2.close()
    # COMPARE uses the same literal/persistent side contract. This catches the
    # exact class of bug where blank paragraphs moved content to the right.
    cmp_pair=m.TextEditor(w,"Compare stable",left_src+"\n\n|||\n\n"+right_src,"COMPARE",left_src,right_src); app.processEvents()
    check(cmp_pair.left_body.toPlainText()==left_src and cmp_pair.right_body.toPlainText()==right_src, "structured COMPARE changed sides on reopen")
    check("no automatic headings" in next((x.text() for x in cmp_pair.findChildren(m.QLabel) if "automatic headings" in x.text()),""), "COMPARE helper still advertises heading semantics")
    cmp_slide=m.slide_defaults({"layout":"COMPARE","body":cmp_pair.body_text(),"leftColumn":left_src,"rightColumn":right_src,"sideStructured":True})
    img=QImage(1280,720,QImage.Format.Format_ARGB32); img.fill(0); qp=QPainter(img)
    m.render_slide(qp,m.QRectF(0,0,1280,720),cmp_slide,"B&W",0,1,True,"Noto Sans",False,"")
    check(qp.isActive(), "persisted COMPARE render failed"); qp.end(); cmp_pair.close()
    te.close()
    legacy=m.clone(m.DEFAULT); legacy.pop("footerAlign",None); legacy.pop("footerSize",None); check(m.normalize_footer_align(legacy.get("footerAlign"))=="LEFT" and m.normalize_footer_size(legacy.get("footerSize"))=="MEDIUM", "legacy footer defaults failed")
    # Panel toggles must be observable/reversible.
    w.set_side_panel('left',False); check(not w.left_panel.isVisible(), "Slides panel did not hide")
    w.set_side_panel('left',True); check(not w.left_panel.isHidden(), "Slides panel did not restore")
    w.set_side_panel('right',False); check(not w.right_panel.isVisible(), "Inspector did not hide")
    w.set_side_panel('right',True); check(not w.right_panel.isHidden(), "Inspector did not restore")
    # Formatting buttons must act on the last focused paired editor. Buttons use
    # NoFocus so clicking B/I/N cannot silently redirect formatting to LEFT.
    editor=m.TextEditor(None,"Title","","COMPARE","Left","Right")
    # The dialog must be visible before Qt can give a child keyboard focus.
    # 1.5.4 tested a hidden dialog, so focus stayed on the previous window and
    # produced a false failure even though the runtime focus tracking was valid.
    editor.show(); QTest.qWait(30); app.processEvents()
    editor.right_body.setFocus(); QTest.qWait(10); app.processEvents()
    check(QApplication.focusWidget() is editor.right_body, "COMPARE right editor could not receive focus in release gate")
    editor.right_body.selectAll(); QTest.mouseClick(editor.bold_btn,Qt.LeftButton); app.processEvents()
    check(editor.right_body.toPlainText()=="**Right**" and editor.left_body.toPlainText()=="Left", "COMPARE right-side formatting target failed")
    editor.left_body.setPlainText("one\ntwo"); editor.left_body.setFocus(); QTest.qWait(10); app.processEvents(); editor.left_body.selectAll(); QTest.mouseClick(editor.italic_btn,Qt.LeftButton); app.processEvents()
    check(editor.left_body.toPlainText()=="*one*\n*two*", "paired multiline italic formatting failed")
    editor.close()

    # Every layout is rendered with inline emphasis. This catches template-specific
    # parsing that accidentally consumes Markdown markers (notably BULLETS).
    emphasis_image=QImage(1280,720,QImage.Format.Format_ARGB32)
    for layout_name in m.LAYOUTS:
        emphasis_image.fill(0); painter=QPainter(emphasis_image)
        body="**Bold**\n*Italic*"
        if layout_name in ("PROCESS","TIMELINE","MATRIX","TABLE","DATA / KPI","NUMBER GRID"):
            body="**Bold** | *Italic*"
        slide=m.slide_defaults({"layout":layout_name,"title":"**Bold title**","body":body})
        m.render_slide(painter,m.QRectF(0,0,1280,720),slide,"B&W",0,1,True,"Noto Sans",True,"Footer")
        check(painter.isActive(), f"emphasis renderer stopped for {layout_name}"); painter.end()

    # 1.5.3 cinematic/media contract
    check(all(x in m.THEMES for x in ("BEKSINSKI","MARTIN","SFUMATO")), "new art themes missing")
    check("VIDEO" in m.LAYOUTS and "VIDEO + TEXT" in m.LAYOUTS, "video layouts missing")
    media=m.slide_defaults({"layout":"VIDEO","videoSource":"https://youtu.be/dQw4w9WgXcQ","videoPoster":"poster.jpg","backgroundMotion":"KEN BURNS"})
    check(media["backgroundMotion"]=="KEN BURNS" and media["videoSource"].startswith("https://"), "media defaults failed")
    check("youtube-nocookie.com/embed/" in m._web_video_url(media["videoSource"]), "YouTube embed normalization failed")
    md=m.export_nirupres_markdown({**m.clone(m.DEFAULT),"slides":[media]})
    check("video:" in md and "background-motion: KEN BURNS" in md and "cinematic-open:" in md, "Markdown v2 media metadata missing")
    emphasis=m.slide_defaults({"layout":"TEXT","title":"**Bold** title","body":"Normal and *italic* and **bold**"})
    img=QImage(1280,720,QImage.Format.Format_ARGB32); img.fill(0); qp=QPainter(img); m.render_slide(qp,m.QRectF(0,0,1280,720),emphasis,"B&W",0,1,True,"Noto Sans",False,""); check(qp.isActive(), "inline emphasis render failed"); qp.end()
    ds15=m.DeckSettings(w,{**deck,"cinematicOpen":True,"cinematicClose":True}); check(ds15.cinematic_open.isChecked() and ds15.cinematic_close.isChecked(), "cinematic deck controls missing"); ds15.close()
    check(m.normalize_theme("CL")=="CARL LARSSON" and "CARL LARSSON" in m.THEMES, "Carl Larsson theme migration missing")
    te15=m.TextEditor(w,"Title","ordinary text","TEXT"); te15.show(); app.processEvents(); te15.body.setFocus(); QTest.qWait(10); app.processEvents(); te15.body.selectAll(); QTest.mouseClick(te15.bold_btn,Qt.LeftButton); app.processEvents(); check(te15.body.toPlainText()=="**ordinary text**", "B formatting control failed"); te15.body.selectAll(); QTest.mouseClick(te15.normal_btn,Qt.LeftButton); app.processEvents(); check(te15.body.toPlainText()=="ordinary text", "N formatting control failed"); te15.close()
    w.close(); app.processEvents()

    # Presentation keyboard regression gate. This specifically protects the real
    # single-display path that regressed in 1.0.0, plus the two-window Online path
    # which shares the same input controller as physical dual-display Presenter View.
    pdeck=m.clone(m.DEFAULT); pdeck["transition"]="NONE"
    pdeck["slides"]=[
        m.slide_defaults({"title":"One"}),
        m.slide_defaults({"title":"Hidden","hidden":True}),
        m.slide_defaults({"title":"Three"}),
        m.slide_defaults({"title":"Four"}),
    ]
    screen=app.primaryScreen()
    session=m.Presenter(pdeck,0,screen,screen,online=False); app.processEvents()
    check(not qt_callback_errors, f"single-display startup callback error: {qt_callback_errors[0][1] if qt_callback_errors else ''}")
    check(not session.dual and session.audience.isVisible(), "single-display presentation path failed")
    QTest.keyClick(session.audience,Qt.Key_Right); app.processEvents(); check(session.i==2,"single-display Right navigation failed")
    QTest.keyClick(session.audience,Qt.Key_Space); app.processEvents(); check(session.i==3,"single-display Space navigation failed")
    QTest.keyClick(session.audience,Qt.Key_Left); app.processEvents(); check(session.i==2,"single-display Left navigation failed")
    QTest.keyClick(session.audience,Qt.Key_Home); app.processEvents(); check(session.i==0,"single-display Home navigation failed")
    QTest.keyClick(session.audience,Qt.Key_End); app.processEvents(); check(session.i==3,"single-display End navigation failed")
    check(not qt_callback_errors, f"single-display keyboard callback error: {qt_callback_errors[0][1] if qt_callback_errors else ''}")
    # Exercise navigation directly as well: index changes alone are insufficient;
    # draw() must complete without an exception.
    session.navigate(-999); check(session.i==0,"single-display direct Home navigation failed")
    session.navigate(1); check(session.i==2,"single-display direct navigation/redraw failed")
    QTest.keyClick(session.audience,Qt.Key_B); app.processEvents(); check(session.blank=="B","single-display B blackout failed")
    QTest.keyClick(session.audience,Qt.Key_B); app.processEvents(); check(session.blank is None,"single-display B restore failed")
    ended=[]; session.finished.connect(lambda:ended.append(True))
    QTest.keyClick(session.audience,Qt.Key_S); app.processEvents(); check(session.audience.focus_overlay.mode=='SPOTLIGHT',"Spotlight shortcut failed")
    QTest.keyClick(session.audience,Qt.Key_F); app.processEvents(); check(session.audience.focus_overlay.mode=='FOCUS',"Focus Reveal shortcut failed")
    QTest.keyClick(session.audience,Qt.Key_C); app.processEvents(); check(session.curtain and session.audience._curtain is not None and session.audience._curtain.isVisible(),"Audience Curtain failed")
    QTest.keyClick(session.audience,Qt.Key_C); app.processEvents(); check(not session.curtain,"Audience Curtain restore failed")
    check(session.audience.focus_overlay.radius==115 and session.audience.focus_overlay.mode=='FOCUS', "Focus Reveal visual contract failed")
    session.audience.focus_overlay.pin_or_move(m.QPointF(200,180)); check(session.audience.focus_overlay.pinned, "Focus Trail pin failed")
    QTest.keyClick(session.audience,Qt.Key_Z); QTest.qWait(30); app.processEvents(); check(session.audience._zoomed, "Live Zoom activation failed")
    QTest.keyClick(session.audience,Qt.Key_Z); QTest.qWait(330); app.processEvents(); check(not session.audience._zoomed, "Live Zoom restore failed")
    # 1.7.1 cinematic pause: P must dim audience, freeze active presentation time,
    # block navigation, then resume on the same slide without charging PACE for the pause.
    before_pause=session._active_elapsed_ms(); pause_index=session.i
    QTest.keyClick(session.audience,Qt.Key_P); QTest.qWait(1250); app.processEvents()
    check(session.paused and session.audience._paused and session.audience.pause_overlay.isVisible(), "Pause activation failed")
    check(session.audience.pause_overlay.elapsed_seconds()>=1, "Pause count-up timer failed")
    QTest.keyClick(session.audience,Qt.Key_Right); app.processEvents(); check(session.i==pause_index, "Pause allowed slide navigation")
    frozen=session._active_elapsed_ms(); check(abs(frozen-before_pause)<250, "Presenter timer advanced during pause")
    QTest.keyClick(session.audience,Qt.Key_P); QTest.qWait(360); app.processEvents()
    check(not session.paused and not session.audience._paused and not session.audience.pause_overlay.isVisible(), "Pause resume failed")
    check(session.i==pause_index and session._paused_total_ms>=1000, "Pause duration accounting failed")
    QTest.keyClick(session.audience,Qt.Key_Escape); QTest.qWait(2050); app.processEvents(); check(not ended and session._black_hold,"cinematic closing did not enter five-second black hold")
    QTest.keyClick(session.audience,Qt.Key_Escape); QTest.qWait(50); app.processEvents(); check(bool(ended),"second Escape did not skip cinematic black hold")

    online=m.Presenter(pdeck,0,screen,screen,online=True); app.processEvents()
    check(not qt_callback_errors, f"presentation callback error before Online test: {qt_callback_errors[0][1] if qt_callback_errors else ''}")
    check(online.dual and online.isVisible() and online.audience.isVisible(), "Online presentation path failed")
    QTest.keyClick(online.audience,Qt.Key_Right); app.processEvents(); check(online.i==2,"Online Audience keyboard navigation failed")
    QTest.keyClick(online,Qt.Key_Space); app.processEvents(); check(online.i==3,"Online Presenter keyboard navigation failed")
    online.navigate(-999); app.processEvents()
    # Presenter View uses this same visible Presenter + Audience input architecture;
    # verify button navigation too, independent of keyboard focus.
    next_buttons=[b for b in online.findChildren(QPushButton) if b.text()=="NEXT  ›"]
    check(bool(next_buttons),"Presenter NEXT control missing")
    next_buttons[0].click(); app.processEvents(); check(online.i==2,"Presenter NEXT control failed")
    online.close_session(); app.processEvents()
    check(not qt_callback_errors, f"Qt callback error escaped release gate: {qt_callback_errors[0][1] if qt_callback_errors else ''}")
    sys.excepthook=original_excepthook

    print("NIRUPRES release gate: PASS")

if __name__ == "__main__":
    try:
        main()
    except BaseException as exc:
        print(f"NIRUPRES release gate: FAIL — {type(exc).__name__}: {exc}", file=sys.stderr, flush=True)
        raise
