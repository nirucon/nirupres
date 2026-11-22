//! User data under `$XDG_DATA_HOME/nirupres` (the same files 2.x wrote):
//! recent files, file workflow, display, inspector, presenter and chrome
//! preferences, the organization profile and the autosave file.
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn data_root() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"));
    base.join("nirupres")
}

pub fn file(name: &str) -> PathBuf {
    data_root().join(name)
}

pub fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub fn read_obj(path: &Path) -> Map<String, Value> {
    match read_json(path) {
        Some(Value::Object(o)) => o,
        _ => Map::new(),
    }
}

/// Write through a temporary file and rename (`indent` like Python's indent=2).
pub fn write_json(path: &Path, v: &Value, pretty: bool) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let text = if pretty {
        crate::package::py_json_pretty(v)
    } else {
        crate::package::py_json(v)
    };
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

// ------------------------------------------------------- file workflow

#[derive(Clone, Debug)]
pub struct FilePrefs {
    pub default_folder: String,
    pub remember_last: bool,
    pub last_folder: String,
    pub last_filter: String,
    pub asset_quality: String,
}

pub fn system_documents_dir() -> PathBuf {
    // XDG_DOCUMENTS_DIR from user-dirs.dirs, as QStandardPaths reads it.
    let cfg = crate::chrome::config_home().join("user-dirs.dirs");
    let mut docs = home().join("Documents");
    if let Ok(t) = std::fs::read_to_string(cfg) {
        for line in t.lines() {
            if let Some(v) = line.trim().strip_prefix("XDG_DOCUMENTS_DIR=") {
                let v = v
                    .trim()
                    .trim_matches('"')
                    .replace("$HOME", &home().to_string_lossy());
                docs = PathBuf::from(v);
            }
        }
    }
    if docs.exists() {
        docs
    } else {
        home()
    }
}

impl FilePrefs {
    pub fn load() -> Self {
        let raw = read_obj(&file("file-workflow.json"));
        let s = |k: &str| {
            raw.get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let remember = raw
            .get("rememberLastFolder")
            .map(|v| crate::model::truthy(Some(v)))
            .unwrap_or(true);
        let mut q = s("assetQuality").to_uppercase();
        if !["OPTIMIZED", "HIGH", "ORIGINAL"].contains(&q.as_str()) {
            q = "OPTIMIZED".into();
        }
        let dir = |v: String| {
            if !v.is_empty() && expand(&v).is_dir() {
                v
            } else {
                String::new()
            }
        };
        Self {
            default_folder: dir(s("defaultFolder")),
            remember_last: remember,
            last_folder: dir(s("lastFolder")),
            last_filter: s("lastFilter"),
            asset_quality: q,
        }
    }

    pub fn save(&self) {
        let v = json!({
            "defaultFolder": self.default_folder,
            "rememberLastFolder": self.remember_last,
            "lastFolder": self.last_folder,
            "lastFilter": self.last_filter,
            "assetQuality": self.asset_quality,
        });
        let _ = write_json(&file("file-workflow.json"), &v, true);
    }

    /// `preferred_folder`.
    pub fn preferred(&self, current: Option<&Path>, prefer_current: bool) -> PathBuf {
        if prefer_current {
            if let Some(c) = current {
                let c = std::fs::canonicalize(c).unwrap_or_else(|_| c.to_path_buf());
                if let Some(p) = c.parent().filter(|p| p.is_dir()) {
                    return p.to_path_buf();
                }
            }
        }
        if self.remember_last && !self.last_folder.is_empty() {
            let p = expand(&self.last_folder);
            if p.is_dir() {
                return p;
            }
        }
        if !self.default_folder.is_empty() {
            let p = expand(&self.default_folder);
            if p.is_dir() {
                return p;
            }
        }
        system_documents_dir()
    }

    /// `remember_file_folder`.
    pub fn remember(&mut self, path: &Path, filter: &str) {
        let folder = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        if self.remember_last && folder.is_dir() {
            self.last_folder = std::fs::canonicalize(&folder)
                .unwrap_or(folder)
                .to_string_lossy()
                .into_owned();
        }
        if !filter.is_empty() {
            self.last_filter = filter.to_string();
        }
        self.save();
    }
}

pub fn expand(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        home().join(rest)
    } else if p == "~" {
        home()
    } else {
        PathBuf::from(p)
    }
}

