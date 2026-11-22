//! Editor chrome: the original monochrome Noir look, or the active NIRUX
//! theme (`_NIRUWM_THEME` on the root window, `~/.config/niruwm`, user
//! theme files and `appearance.toml` overrides). Slide themes never follow
//! it: a deck looks the same everywhere.
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Chrome {
    pub bg: u32,
    pub toolbar: u32,
    pub input: u32,
    pub button: u32,
    pub button_hover: u32,
    pub hover: u32,
    pub fg: u32,
    pub muted: u32,
    pub label: u32,
    pub border: u32,
    pub border_strong: u32,
    pub focus: u32,
    pub accent: u32,
    pub on_accent: u32,
    pub disabled: u32,
    pub font_px: f32,
    pub reduced_motion: bool,
    pub active_theme: String,
    pub source: String,
    pub follows: bool,
}

pub fn blend(a: u32, b: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |x: u32, y: u32| {
        let x = (x & 0xff) as f32;
        let y = (y & 0xff) as f32;
        (x + (y - x) * amount).round().clamp(0.0, 255.0) as u32
    };
    (mix(a >> 16, b >> 16) << 16) | (mix(a >> 8, b >> 8) << 8) | mix(a, b)
}

fn lin(c: u32) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn luminance(v: u32) -> f32 {
    0.2126 * lin((v >> 16) & 0xff) + 0.7152 * lin((v >> 8) & 0xff) + 0.0722 * lin(v & 0xff)
}

pub fn contrast_text(bg: u32) -> u32 {
    if luminance(bg) > 0.42 {
        0x090909
    } else {
        0xf6f3ed
    }
}

impl Chrome {
    /// The 2.x Noir chrome.
    pub fn noir() -> Self {
        Self {
            bg: 0x080808,
            toolbar: 0x0d0d0d,
            input: 0x0e0e0e,
            button: 0x121212,
            button_hover: 0x202020,
            hover: 0x191919,
            fg: 0xe8e8e8,
            muted: 0xa8a8a8,
            label: 0x737373,
            border: 0x292929,
            border_strong: 0x303030,
            focus: 0x8c8c8c,
            accent: 0xe8e8e8,
            on_accent: 0x090909,
            disabled: 0x5d5d5d,
            font_px: 13.0,
            reduced_motion: false,
            active_theme: String::new(),
            source: String::new(),
            follows: false,
        }
    }
}

pub fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| crate::prefs::home().join(".config"))
}

pub fn watch_dir() -> PathBuf {
    config_home().join("niruwm")
}

/// NIRUX is present when NIRUWM has a configuration (2.x rule).
pub fn detect() -> bool {
    if std::env::var("NIRUPRES_CHROME")
        .map(|v| v.trim().eq_ignore_ascii_case("noir"))
        .unwrap_or(false)
    {
        return false;
    }
    let d = watch_dir();
    d.join("config.toml").is_file() || d.join("appearance.toml").is_file()
}

fn read_toml(p: &Path) -> Option<toml::Value> {
    std::fs::read_to_string(p).ok()?.parse::<toml::Value>().ok()
}

fn parse_hex(s: &str) -> Option<u32> {
    let mut t = s.trim().trim_start_matches('#').to_lowercase();
    if t.len() == 3 {
        t = t.chars().flat_map(|c| [c, c]).collect();
    }
    (t.len() == 6 && t.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(&t, 16).ok())
        .flatten()
}

/// Built-in NIRU themes: background, foreground, muted, border, focus.
fn builtin(name: &str) -> Option<[u32; 5]> {
    Some(match name {
        "niru-noir" => [0x0f0f10, 0xe5e5e5, 0xa8a8a8, 0x2a2a2d, 0x5a5a60],
        "niru-nord" => [0x2e3440, 0xeceff4, 0x9aa5b1, 0x3b4252, 0x88c0d0],
        "niru-satie" => [0x141414, 0xdedad4, 0x8f8b86, 0x2a2724, 0xb08d7d],
        "niru-carl-larsson" => [0xf3ece0, 0x3b3228, 0x7d6f5f, 0xd8cdbb, 0xa63d2f],
        "niru-forest-vigil" => [0x0b100c, 0xd8ded2, 0x96a292, 0x2b362c, 0x8aa87e],
        "niru-blood-of-urdr" => [0x120808, 0xe8d6cf, 0xb09a92, 0x3a1d1b, 0xc96050],
        "niru-root-and-stone" => [0x17100b, 0xeadfd2, 0xb5a08c, 0x3a2a1e, 0xc19768],
        "niru-blue-horizon" => [0x0b1821, 0xdce6eb, 0x93a6b2, 0x243846, 0x86aec8],
        "niru-the-norns" => [0x111110, 0xece9e4, 0xa6a19b, 0x3b3936, 0xc9c2b9],
        "niru-winter-nights" => [0x1f232b, 0xead9c3, 0xb9aa9a, 0x4e5157, 0xd8c4aa],
        _ => return None,
    })
}

