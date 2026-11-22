//! PowerPoint export: each visible slide is a full-slide picture (as in
//! 2.x, "pixel perfect", not editable) with its speaker notes, written as
//! a minimal Office Open XML package.
use crate::zip::Writer;
use std::path::Path;

const EMU_PER_INCH: f64 = 914_400.0;

fn xml_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {}
            c => o.push(c),
        }
    }
    o
}

const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn theme(name: &str) -> String {
    let fills = r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#;
    let line = r#"<a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>"#;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="{name}"><a:themeElements><a:clrScheme name="NIRUPRES"><a:dk1><a:srgbClr val="000000"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F1F1F"/></a:dk2><a:lt2><a:srgbClr val="EEEEEE"/></a:lt2><a:accent1><a:srgbClr val="4F4F4F"/></a:accent1><a:accent2><a:srgbClr val="6F6F6F"/></a:accent2><a:accent3><a:srgbClr val="8F8F8F"/></a:accent3><a:accent4><a:srgbClr val="AFAFAF"/></a:accent4><a:accent5><a:srgbClr val="3F3F3F"/></a:accent5><a:accent6><a:srgbClr val="5F5F5F"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="NIRUPRES"><a:majorFont><a:latin typeface="Noto Sans"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Noto Sans"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="NIRUPRES"><a:fillStyleLst>{fills}{fills}{fills}</a:fillStyleLst><a:lnStyleLst>{line}{line}{line}</a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst>{fills}{fills}{fills}</a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#
    )
}

const CLRMAP: &str = r#"<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>"#;

fn sp_tree(inner: &str) -> String {
    format!(
        r#"<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{inner}</p:spTree>"#
    )
}

fn notes_body(text: &str) -> String {
    let paras: String = text
        .split('\n')
        .map(|l| {
            if l.is_empty() {
                r#"<a:p><a:endParaRPr lang="en-US"/></a:p>"#.to_string()
            } else {
                format!(
                    r#"<a:p><a:r><a:rPr lang="en-US" dirty="0"/><a:t>{}</a:t></a:r></a:p>"#,
                    xml_escape(l)
                )
            }
        })
        .collect();
    paras
}

