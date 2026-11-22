//! NIRUPRES Markdown v2: export, import (with the normalization report)
//! and the provider-independent AI brief.
use crate::model::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const AI_TEMPLATE: &str = include_str!("../assets/ai-template.md");

fn md_bool(v: &str) -> bool {
    matches!(
        v.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn lower_bool(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

/// Find `<!--\s*KIND\s*\n(.*?)-->` (case-insensitive): the block body.
fn find_block<'a>(text: &'a str, kind: &str) -> Option<&'a str> {
    let lower = text.to_lowercase();
    // Lowercasing can change byte lengths for some scripts; fall back to a
    // byte-for-byte scan in that case.
    let same = lower.len() == text.len();
    let hay = if same { lower.as_str() } else { text };
    let kind_l = kind.to_lowercase();
    let mut from = 0;
    while let Some(off) = hay[from..].find("<!--") {
        let start = from + off;
        let mut i = start + 4;
        let b = hay.as_bytes();
        while i < b.len() && (b[i] as char).is_ascii_whitespace() {
            i += 1;
        }
        let matches_kind = if same {
            hay[i..].starts_with(&kind_l)
        } else {
            hay.get(i..i + kind.len())
                .is_some_and(|s| s.eq_ignore_ascii_case(kind))
        };
        if matches_kind {
            let mut j = i + kind.len();
            let mut last_nl = None;
            while j < b.len() && (b[j] as char).is_ascii_whitespace() {
                if b[j] == b'\n' {
                    last_nl = Some(j);
                }
                j += 1;
            }
            if let Some(nl) = last_nl {
                let body_start = nl + 1;
                if let Some(end) = text[body_start..].find("-->") {
                    return Some(&text[body_start..body_start + end]);
                }
            }
        }
        from = start + 4;
    }
    None
}

/// `_meta_block`: `key: value` lines of a block, keys lower-cased.
pub fn meta_block(text: &str, kind: &str) -> Vec<(String, String)> {
    let Some(body) = find_block(text, kind) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = Vec::new();
    for line in py_splitlines(body) {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_lowercase();
            let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
            if let Some(e) = out.iter_mut().find(|(x, _)| *x == k) {
                e.1 = v;
            } else {
                out.push((k, v));
            }
        }
    }
    out
}

struct Meta(Vec<(String, String)>);

impl Meta {
    fn get(&self, k: &str, d: &str) -> String {
        self.0
            .iter()
            .find(|(x, _)| x == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| d.to_string())
    }
    fn num(&self, k: &str, d: f64) -> f64 {
        match self.0.iter().find(|(x, _)| x == k) {
            Some((_, v)) => v.trim().parse::<f64>().unwrap_or(d),
            None => d,
        }
    }
}