fn display_name(id: &str) -> String {
    let rest = id.strip_prefix("niru-").unwrap_or(id);
    let words: Vec<String> = rest
        .split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect();
    if id.starts_with("niru-") {
        format!("NIRU {}", words.join(" "))
    } else {
        words.join(" ")
    }
}

/// The NIRUX chrome for the theme `root_theme` (from the root window) or
/// the configured one; None when NIRUX is not detected.
pub fn load_nirux(root_theme: Option<&str>) -> Option<Chrome> {
    if !detect() {
        return None;
    }
    let base = watch_dir();
    let config = read_toml(&base.join("config.toml"));
    let appearance = read_toml(&base.join("appearance.toml"));
    let active = root_theme
        .map(str::to_string)
        .or_else(|| {
            config
                .as_ref()
                .and_then(|c| c.get("theme")?.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| "niru-noir".into())
        .trim()
        .to_lowercase();
    let mut c = builtin(&active).unwrap_or(builtin("niru-noir").unwrap_or([0; 5]));
    let mut name = display_name(&active);
    let theme_file = base.join("themes").join(format!("{active}.toml"));
    let keys = ["background", "foreground", "muted", "border", "focus"];
    if let Some(t) = read_toml(&theme_file) {
        if let Some(n) = t.get("name").and_then(|v| v.as_str()) {
            name = n.to_string();
        }
        let tbl = t.get("colors").cloned().unwrap_or(t.clone());
        for (i, k) in keys.iter().enumerate() {
            if let Some(v) = tbl.get(*k).and_then(|v| v.as_str()).and_then(parse_hex) {
                c[i] = v;
            } else if *k == "focus" {
                if let Some(v) = tbl
                    .get("accent")
                    .and_then(|v| v.as_str())
                    .and_then(parse_hex)
                {
                    c[i] = v;
                }
            }
        }
    }
    let overrides = appearance.as_ref().and_then(|a| a.get("colors").cloned());
    if let Some(o) = &overrides {
        for (i, k) in keys.iter().enumerate() {
            if let Some(v) = o.get(*k).and_then(|v| v.as_str()).and_then(parse_hex) {
                c[i] = v;
            }
        }
    }
    let font_px = appearance
        .as_ref()
        .and_then(|a| a.get("font")?.get("size").cloned())
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        .map(|f| f.round() as f32)
        .unwrap_or(14.0)
        .clamp(12.0, 17.0);
    let reduced = appearance
        .as_ref()
        .and_then(|a| a.get("motion")?.get("reduced")?.as_bool())
        .unwrap_or(false);
    let [background, foreground, muted, border, focus] = c;
    let accent = overrides
        .as_ref()
        .and_then(|o| o.get("accent")?.as_str().and_then(parse_hex))
        .unwrap_or(focus);
    Some(Chrome {
        bg: background,
        toolbar: blend(background, foreground, 0.045),
        input: blend(background, foreground, 0.075),
        button: blend(background, foreground, 0.105),
        button_hover: blend(background, foreground, 0.16),
        hover: blend(background, foreground, 0.20),
        fg: foreground,
        muted,
        label: muted,
        border,
        border_strong: blend(border, foreground, 0.20),
        focus,
        accent,
        on_accent: contrast_text(accent),
        disabled: blend(foreground, background, 0.58),
        font_px,
        reduced_motion: reduced,
        active_theme: name,
        source: active,
        follows: true,
    })
}

pub fn status_text(c: Option<&Chrome>, follow: bool) -> String {
    if !follow {
        return "Editor chrome: Noir (NIRUX follow off)".into();
    }
    match c {
        None => "Editor chrome: NIRUX unavailable · Noir fallback".into(),
        Some(c) => format!("Editor chrome: NIRUX · {}", c.active_theme),
    }
}

pub fn read_follow() -> Option<bool> {
    let v = crate::prefs::read_json(&crate::prefs::file("chrome.json"))?;
    v.get("follow_nirux").map(|x| crate::model::truthy(Some(x)))
}

pub fn write_follow(follow: bool) {
    let v = serde_json::json!({ "follow_nirux": follow });
    let path = crate::prefs::file("chrome.json");
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, crate::package::py_json_pretty(&v) + "\n").is_ok() {
        let _ = std::fs::rename(tmp, path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_contrast() {
        assert_eq!(display_name("niru-carl-larsson"), "NIRU Carl Larsson");
        assert_eq!(contrast_text(0xffffff), 0x090909);
        assert_eq!(contrast_text(0x000000), 0xf6f3ed);
        assert_eq!(blend(0x000000, 0xffffff, 0.5), 0x808080);
    }
}
