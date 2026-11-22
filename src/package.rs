//! `.nirupres` packages: one ZIP with `project.json` and `assets/`. Loading
//! keeps every 2.x safety check (paths, file types, counts, sizes); saving
//! embeds images (optimized as chosen), local video and verifies the new
//! package before it replaces the old file.
use crate::images;
use crate::model::*;
use crate::zip;
use serde_json::Value;
use std::path::{Path, PathBuf};

// ------------------------------------------------------ Python-like JSON

fn esc(s: &str, ascii: bool, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if ascii && (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04x}", u));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn num_json(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    let f = n.as_f64().unwrap_or(0.0);
    if f == f.trunc() && f.abs() < 1e16 {
        format!("{f:.1}")
    } else {
        let s = format!("{f}");
        s
    }
}

fn write_val(v: &Value, indent: Option<usize>, level: usize, ascii: bool, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&num_json(n)),
        Value::String(s) => esc(s, ascii, out),
        Value::Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(if indent.is_some() { ',' } else { ',' });
                    if indent.is_none() {
                        out.push(' ');
                    }
                }
                if let Some(n) = indent {
                    out.push('\n');
                    out.push_str(&" ".repeat(n * (level + 1)));
                }
                write_val(x, indent, level + 1, ascii, out);
            }
            if let Some(n) = indent {
                out.push('\n');
                out.push_str(&" ".repeat(n * level));
            }
            out.push(']');
        }
        Value::Object(o) => {
            if o.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, x)) in o.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                    if indent.is_none() {
                        out.push(' ');
                    }
                }
                if let Some(n) = indent {
                    out.push('\n');
                    out.push_str(&" ".repeat(n * (level + 1)));
                }
                esc(k, ascii, out);
                out.push_str(": ");
                write_val(x, indent, level + 1, ascii, out);
            }
            if let Some(n) = indent {
                out.push('\n');
                out.push_str(&" ".repeat(n * level));
            }
            out.push('}');
        }
    }
}

/// `json.dumps(v)` (ASCII escapes, ", " and ": " separators).
pub fn py_json(v: &Value) -> String {
    let mut s = String::new();
    write_val(v, None, 0, true, &mut s);
    s
}

/// `json.dumps(v, indent=2)` (ASCII escapes).
pub fn py_json_pretty(v: &Value) -> String {
    let mut s = String::new();
    write_val(v, Some(2), 0, true, &mut s);
    s
}

/// `json.dumps(v, indent=2, ensure_ascii=False)`: the project file.
pub fn py_json_project(v: &Value) -> String {
    let mut s = String::new();
    write_val(v, Some(2), 0, false, &mut s);
    s
}

/// `json.dumps(v, ensure_ascii=False)`: the autosave file.
pub fn py_json_utf8(v: &Value) -> String {
    let mut s = String::new();
    write_val(v, None, 0, false, &mut s);
    s
}

// ------------------------------------------------------------- loading

pub fn safe_zip_name(name: &str) -> bool {
    let name = name.replace('\\', "/");
    if name.is_empty() || name.starts_with('/') {
        return false;
    }
    let b = name.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return false;
    }
    !name.split('/').any(|p| p == "..")
}