// --------------------------------------------------------------- recents

pub fn recents() -> Vec<PathBuf> {
    match read_json(&file("recent.json")) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str())
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect(),
        _ => Vec::new(),
    }
}

pub fn add_recent(path: &Path) {
    let p = path.to_string_lossy().into_owned();
    let mut items: Vec<String> = match read_json(&file("recent.json")) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    items.retain(|x| *x != p && Path::new(x).exists());
    items.insert(0, p);
    items.truncate(8);
    let _ = std::fs::create_dir_all(data_root());
    let _ = std::fs::write(
        file("recent.json"),
        crate::package::py_json(&Value::Array(
            items.into_iter().map(Value::String).collect(),
        )),
    );
}

// --------------------------------------------------------------- display

pub fn display_prefs() -> Map<String, Value> {
    read_obj(&file("display.json"))
}

pub fn save_display_prefs(changes: &[(&str, Option<&str>)]) {
    let mut p = display_prefs();
    for (k, v) in changes {
        match v {
            Some(v) => {
                p.insert(k.to_string(), Value::String(v.to_string()));
            }
            None => {
                p.remove(*k);
            }
        }
    }
    let _ = std::fs::create_dir_all(data_root());
    let _ = std::fs::write(
        file("display.json"),
        crate::package::py_json(&Value::Object(p)),
    );
}

// ------------------------------------------------------------- inspector

pub const INSPECTOR_SECTIONS: [&str; 5] = ["CONTENT", "IMAGE", "VIDEO", "STYLE", "PRESENTATION"];

pub fn inspector_state() -> Vec<bool> {
    let raw = read_obj(&file("ui.json"));
    let st = raw.get("inspector").and_then(|v| v.as_object());
    INSPECTOR_SECTIONS
        .iter()
        .map(|n| {
            st.and_then(|s| s.get(*n))
                .and_then(|v| v.as_bool())
                .unwrap_or(*n == "CONTENT")
        })
        .collect()
}

pub fn save_inspector_state(open: &[bool]) {
    let path = file("ui.json");
    let mut raw = read_obj(&path);
    let mut m = Map::new();
    for (n, o) in INSPECTOR_SECTIONS.iter().zip(open) {
        m.insert(n.to_string(), Value::Bool(*o));
    }
    raw.insert("inspector".into(), Value::Object(m));
    let _ = write_json(&path, &Value::Object(raw), true);
}

// ------------------------------------------------------------- presenter

pub fn notes_px(default: i64) -> i64 {
    let p = read_obj(&file("presenter.json"));
    let v = crate::model::to_i64(p.get("notesPx"))
        .filter(|v| *v != 0)
        .unwrap_or(default);
    v.clamp(12, 64)
}

pub fn save_notes_px(px: i64) {
    let path = file("presenter.json");
    let mut p = read_obj(&path);
    p.insert("notesPx".into(), json!(px));
    let _ = std::fs::create_dir_all(data_root());
    let _ = std::fs::write(path, crate::package::py_json(&Value::Object(p)));
}

// --------------------------------------------------------------- profile

pub struct Profile {
    pub key: &'static str,
    pub label: &'static str,
    pub show_logo_default: bool,
    pub logo_tooltip: &'static str,
    pub logo_checkbox: &'static str,
    pub logo_row: &'static str,
    pub footer_placeholder: &'static str,
    pub footer_placeholder_short: &'static str,
    pub title_preset: &'static str,
    pub accent_tooltip: &'static str,
    pub starters: &'static [(&'static str, &'static str)],
}

