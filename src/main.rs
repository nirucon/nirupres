#![recursion_limit = "256"]
//! NIRUPRES: minimal, keyboard-oriented presentation studio for X11.
mod canvas;
mod chrome;
mod export;
mod fitcheck;
mod fonts;
mod images;
mod markdown;
mod markup;
mod model;
mod package;
mod paint;
mod pdf;
mod pptx;
mod prefs;
mod render;
mod textfit;
mod zip;

use std::path::PathBuf;

const HELP: &str = "Usage: nirupres [FILE] [--present FILE] [--talk FILE [PROFILE]] [--export-pdf FILE [OUTPUT]] [--check-config] [--version]

The graphical editor opens when no command is given. Talk mode is available
from the TALK menu and uses NIRUCLIP profiles screen-mic, screen-camera or
camera-mic.";

fn load_cli(path: &std::path::Path) -> Result<(model::Obj, tempdir::TempDir), String> {
    let dir = tempdir::TempDir::new("nirupres-cli-")?;
    let data = package::load_project(path, dir.path())?;
    Ok((data, dir))
}

mod tempdir {
    use std::path::{Path, PathBuf};
    /// A private temporary folder removed on drop.
    pub struct TempDir(PathBuf);
    impl TempDir {
        pub fn new(prefix: &str) -> Result<Self, String> {
            let base = std::env::temp_dir();
            for _ in 0..20 {
                let mut b = [0u8; 6];
                crate::model::random_bytes(&mut b);
                let name: String = b.iter().map(|x| format!("{x:02x}")).collect();
                let p = base.join(format!("{prefix}{name}"));
                if std::fs::create_dir(&p).is_ok() {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700));
                    return Ok(Self(p));
                }
            }
            Err("could not create a temporary folder".into())
        }
        pub fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().map(String::as_str);
    match first {
        Some("--version") | Some("-V") => {
            println!("{} {}", model::APP, model::VERSION);
        }
        Some("--help") | Some("-h") => {
            println!("NIRUPRES {}\n{HELP}", model::VERSION);
        }
        Some("--check-config") => {
            let c = chrome::load_nirux(None);
            println!("{} {}", model::APP, model::VERSION);
            println!("data: {}", prefs::data_root().display());
            println!("chrome: {}", chrome::status_text(c.as_ref(), c.is_some()));
            println!("format: {}; markdown: {}", model::FORMAT, model::MD_FORMAT);
            println!("configuration: OK");
        }
        Some("--export-pdf") => {
            let Some(src) = args.get(1) else {
                eprintln!("nirupres: --export-pdf needs a project file");
                std::process::exit(2);
            };
            let source = prefs::expand(src);
            if !source.is_file() {
                eprintln!("nirupres: project not found: {}", source.display());
                std::process::exit(2);
            }
            let target = args
                .get(2)
                .map(|t| prefs::expand(t))
                .unwrap_or_else(|| source.with_extension("pdf"));
            let (data, _dir) = match load_cli(&source) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("nirupres: cannot open {}: {e}", source.display());
                    std::process::exit(1);
                }
            };
            match export::export_pdf(&data, &target) {
                Ok(p) => println!("PDF exported: {}", p.display()),
                Err(e) => {
                    eprintln!("nirupres: PDF export failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("--export-pptx") | Some("--export-png") => {
            let (Some(src), Some(out)) = (args.get(1), args.get(2)) else {
                eprintln!("nirupres: {} needs a project file and an output", args[0]);
                std::process::exit(2);
            };
            let (data, _dir) = match load_cli(&prefs::expand(src)) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("nirupres: cannot open {src}: {e}");
                    std::process::exit(1);
                }
            };
            let r = if args[0] == "--export-pptx" {
                export::export_pptx(&data, &PathBuf::from(out)).map(|p| p.display().to_string())
            } else {
                let dir = PathBuf::from(out);
                let _ = std::fs::create_dir_all(&dir);
                export::export_all_png(&data, &dir).map(|n| format!("{n} PNG slides in {}", dir.display()))
            };
            match r {
                Ok(m) => println!("exported: {m}"),
                Err(e) => {
                    eprintln!("nirupres: export failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("--measure") => {
            // Developer aid: --measure FAMILY WEIGHT PX TEXT
            let fam = args.get(1).cloned().unwrap_or_default();
            let w: u16 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(400);
            let px: f32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(16.0);
            let t = args.get(4).cloned().unwrap_or_default();
            let f = textfit::face(&fam, w, false).expect("face");
            println!(
                "{} w={:.3} asc={} desc={} ls={}",
                f.family,
                textfit::text_width(&f, px, &t),
                f.ascent(px),
                f.descent(px),
                f.line_spacing(px)
            );
        }
        Some("--self-test-fit") => {
            let out = args.get(1).map(PathBuf::from);
            let (fails, notes) = fitcheck::run(out.as_deref());
            for f in &fails {
                println!("{f}");
            }
            println!("template fit: {} failure(s), {} long-text note(s)", fails.len(), notes.len());
            std::process::exit(i32::from(!fails.is_empty()));
        }
        Some(o) if o.starts_with("--") && o != "--present" && o != "--talk" => {
            eprintln!("nirupres: unknown option: {o}");
            std::process::exit(2);
        }
        _ => {
            eprintln!("nirupres: the editor is not built yet");
            std::process::exit(1);
        }
    }
}