fn stem(p: &Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

const S_IFMT: u32 = 0o170000;
const S_IFREG: u32 = 0o100000;
const S_IFDIR: u32 = 0o040000;
const S_IFLNK: u32 = 0o120000;

/// `load_project`: read a package (or a plain project.json) and extract
/// its assets into `extract_dir`.
pub fn load_project(path: &Path, extract_dir: &Path) -> Result<Obj, String> {
    let mut data: Value;
    if zip::is_zip(path) {
        let root = std::fs::canonicalize(extract_dir).unwrap_or_else(|_| extract_dir.to_path_buf());
        let mut z = zip::Reader::open(path).map_err(|e| format!("Invalid NIRUPRES package: {e}"))?;
        if z.entries.len() > MAX_ASSETS + 8 {
            return Err("Project package contains too many files.".into());
        }
        let mut assets = 0usize;
        let mut total = 0u64;
        let mut project_idx = None;
        for (i, e) in z.entries.iter().enumerate() {
            let name = e.name.replace('\\', "/");
            if !safe_zip_name(&name) {
                return Err(format!("Unsafe path in project package: {name}"));
            }
            let mode = (e.external_attr >> 16) & 0xFFFF;
            let ftype = mode & S_IFMT;
            if ftype == S_IFLNK || (ftype != 0 && ftype != S_IFREG && ftype != S_IFDIR) {
                return Err(format!("Unsupported file type in package: {name}"));
            }
            if name != "project.json" && !name.starts_with("assets/") && !name.ends_with('/') {
                return Err(format!("Unexpected file in project package: {name}"));
            }
            if name.starts_with("assets/") && !name.ends_with('/') {
                assets += 1;
            }
            total += e.size;
            if total > MAX_PACKAGE_UNPACKED {
                return Err("Project package is too large when unpacked.".into());
            }
            if name == "project.json" {
                if e.size > MAX_PROJECT_JSON {
                    return Err("project.json is too large.".into());
                }
                project_idx = Some(i);
            }
        }
        let Some(pi) = project_idx else {
            return Err("Invalid NIRUPRES package: project.json is missing.".into());
        };
        if assets > MAX_ASSETS {
            return Err(format!("Project contains too many assets (maximum {MAX_ASSETS})."));
        }
        let bytes = z.read(pi)?;
        let text = String::from_utf8(bytes).map_err(|_| "project.json is not UTF-8.".to_string())?;
        data = serde_json::from_str(&text).map_err(|e| format!("Invalid project.json: {e}"))?;
        validate_project_data(&data)?;
        for i in 0..z.entries.len() {
            let name = z.entries[i].name.replace('\\', "/");
            if !name.starts_with("assets/") || name.ends_with('/') {
                continue;
            }
            let target = root.join(&name);
            if !target.starts_with(&root) || name.split('/').any(|p| p == "..") {
                return Err(format!("Unsafe asset path: {name}"));
            }
            if let Some(d) = target.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            let mut f = std::fs::File::create(&target).map_err(|e| e.to_string())?;
            z.extract(i, &mut f)?;
        }
        if let Some(Value::Array(slides)) = data.get_mut("slides") {
            for s in slides.iter_mut() {
                let Some(s) = s.as_object_mut() else { continue };
                for field in ["image", "backgroundImage", "videoPoster", "videoSource"] {
                    let v = s_get(s, field, "");
                    if let Some(rel) = v.strip_prefix("asset:") {
                        let rel = rel.replace('\\', "/");
                        let nv = if !safe_zip_name(&rel) || rel.contains('/') {
                            String::new()
                        } else {
                            root.join("assets").join(&rel).to_string_lossy().into_owned()
                        };
                        s.insert(field.into(), Value::String(nv));
                    }
                }
            }
        }
    } else {
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if meta.len() > MAX_PROJECT_JSON {
            return Err("Project JSON is too large.".into());
        }
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        data = serde_json::from_str(&text).map_err(|e| format!("Invalid project: {e}"))?;
        validate_project_data(&data)?;
    }
    let Value::Object(mut obj) = data else {
        return Err("Invalid project: project.json must contain an object.".into());
    };
    normalize_loaded(&mut obj, &stem(path));
    obj.insert("format".into(), Value::from(FORMAT));
    obj.insert("appVersion".into(), Value::from(VERSION));
    Ok(obj)
}

// -------------------------------------------------------------- saving

pub struct SaveStats {
    pub images: usize,
    pub saved_bytes: u64,
}

fn is_url(s: &str) -> bool {
    let mut it = s.splitn(2, "://");
    let scheme = it.next().unwrap_or("");
    it.next().is_some() && !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphabetic())
}

pub fn has_scheme(s: &str) -> bool {
    is_url(s)
}