pub const PROFILES: [Profile; 2] = [
    Profile {
        key: "neutral",
        label: "Neutral",
        show_logo_default: false,
        logo_tooltip: "Master switch for the organization logo on themes that carry one · slide AUTO/ON/OFF is in STYLE",
        logo_checkbox: "Show organization logo (Uddevalla themes)",
        logo_row: "Logo",
        footer_placeholder: "Optional footer · e.g. Team or event · 2026-09-24",
        footer_placeholder_short: "Optional · e.g. Team or event · 2026-09-24",
        title_preset: "Title",
        accent_tooltip: "Per-slide accent override for Uddevalla themes",
        starters: &[("Blank", "B&W"), ("Light", "W&B"), ("Gray", "GRAY")],
    },
    Profile {
        key: "uddevalla",
        label: "Uddevalla",
        show_logo_default: true,
        logo_tooltip: "Master switch for the official Uddevalla kommun logo · slide AUTO/ON/OFF is in STYLE",
        logo_checkbox: "Show official Uddevalla logo",
        logo_row: "Uddevalla logo",
        footer_placeholder: "Optional footer · e.g. Socialtjänsten · 2026-09-24",
        footer_placeholder_short: "Optional · e.g. Socialtjänsten · 2026-09-24",
        title_preset: "Uddevalla title",
        accent_tooltip: "Per-slide Uddevalla accent override",
        starters: &[
            ("Blank", "B&W"),
            ("Uddevalla Blue", "UDDEVALLA BLUE"),
            ("Uddevalla Dark", "UDDEVALLA DARK"),
            ("Uddevalla Black", "UDDEVALLA BLACK"),
        ],
    },
];

pub fn profile_info(name: &str) -> &'static Profile {
    PROFILES
        .iter()
        .find(|p| p.key == name)
        .unwrap_or(&PROFILES[0])
}

const LEGACY_SETTINGS: [&str; 5] = [
    "autosave.json",
    "recent.json",
    "display.json",
    "file-workflow.json",
    "chrome.json",
];

fn write_profile(root: &Path, name: &str, reason: &str) -> std::io::Result<()> {
    let v = json!({"profile": name, "reason": reason});
    std::fs::create_dir_all(root)?;
    let path = root.join("profile.json");
    let tmp = root.join("profile.json.tmp");
    std::fs::write(&tmp, crate::package::py_json_pretty(&v) + "\n")?;
    std::fs::rename(tmp, path)
}

/// `resolve_profile`: the active profile, creating profile.json once.
pub fn resolve_profile(root: &Path) -> String {
    let path = root.join("profile.json");
    let mut unreadable = false;
    if path.exists() {
        if let Some(name) = read_json(&path).and_then(|v| {
            v.get("profile")
                .and_then(|p| p.as_str())
                .map(str::to_string)
        }) {
            if PROFILES.iter().any(|p| p.key == name) {
                return name;
            }
        }
        unreadable = true;
    }
    let legacy = LEGACY_SETTINGS.iter().any(|f| root.join(f).exists());
    let name = if legacy { "uddevalla" } else { "neutral" };
    if !unreadable {
        let _ = write_profile(
            root,
            name,
            if legacy {
                "existing settings"
            } else {
                "fresh install"
            },
        );
    }
    name.to_string()
}

pub fn set_profile(root: &Path, name: &str) -> std::io::Result<()> {
    write_profile(root, name, "selected")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_resolution_keeps_legacy_installs_on_uddevalla() {
        let dir = std::env::temp_dir().join(format!("nirupres-prof-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(resolve_profile(&dir), "neutral");
        std::fs::remove_file(dir.join("profile.json")).unwrap();
        std::fs::write(dir.join("recent.json"), "[]").unwrap();
        assert_eq!(resolve_profile(&dir), "uddevalla");
        set_profile(&dir, "neutral").unwrap();
        assert_eq!(resolve_profile(&dir), "neutral");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