/// Remove every `<!-- ... -->` comment.
fn strip_comments(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find("<!--") {
        out.push_str(&rest[..a]);
        match rest[a + 4..].find("-->") {
            Some(b) => rest = &rest[a + 4 + b + 3..],
            None => {
                out.push_str(&rest[a..]);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Split on lines that are only `---` (plus trailing whitespace).
fn split_slides(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if bare.starts_with("---") && bare[3..].chars().all(char::is_whitespace) {
            parts.push(std::mem::take(&mut cur));
        } else {
            cur.push_str(line);
        }
    }
    parts.push(cur);
    parts
}

fn has_url_scheme(s: &str) -> bool {
    match s.find("://") {
        Some(i) => i > 0 && s[..i].chars().all(|c| c.is_ascii_alphabetic()),
        None => false,
    }
}

fn resolve(base: &Path, p: &str) -> String {
    let joined = base.join(p);
    let r = std::fs::canonicalize(&joined).unwrap_or_else(|_| normalize(&joined));
    r.to_string_lossy().into_owned()
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn export(data: &Obj) -> String {
    let d = Deck(data);
    let mut lines: Vec<String> = vec![
        "<!-- deck".into(),
        format!("nirupres: {MD_FORMAT}"),
        format!(
            "title: {}",
            py_str(Some(data.get("title").unwrap_or(&json!("Untitled"))))
        ),
        format!(
            "theme: {}",
            py_str(Some(data.get("theme").unwrap_or(&json!("B&W"))))
        ),
        format!(
            "font: {}",
            py_str(Some(data.get("fontFamily").unwrap_or(&json!("Noto Sans"))))
        ),
        format!(
            "ratio: {}",
            py_str(Some(data.get("aspect").unwrap_or(&json!("16:9"))))
        ),
        format!("numbers: {}", lower_bool(b_get(data, "showNumbers"))),
        format!("logo: {}", lower_bool(d.show_logo())),
        format!("footer: {}", s_get(data, "footerText", "")),
        format!(
            "footer-align: {}",
            normalize_footer_align(&s_get(data, "footerAlign", "LEFT"))
        ),
        format!(
            "footer-size: {}",
            normalize_footer_size(&s_get(data, "footerSize", "MEDIUM"))
        ),
        format!(
            "transition: {}",
            normalize_transition(data.get("transition"), false)
        ),
        format!("reduce-motion: {}", lower_bool(b_get(data, "reduceMotion"))),
        format!(
            "cinematic-open: {}",
            lower_bool(d.flag_default_true("cinematicOpen"))
        ),
        format!(
            "cinematic-close: {}",
            lower_bool(d.flag_default_true("cinematicClose"))
        ),
        "-->".into(),
        String::new(),
    ];
    let slides = d.slides();
    for (n, s) in slides.iter().enumerate() {
        let Some(s) = s.as_object() else { continue };
        let g = |k: &str, dflt: Value| py_str(Some(s.get(k).unwrap_or(&dflt)));
        let mono = b_get(s, "mono");
        lines.extend([
            "<!-- slide".to_string(),
            format!("layout: {}", g("layout", json!("TITLE"))),
            format!("theme: {}", g("themeOverride", json!("DECK"))),
            format!("font: {}", g("fontFamily", json!("DECK"))),
            format!("hidden: {}", lower_bool(b_get(s, "hidden"))),
            format!("image-mode: {}", g("imageMode", json!("FILL"))),
            format!("mono: {}", lower_bool(mono)),
            format!(
                "image-treatment: {}",
                g(
                    "imageTreatment",
                    json!(if mono { "MONO" } else { "NATURAL" })
                )
            ),
            format!("focal-x: {}", g("focalX", json!(0.5))),
            format!("focal-y: {}", g("focalY", json!(0.5))),
            format!("align: {}", g("align", json!("LEFT"))),
            format!("weight: {}", g("weight", json!("AUTO"))),
            format!("font-scale: {}", g("fontScale", json!(1.0))),
            format!("brightness: {}", g("brightness", json!(0))),
            format!("contrast: {}", g("contrast", json!(0))),
            format!("overlay: {}", g("overlay", json!(0))),
            format!("blur: {}", g("blur", json!(0))),
            format!("list-style: {}", g("listStyle", json!("NUMBERS"))),
            format!("background-dim: {}", g("backgroundDim", json!(75))),
            format!("background-blur: {}", g("backgroundBlur", json!(0))),
            format!(
                "background-mono: {}",
                lower_bool(b_get(s, "backgroundMono"))
            ),
            format!("background-focal-x: {}", g("backgroundFocalX", json!(0.5))),
            format!("background-focal-y: {}", g("backgroundFocalY", json!(0.5))),
            format!(
                "background-motion: {}",
                g("backgroundMotion", json!("NONE"))
            ),
            format!("logo: {}", g("logoMode", json!("AUTO"))),
            format!("accent: {}", g("accentOverride", json!("AUTO"))),
            format!(
                "transition: {}",
                normalize_transition(s.get("transition").or(Some(&json!("DECK"))), true)
            ),
            format!("video-autoplay: {}", lower_bool(b_get(s, "videoAutoplay"))),
            format!("video-muted: {}", lower_bool(b_get(s, "videoMuted"))),
        ]);
        for (k, f) in [
            ("image", "image"),
            ("backgroundImage", "background-image"),
            ("videoSource", "video"),
            ("videoPoster", "video-poster"),
            ("imageRequest", "image-query"),
            ("caption", "caption"),
        ] {
            if truthy(s.get(k)) {
                lines.push(format!("{f}: {}", py_str(s.get(k))));
            }
        }
        lines.push("-->".into());
        lines.push(String::new());
        lines.push(
            format!("# {}", g("title", json!("")))
                .trim_end()
                .to_string(),
        );
        lines.push(String::new());
        let body = py_str(
            s.get("body")
                .filter(|v| truthy(Some(v)))
                .or(Some(&json!(""))),
        );
        if truthy(s.get("body")) && s_get(s, "layout", "") == "CODE" {
            lines.push("```".into());
            lines.push(body.replace("```", "``\u{200b}`"));
            lines.push("```".into());
            lines.push(String::new());
        } else if truthy(s.get("body")) {
            lines.push(body);
            lines.push(String::new());
        }
        if truthy(s.get("notes")) {
            lines.push("<!-- notes".into());
            lines.push(py_str(s.get("notes")));
            lines.push("-->".into());
            lines.push(String::new());
        }
        if n + 1 < slides.len() {
            lines.push("---".into());
            lines.push(String::new());
        }
    }
    format!("{}\n", lines.join("\n").trim_end())
}

pub fn import(text: &str, base: Option<&Path>) -> Result<Obj, String> {
    let deck = Meta(meta_block(text, "deck"));
    let mut data = default_deck();
    let theme = normalize_theme(&deck.get("theme", "B&W"));
    let theme = if crate::model::theme(&theme).is_some() {
        theme
    } else {
        "B&W".into()
    };
    let ratio = deck.get("ratio", "16:9");
    let ratio = if ASPECTS.contains(&ratio.as_str()) {
        ratio
    } else {
        "16:9".into()
    };
    data.insert("title".into(), json!(deck.get("title", "Untitled")));
    data.insert("theme".into(), json!(theme));
    data.insert("fontFamily".into(), json!(deck.get("font", "Noto Sans")));
    data.insert("aspect".into(), json!(ratio));
    data.insert(
        "showNumbers".into(),
        json!(md_bool(&deck.get("numbers", "false"))),
    );
    data.insert("showLogo".into(), json!(md_bool(&deck.get("logo", "true"))));
    data.insert("footerText".into(), json!(deck.get("footer", "")));
    data.insert(
        "footerAlign".into(),
        json!(normalize_footer_align(&deck.get("footer-align", "LEFT"))),
    );
    data.insert(
        "footerSize".into(),
        json!(normalize_footer_size(&deck.get("footer-size", "MEDIUM"))),
    );
    data.insert(
        "transition".into(),
        json!(normalize_transition_str(
            &deck.get("transition", "NONE"),
            false
        )),
    );
    data.insert(
        "reduceMotion".into(),
        json!(md_bool(&deck.get("reduce-motion", "false"))),
    );
    data.insert(
        "cinematicOpen".into(),
        json!(md_bool(&deck.get("cinematic-open", "true"))),
    );
    data.insert(
        "cinematicClose".into(),
        json!(md_bool(&deck.get("cinematic-close", "true"))),
    );
    let mut slides: Vec<Value> = Vec::new();
    for part in split_slides(text) {
        let meta = Meta(meta_block(&part, "slide"));
        if meta.0.is_empty() {
            continue;
        }
        let layout = meta.get("layout", "TITLE").to_uppercase();
        let layout = if LAYOUTS.contains(&layout.as_str()) {
            layout
        } else {
            "TEXT".into()
        };
        let clean = strip_comments(&part).trim().to_string();
        let ls = py_splitlines(&clean);
        let (title, mut body) = if ls.first().is_some_and(|l| l.trim_start().starts_with("# ")) {
            (
                ls[0].trim_start()[2..].trim().to_string(),
                ls[1..].join("\n").trim().to_string(),
            )
        } else {
            (String::new(), clean.clone())
        };
        if layout == "CODE" {
            let raw = strip_comments(&part);
            let ls_raw: Vec<&str> = raw.split('\n').collect();
            let fence: Vec<usize> = ls_raw
                .iter()
                .enumerate()
                .filter(|(_, q)| q.trim().starts_with("```"))
                .map(|(i, _)| i)
                .collect();
            if fence.len() >= 2 {
                body = ls_raw[fence[0] + 1..fence[fence.len() - 1]]
                    .join("\n")
                    .replace("``\u{200b}`", "```");
            }
        }
        let fix_path = |v: String| -> String {
            if v.is_empty() {
                return v;
            }
            if has_url_scheme(&v) {
                return String::new();
            }
            match base {
                Some(b) if !Path::new(&v).is_absolute() => resolve(b, &v),
                _ => v,
            }
        };
        let img = fix_path(meta.get("image", ""));
        let bgimg = fix_path(meta.get("background-image", ""));
        let imode = meta.get("image-mode", "FILL").to_uppercase();
        let imode = if imode == "FIT" || imode == "FILL" {
            imode
        } else {
            "FILL".into()
        };
        let align = meta.get("align", "LEFT").to_uppercase();
        let align = if ["LEFT", "CENTER", "RIGHT"].contains(&align.as_str()) {
            align
        } else {
            "LEFT".into()
        };
        let weight = meta.get("weight", "AUTO").to_uppercase();
        let weight = if ["AUTO", "LIGHT", "REGULAR", "BOLD"].contains(&weight.as_str()) {
            weight
        } else {
            "AUTO".into()
        };
        let mono = md_bool(&meta.get("mono", "false"));
        let mut s = json!({
            "layout": layout,
            "title": title,
            "body": body,
            "themeOverride": normalize_theme(&meta.get("theme", "DECK")),
            "fontFamily": meta.get("font", "DECK"),
            "hidden": md_bool(&meta.get("hidden", "false")),
            "image": img,
            "imageMode": imode,
            "mono": mono,
            "imageTreatment": meta.get("image-treatment", if mono { "MONO" } else { "NATURAL" }).to_uppercase(),
            "listStyle": meta.get("list-style", "NUMBERS").to_uppercase(),
            "backgroundImage": bgimg,
            "backgroundDim": (meta.num("background-dim", 75.0) as i64).clamp(0, 90),
            "backgroundBlur": (meta.num("background-blur", 0.0) as i64).clamp(0, 20),
            "backgroundMono": md_bool(&meta.get("background-mono", "false")),
            "backgroundFocalX": meta.num("background-focal-x", 0.5).clamp(0.0, 1.0),
            "backgroundFocalY": meta.num("background-focal-y", 0.5).clamp(0.0, 1.0),
            "backgroundMotion": meta.get("background-motion", "NONE").to_uppercase(),
            "focalX": meta.num("focal-x", 0.5).clamp(0.0, 1.0),
            "focalY": meta.num("focal-y", 0.5).clamp(0.0, 1.0),
            "align": align,
            "weight": weight,
            "fontScale": meta.num("font-scale", 1.0).clamp(0.5, 2.0),
            "brightness": (meta.num("brightness", 0.0) as i64).clamp(-30, 30),
            "contrast": (meta.num("contrast", 0.0) as i64).clamp(0, 50),
            "overlay": (meta.num("overlay", 0.0) as i64).clamp(0, 60),
            "blur": (meta.num("blur", 0.0) as i64).clamp(0, 10),
            "caption": meta.get("caption", ""),
            "imageRequest": meta.get("image-query", ""),
            "logoMode": meta.get("logo", "AUTO").to_uppercase(),
            "accentOverride": meta.get("accent", "AUTO").to_uppercase(),
            "transition": normalize_transition_str(&meta.get("transition", "DECK"), true),
            "videoSource": meta.get("video", ""),
            "videoPoster": meta.get("video-poster", ""),
            "videoAutoplay": md_bool(&meta.get("video-autoplay", "false")),
            "videoMuted": md_bool(&meta.get("video-muted", "false")),
        });
        let mut s = slide_defaults(s.as_object_mut().map(|o| &*o));
        for k in ["videoSource", "videoPoster"] {
            let v = s_get(&s, k, "");
            if !v.is_empty() && !has_url_scheme(&v) {
                if let Some(b) = base {
                    if !Path::new(&v).is_absolute() {
                        s.insert(k.into(), json!(resolve(b, &v)));
                    }
                }
            }
        }
        let notes = find_block(&part, "notes")
            .map(|n| n.trim().to_string())
            .unwrap_or_default();
        s.insert("notes".into(), json!(notes));
        let ov = s_get(&s, "themeOverride", "DECK");
        if ov != "DECK" && crate::model::theme(&ov).is_none() {
            s.insert("themeOverride".into(), json!("DECK"));
        }
        slides.push(Value::Object(s));
    }
    if slides.is_empty() {
        return Err("No valid <!-- slide ... --> blocks found.".into());
    }
    data.insert("slides".into(), Value::Array(slides));
    data.insert("format".into(), json!(FORMAT));
    data.insert("appVersion".into(), json!(VERSION));
    Ok(data)
}

/// Import with the list of normalizations that were made.
pub fn import_report(text: &str, base: Option<&Path>) -> Result<(Obj, Vec<String>), String> {
    let mut report = Vec::new();
    let deck = Meta(meta_block(text, "deck"));
    let raw_theme = deck.get("theme", "B&W");
    let raw_ratio = deck.get("ratio", "16:9");
    let raw_trans = deck.get("transition", "NONE");
    let raw_fa = deck.get("footer-align", "LEFT");
    let raw_fs = deck.get("footer-size", "MEDIUM");
    let data = import(text, base)?;
    if crate::model::theme(&normalize_theme(&raw_theme)).is_none() {
        report.push(format!("Unknown deck theme '{raw_theme}' → B&W"));
    }
    if !ASPECTS.contains(&raw_ratio.as_str()) {
        report.push(format!("Unknown ratio '{raw_ratio}' → 16:9"));
    }
    if !["NONE", "FADE", "MORPH", "REVEAL", "CUT"].contains(&raw_trans.to_uppercase().as_str()) {
        report.push(format!("Unknown transition '{raw_trans}' → NONE"));
    }
    if !["LEFT", "CENTER", "RIGHT"].contains(&raw_fa.to_uppercase().as_str()) {
        report.push(format!("Unknown footer alignment '{raw_fa}' → LEFT"));
    }
    if !["SMALL", "MEDIUM", "LARGE"].contains(&raw_fs.to_uppercase().as_str()) {
        report.push(format!("Unknown footer size '{raw_fs}' → MEDIUM"));
    }
    let parts: Vec<String> = split_slides(text)
        .into_iter()
        .filter(|p| !meta_block(p, "slide").is_empty())
        .collect();
    for (idx, part) in parts.iter().enumerate() {
        let idx = idx + 1;
        let m = Meta(meta_block(part, "slide"));
        let layout = m.get("layout", "TITLE").to_uppercase();
        if !LAYOUTS.contains(&layout.as_str()) {
            report.push(format!("Slide {idx}: unknown layout '{layout}' → TEXT"));
        }
        if !["AUTO", "ON", "OFF"].contains(&m.get("logo", "AUTO").to_uppercase().as_str()) {
            report.push(format!("Slide {idx}: invalid logo mode → AUTO"));
        }
        if !["AUTO", "BLUE", "GREEN", "YELLOW", "RED", "PINK", "PURPLE"]
            .contains(&m.get("accent", "AUTO").to_uppercase().as_str())
        {
            report.push(format!("Slide {idx}: invalid accent → AUTO"));
        }
        if !["DECK", "NONE", "FADE", "MORPH", "REVEAL", "CUT"]
            .contains(&m.get("transition", "DECK").to_uppercase().as_str())
        {
            report.push(format!("Slide {idx}: invalid transition → DECK"));
        }
        if has_url_scheme(&m.get("image", "")) {
            report.push(format!(
                "Slide {idx}: remote image URL removed; use image-query instead"
            ));
        }
        if !["FIT", "FILL"].contains(&m.get("image-mode", "FILL").to_uppercase().as_str()) {
            report.push(format!("Slide {idx}: invalid image mode → FILL"));
        }
        if !["LEFT", "CENTER", "RIGHT"].contains(&m.get("align", "LEFT").to_uppercase().as_str()) {
            report.push(format!("Slide {idx}: invalid alignment → LEFT"));
        }
        if !["AUTO", "LIGHT", "REGULAR", "BOLD"]
            .contains(&m.get("weight", "AUTO").to_uppercase().as_str())
        {
            report.push(format!("Slide {idx}: invalid font weight → AUTO"));
        }
    }
    Ok((data, report))
}

pub fn ai_brief(data: &Obj) -> String {
    format!(
        "{AI_TEMPLATE}\n\n## CURRENT PRESENTATION\n\n{}",
        export(data)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_slides_and_notes() {
        let mut d = default_deck();
        normalize_loaded(&mut d, "x");
        d["slides"][0]["notes"] = json!("secret");
        let mut code =
            slide_defaults(json!({"layout": "CODE", "title": "C", "body": "a\n\n  b"}).as_object());
        code.insert("id".into(), json!("1"));
        d["slides"]
            .as_array_mut()
            .unwrap()
            .push(Value::Object(code));
        let md = export(&d);
        let back = import(&md, None).unwrap();
        assert_eq!(back["slides"].as_array().unwrap().len(), 2);
        assert_eq!(back["slides"][0]["notes"], "secret");
        assert_eq!(back["slides"][0]["title"], "NIRUPRES");
        assert_eq!(back["slides"][1]["layout"], "CODE");
    }

    #[test]
    fn blocks_are_found_case_insensitively() {
        let t = "<!-- DECK\ntitle: \"Hi\"\n-->\n";
        assert_eq!(
            meta_block(t, "deck"),
            vec![("title".to_string(), "Hi".to_string())]
        );
        assert!(import("nothing here", None).is_err());
        let (_, rep) = import_report(
            "<!-- deck\nratio: 2:1\n-->\n<!-- slide\nlayout: WHAT\n-->\n# A",
            None,
        )
        .unwrap();
        assert!(rep.iter().any(|r| r.contains("ratio")));
        assert!(rep.iter().any(|r| r.contains("unknown layout")));
    }
}