/// `_optimize_asset`: (asset file name, bytes before, bytes after, data).
pub fn optimize_asset(src: &Path, quality: &str, keep: bool) -> Result<(String, u64, u64, Vec<u8>), String> {
    let data = std::fs::read(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let before = data.len() as u64;
    let ext_of = |p: &Path| {
        p.extension()
            .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
            .unwrap_or_else(|| ".bin".into())
    };
    if quality == "ORIGINAL" || keep {
        let digest = images::sha256_hex(&data);
        let name = if keep {
            src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        } else {
            format!("{}{}", &digest[..20], ext_of(src))
        };
        return Ok((name, before, before, data));
    }
    let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let info = images::info_bytes(&data)
        .filter(|i| i.w > 0 && i.h > 0 && (i.w as u64) * (i.h as u64) <= MAX_IMAGE_PIXELS)
        .ok_or_else(|| format!("Unsupported or excessively large image: {name}"))?;
    let mut img = images::decode(&data, None).ok_or_else(|| format!("Could not decode image: {name}"))?;
    drop(data);
    let max_edge = if quality == "HIGH" { 4096 } else { 3200 };
    if img.w.max(img.h) > max_edge {
        let (w, h) = images::qt_scaled(img.w, img.h, max_edge, max_edge, false);
        img = images::resize(&img, w, h);
    }
    let suffix = ext_of(src);
    let (bytes, ext) = if info.alpha || suffix == ".png" || suffix == ".bmp" {
        (images::encode_png(&img), ".png")
    } else {
        let q = if quality == "HIGH" { 94 } else { 88 };
        (images::encode_jpeg(&img, q), ".jpg")
    };
    let bytes = bytes.ok_or_else(|| format!("Could not encode image: {name}"))?;
    let digest = images::sha256_hex(&bytes);
    let after = bytes.len() as u64;
    Ok((format!("{}{ext}", &digest[..20]), before, after, bytes))
}

/// Save `data` to `path` as a verified package. `extract` is the folder
/// assets of the open package live in: those are kept byte for byte.
pub fn save_project(data: &Obj, path: &Path, quality: &str, extract: Option<&Path>) -> Result<SaveStats, String> {
    let mut payload = data.clone();
    payload.insert("format".into(), Value::from(FORMAT));
    payload.insert("appVersion".into(), Value::from(VERSION));
    if let Some(d) = path.parent() {
        if !d.as_os_str().is_empty() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
    }
    let extract_assets = extract.map(|e| {
        std::fs::canonicalize(e.join("assets")).unwrap_or_else(|_| e.join("assets"))
    });
    let mut used: Vec<(String, String)> = Vec::new();
    let mut blobs: Vec<(String, Vec<u8>)> = Vec::new();
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let mut stats = SaveStats {
        images: 0,
        saved_bytes: 0,
    };
    let (mut before_total, mut after_total) = (0u64, 0u64);
    if let Some(Value::Array(slides)) = payload.get_mut("slides") {
        for s in slides.iter_mut() {
            let Some(s) = s.as_object_mut() else { continue };
            for field in ["image", "backgroundImage", "videoPoster"] {
                let v = s_get(s, field, "");
                if v.is_empty() {
                    continue;
                }
                let src = PathBuf::from(&v);
                if !src.exists() {
                    continue;
                }
                let real = std::fs::canonicalize(&src).unwrap_or(src.clone());
                let key = format!("{}|{quality}", real.display());
                let name = match used.iter().find(|(k, _)| *k == key) {
                    Some((_, n)) => n.clone(),
                    None => {
                        let keep = extract_assets.as_ref().is_some_and(|a| real.starts_with(a));
                        let (n, b, a, bytes) = optimize_asset(&real, quality, keep)?;
                        before_total += b;
                        after_total += a;
                        stats.images += 1;
                        if !blobs.iter().any(|(x, _)| *x == n) {
                            blobs.push((n.clone(), bytes));
                        }
                        used.push((key, n.clone()));
                        n
                    }
                };
                s.insert(field.into(), Value::String(format!("asset:{name}")));
            }
        }
        for s in slides.iter_mut() {
            let Some(s) = s.as_object_mut() else { continue };
            let v = s_get(s, "videoSource", "");
            if v.is_empty() || is_url(&v) {
                continue;
            }
            let src = PathBuf::from(&v);
            if !src.is_file() {
                continue;
            }
            let real = std::fs::canonicalize(&src).unwrap_or(src.clone());
            let key = format!("video|{}", real.display());
            let name = match used.iter().find(|(k, _)| *k == key) {
                Some((_, n)) => n.clone(),
                None => {
                    let keep = extract_assets.as_ref().is_some_and(|a| real.starts_with(a));
                    let fname = real
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let n = if keep {
                        fname
                    } else {
                        let h = images::sha256_hex(real.to_string_lossy().as_bytes());
                        format!("{}-{fname}", &h[..10])
                    };
                    files.push((n.clone(), real.clone()));
                    used.push((key, n.clone()));
                    n
                }
            };
            s.insert("videoSource".into(), Value::String(format!("asset:{name}")));
        }
    }
    stats.saved_bytes = before_total.saturating_sub(after_total);
    let json = py_json_project(&Value::Object(payload));
    let mut tmpname = path.as_os_str().to_owned();
    tmpname.push(".tmp");
    let tmp = PathBuf::from(tmpname);
    let result = (|| -> Result<(), String> {
        let mut w = zip::Writer::create(&tmp)?;
        w.add_deflated("project.json", json.as_bytes())?;
        for (n, b) in &blobs {
            w.add_stored(&format!("assets/{n}"), b)?;
        }
        for (n, p) in &files {
            w.add_file_stored(&format!("assets/{n}"), p)?;
        }
        w.finish()?;
        // Verify the complete package before replacing the last good file.
        let mut r = zip::Reader::open(&tmp)?;
        let mut sink = std::io::sink();
        let mut project = None;
        for i in 0..r.entries.len() {
            if r.entries[i].name == "project.json" {
                project = Some(r.read(i)?);
            } else {
                r.extract(i, &mut sink).map_err(|e| format!("ZIP verification failed: {e}"))?;
            }
        }
        let probe: Value = serde_json::from_slice(&project.ok_or("ZIP verification failed.")?)
            .map_err(|e| e.to_string())?;
        validate_project_data(&probe)?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_json_formatting() {
        let v: Value = serde_json::from_str(r#"{"a": [1, 0.5, "å"], "b": {}}"#).unwrap();
        assert_eq!(py_json(&v), r#"{"a": [1, 0.5, "\u00e5"], "b": {}}"#);
        assert_eq!(py_json_project(&v), "{\n  \"a\": [\n    1,\n    0.5,\n    \"å\"\n  ],\n  \"b\": {}\n}");
    }

    #[test]
    fn unsafe_names_are_rejected() {
        assert!(safe_zip_name("assets/a.png"));
        assert!(!safe_zip_name("../x"));
        assert!(!safe_zip_name("/etc/passwd"));
        assert!(!safe_zip_name("C:/x"));
        assert!(!safe_zip_name("assets/../../x"));
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("nirupres-pkg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("x")).unwrap();
        let mut img = crate::paint::Img::new(8, 8);
        img.data.fill(200);
        std::fs::write(dir.join("pic.png"), images::encode_png(&img).unwrap()).unwrap();
        let mut deck = default_deck();
        normalize_loaded(&mut deck, "t");
        deck["slides"][0]["image"] = Value::String(dir.join("pic.png").to_string_lossy().into());
        let p = dir.join("deck.nirupres");
        save_project(&deck, &p, "OPTIMIZED", None).unwrap();
        let back = load_project(&p, &dir.join("x")).unwrap();
        let img_path = back["slides"][0]["image"].as_str().unwrap().to_string();
        assert!(img_path.starts_with(&dir.join("x").to_string_lossy().to_string()));
        assert!(Path::new(&img_path).is_file());
        assert_eq!(back["slides"][0]["title"], "NIRUPRES");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