/// Slides: (PNG bytes, speaker notes).
pub fn write(path: &Path, slides: &[(Vec<u8>, String)], aspect: f64) -> Result<(), String> {
    let cx = (13.333 * EMU_PER_INCH) as i64;
    let cy = (cx as f64 / aspect) as i64;
    let tmp = path.with_extension("pptx.tmp");
    let mut z = Writer::create(&tmp)?;
    let n = slides.len();
    let has_notes = slides.iter().any(|(_, t)| !t.trim().is_empty());
    let mut ct = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/ppt/theme/theme2.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/ppt/notesMasters/notesMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"/><Override PartName="/ppt/presProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"/><Override PartName="/ppt/viewProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"/><Override PartName="/ppt/tableStyles.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#,
    );
    for i in 1..=n {
        ct.push_str(&format!(r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#));
        if !slides[i - 1].1.trim().is_empty() {
            ct.push_str(&format!(r#"<Override PartName="/ppt/notesSlides/notesSlide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>"#));
        }
    }
    ct.push_str("</Types>");
    z.add_deflated("[Content_Types].xml", ct.as_bytes())?;
    z.add_deflated(
        "_rels/.rels",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="{REL}/extended-properties" Target="docProps/app.xml"/></Relationships>"#).as_bytes(),
    )?;
    z.add_deflated(
        "docProps/core.xml",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>NIRUPRES</dc:title><dc:creator>NIRUPRES</dc:creator></cp:coreProperties>"#,
    )?;
    z.add_deflated(
        "docProps/app.xml",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>NIRUPRES {}</Application><Slides>{n}</Slides></Properties>"#, crate::model::VERSION).as_bytes(),
    )?;
    let mut ids = String::new();
    let mut rels = String::new();
    for i in 1..=n {
        ids.push_str(&format!(
            r#"<p:sldId id="{}" r:id="rId{}"/>"#,
            255 + i,
            i + 1
        ));
        rels.push_str(&format!(
            r#"<Relationship Id="rId{}" Type="{REL}/slide" Target="slides/slide{i}.xml"/>"#,
            i + 1
        ));
    }
    let base = n + 2;
    rels.push_str(&format!(r#"<Relationship Id="rId1" Type="{REL}/slideMaster" Target="slideMasters/slideMaster1.xml"/><Relationship Id="rId{}" Type="{REL}/notesMaster" Target="notesMasters/notesMaster1.xml"/><Relationship Id="rId{}" Type="{REL}/theme" Target="theme/theme1.xml"/><Relationship Id="rId{}" Type="{REL}/presProps" Target="presProps.xml"/><Relationship Id="rId{}" Type="{REL}/viewProps" Target="viewProps.xml"/><Relationship Id="rId{}" Type="{REL}/tableStyles" Target="tableStyles.xml"/>"#, base, base + 1, base + 2, base + 3, base + 4));
    let _ = has_notes;
    z.add_deflated(
        "ppt/presentation.xml",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation {NS} saveSubsetFonts="1"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:notesMasterIdLst><p:notesMasterId r:id="rId{base}"/></p:notesMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="{cx}" cy="{cy}"/><p:notesSz cx="6858000" cy="9144000"/><p:defaultTextStyle/></p:presentation>"#).as_bytes(),
    )?;
    z.add_deflated(
        "ppt/_rels/presentation.xml.rels",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#).as_bytes(),
    )?;
    z.add_deflated(
        "ppt/presProps.xml",
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentationPr {NS}/>"#
        )
        .as_bytes(),
    )?;
    z.add_deflated("ppt/viewProps.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:viewPr {NS}><p:normalViewPr><p:restoredLeft sz="15620"/><p:restoredTop sz="94660"/></p:normalViewPr><p:gridSpacing cx="76200" cy="76200"/></p:viewPr>"#).as_bytes())?;
    z.add_deflated("ppt/tableStyles.xml", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:tblStyleLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" def="{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}"/>"#)?;
    z.add_deflated("ppt/theme/theme1.xml", theme("NIRUPRES").as_bytes())?;
    z.add_deflated("ppt/theme/theme2.xml", theme("NIRUPRES Notes").as_bytes())?;
    z.add_deflated(
        "ppt/slideMasters/slideMaster1.xml",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg>{}</p:cSld>{CLRMAP}<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>"#, sp_tree("")).as_bytes(),
    )?;
    z.add_deflated(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="{REL}/theme" Target="../theme/theme1.xml"/></Relationships>"#).as_bytes(),
    )?;
    z.add_deflated(
        "ppt/slideLayouts/slideLayout1.xml",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout {NS} type="blank" preserve="1"><p:cSld name="Blank">{}</p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#, sp_tree("")).as_bytes(),
    )?;
    z.add_deflated(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#).as_bytes(),
    )?;
    let notes_ph = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="381000" y="685800"/><a:ext cx="6096000" cy="3429000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="4343400"/><a:ext cx="5486400" cy="4114800"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody></p:sp>"#;
    z.add_deflated(
        "ppt/notesMasters/notesMaster1.xml",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notesMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg>{}</p:cSld>{CLRMAP}<p:notesStyle/></p:notesMaster>"#, sp_tree(notes_ph)).as_bytes(),
    )?;
    z.add_deflated(
        "ppt/notesMasters/_rels/notesMaster1.xml.rels",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/theme" Target="../theme/theme2.xml"/></Relationships>"#).as_bytes(),
    )?;
    for (i, (png, notes)) in slides.iter().enumerate() {
        let k = i + 1;
        z.add_stored(&format!("ppt/media/image{k}.png"), png)?;
        let pic = format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="2" name="Picture 1"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId2"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
        );
        z.add_deflated(
            &format!("ppt/slides/slide{k}.xml"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld {NS}><p:cSld>{}</p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#,
                sp_tree(&pic)
            )
            .as_bytes(),
        )?;
        let has = !notes.trim().is_empty();
        let notes_rel = if has {
            format!(
                r#"<Relationship Id="rId3" Type="{REL}/notesSlide" Target="../notesSlides/notesSlide{k}.xml"/>"#
            )
        } else {
            String::new()
        };
        z.add_deflated(
            &format!("ppt/slides/_rels/slide{k}.xml.rels"),
            format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="{REL}/image" Target="../media/image{k}.png"/>{notes_rel}</Relationships>"#).as_bytes(),
        )?;
        if has {
            let body = format!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{}</p:txBody></p:sp>"#,
                notes_body(notes.trim())
            );
            z.add_deflated(
                &format!("ppt/notesSlides/notesSlide{k}.xml"),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notes {NS}><p:cSld>{}</p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#,
                    sp_tree(&body)
                )
                .as_bytes(),
            )?;
            z.add_deflated(
                &format!("ppt/notesSlides/_rels/notesSlide{k}.xml.rels"),
                format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/notesMaster" Target="../notesMasters/notesMaster1.xml"/><Relationship Id="rId2" Type="{REL}/slide" Target="../slides/slide{k}.xml"/></Relationships>"#).as_bytes(),
            )?;
        }
    }
    z.finish()?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}
