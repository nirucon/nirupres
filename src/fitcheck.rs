//! Template stress render (the 2.3 `template_fit` gate): every layout with
//! short, normal, long and very long text at several sizes. Fails when a
//! line is painted outside its box or the slide, or when short and normal
//! text does not fit. `nirupres --self-test-fit [DIR]` runs it (and writes
//! the 1920x1080 renders to DIR).
use crate::canvas::{Canvas, RasterPainter};
use crate::model::*;
use crate::paint::{Img, RectF};
use crate::render::{self, Ctx};
use crate::textfit;
use serde_json::json;

const LONGWORD: &str = "https://intranet.example.org/socialtjanst/rutiner/handlaggning/dokumentation";
const IMAGE_LAYOUTS: [&str; 7] = [
    "IMAGE",
    "PHOTO",
    "HERO IMAGE",
    "SPLIT",
    "IMAGE + QUOTE",
    "FULL BLEED",
    "IMAGE + CAPTION",
];

fn words(n: usize, seed: usize) -> String {
    let base: Vec<&str> = "presentation slide content needs to remain readable when people add more text than the template designer expected in the first place so we test it"
        .split(' ')
        .collect();
    (0..n)
        .map(|i| base[(i + seed) % base.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn level(lv: &str) -> (String, usize, usize) {
    // (title, words per line, line count)
    match lv {
        "short" => ("Q3".into(), 2, 2),
        "normal" => ("Quarterly results and next steps".into(), 7, 4),
        "long" => (
            "Quarterly results, what we learned and the next steps for the whole organisation".into(),
            16,
            7,
        ),
        _ => (
            format!("Quarterly results, what we learned, what went wrong and the next steps for the whole organisation in 2027 and beyond {LONGWORD}"),
            32,
            12,
        ),
    }
}

fn line(lv: &str, i: usize) -> String {
    let (_, n, _) = level(lv);
    let mut s = words(n, i);
    if lv == "vlong" && i == 0 {
        s = format!("{s} {LONGWORD}");
    }
    s
}

fn body_for(layout: &str, lv: &str) -> String {
    let (_, _, n) = level(lv);
    let join = |v: Vec<String>| v.join("\n");
    match layout {
        "DATA / KPI" | "NUMBER GRID" => {
            let vals = match lv {
                "short" => "7",
                "normal" => "42 %",
                "long" => "1 234 567",
                _ => "1 234 567 890 kr",
            };
            join((0..n).map(|i| format!("{vals} | {}", line(lv, i))).collect())
        }
        "PROCESS" | "TIMELINE" | "MATRIX" => {
            let step = match lv {
                "short" => "Plan",
                "normal" => "Discovery",
                "long" => "Discovery and planning",
                _ => "Discovery, planning and stakeholder alignment",
            };
            join((0..n).map(|i| format!("{step} {} | {}", i + 1, line(lv, i))).collect())
        }
        "TABLE" => {
            let cols = match lv {
                "short" => 2,
                "normal" => 3,
                "long" => 4,
                _ => 6,
            };
            let wn = match lv {
                "short" => 2,
                "normal" => 4,
                _ => 9,
            };
            let mut rows = vec![(0..cols).map(|c| format!("Header {}", c + 1)).collect::<Vec<_>>().join(" | ")];
            for r in 0..n {
                rows.push((0..cols).map(|c| words(wn, r + c)).collect::<Vec<_>>().join(" | "));
            }
            join(rows)
        }
        "TWO COLUMN" | "COMPARE" => {
            let l = join((0..n).map(|i| line(lv, i)).collect());
            let r = join((0..n).map(|i| line(lv, i + 3)).collect());
            format!("{l}\n|||\n{r}")
        }
        "THREE COLUMN" => (0..3)
            .map(|k| join((0..n.saturating_sub(1).max(1)).map(|i| line(lv, i + k)).collect()))
            .collect::<Vec<_>>()
            .join("\n|||\n"),
        "CODE" => {
            let count = match lv {
                "short" => 3,
                "normal" => 12,
                "long" => 24,
                _ => 60,
            };
            let width = match lv {
                "short" => 20,
                "normal" => 50,
                "long" => 80,
                _ => 140,
            };
            join(
                (0..count)
                    .map(|i| {
                        let s = format!(
                            "{}value_{i} = compute('{}'){}",
                            "    ".repeat(i % 3),
                            words(3, i),
                            " # x".repeat(width)
                        );
                        s.chars().take(width + 8).collect()
                    })
                    .collect(),
            )
        }
        "BULLETS" | "AGENDA" => join((0..n).map(|i| line(lv, i)).collect()),
        "TITLE" | "SECTION" | "END" | "STATEMENT" | "QUOTE" => match lv {
            "short" => "Hi".into(),
            "normal" => "A subtitle line for the slide".into(),
            "long" => words(25, 0),
            _ => format!("{} {LONGWORD}", words(70, 0)),
        },
        "BIG NUMBER" => line(lv, 0),
        l if IMAGE_LAYOUTS.contains(&l) || l.starts_with("VIDEO") => {
            join((0..n.min(3)).map(|i| line(lv, i)).collect())
        }
        _ => join((0..n).map(|i| line(lv, i)).collect()),
    }
}

fn title_for(layout: &str, lv: &str) -> String {
    if layout == "BIG NUMBER" {
        return match lv {
            "short" => "7",
            "normal" => "42 %",
            "long" => "1 234 567",
            _ => "1 234 567 890 kr (prognos)",
        }
        .into();
    }
    level(lv).0
}

fn test_image(path: &std::path::Path) {
    let (w, h) = (1600u32, 1000u32);
    let mut img = Img::new(w, h);
    let (a, b) = ([0x33, 0x55, 0x77], [0xaa, 0x77, 0x33]);
    let len2 = (w * w + h * h) as f32;
    for y in 0..h {
        for x in 0..w {
            let t = ((x * w + y * h) as f32 / len2).clamp(0.0, 1.0);
            let o = ((y * w + x) * 4) as usize;
            for k in 0..3 {
                img.data[o + k] = (a[k] as f32 + (b[k] as f32 - a[k] as f32) * t) as u8;
            }
            img.data[o + 3] = 255;
        }
    }
    let _ = std::fs::write(path, crate::images::encode_png(&img).unwrap_or_default());
}

pub fn slide_for(layout: &str, lv: &str, img: &str) -> Obj {
    let mut s = slide_defaults(
        json!({
            "layout": layout,
            "title": title_for(layout, lv),
            "body": body_for(layout, lv),
            "notes": "secret notes",
            "caption": if layout == "IMAGE" || layout == "PHOTO" { level(lv).0 } else { String::new() },
        })
        .as_object(),
    );
    if IMAGE_LAYOUTS.contains(&layout) || layout == "TITLE" {
        s.insert("image".into(), json!(img));
    }
    s
}

/// Run the stress render; returns (failures, notes).
pub fn run(out_dir: Option<&std::path::Path>) -> (Vec<String>, Vec<String>) {
    let tmp = std::env::temp_dir().join(format!("nirupres-fit-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let imgp = tmp.join("test.png");
    test_image(&imgp);
    let img = imgp.to_string_lossy().into_owned();
    let mut failures = Vec::new();
    let mut notes = Vec::new();
    let ctx = Ctx {
        theme: "B&W".into(),
        index: 2,
        total: 10,
        show_numbers: true,
        font: "Noto Sans".into(),
        show_logo: false,
        footer: "Footer text · Organisation".into(),
        footer_align: "LEFT".into(),
        footer_size: "MEDIUM".into(),
    };
    for (w, h) in [(1920, 1080), (1280, 720), (2560, 1440)] {
        for layout in LAYOUTS {
            for lv in ["short", "normal", "long", "vlong"] {
                let s = slide_for(layout, lv, &img);
                let mut cv = Canvas::new(w, h);
                let slide = RectF::new(0.0, 0.0, w as f32, h as f32);
                let (_, report, trace) = textfit::collect(true, || {
                    let mut p = RasterPainter { cv: &mut cv };
                    render::render_slide(&mut p, slide, &s, &ctx);
                });
                let tag = format!("{layout} {lv} {w}x{h}");
                for (bx, ln, txt) in &trace {
                    let inside = ln.x >= bx.x - 1.0
                        && ln.right() <= bx.right() + 1.0
                        && ln.y >= bx.y - 1.0
                        && ln.bottom() <= bx.bottom() + 1.0;
                    let on_slide = bx.x >= -1.0 && bx.y >= -1.0 && bx.right() <= w as f32 + 1.0 && bx.bottom() <= h as f32 + 1.0;
                    if !inside || !on_slide {
                        failures.push(format!("OVERFLOW {tag}: {:?} line={ln:?} box={bx:?}", txt.chars().take(40).collect::<String>()));
                    }
                }
                let bad: Vec<_> = report
                    .iter()
                    .filter(|r| !matches!(r.label.as_str(), "footer" | "label" | "request"))
                    .collect();
                if !bad.is_empty() && (lv == "short" || lv == "normal") {
                    failures.push(format!("NOFIT {tag}: {:?}", bad.iter().map(|r| (&r.label, r.kind, r.count)).collect::<Vec<_>>()));
                } else if !bad.is_empty() {
                    notes.push(format!(
                        "{tag}: {}",
                        bad.iter().map(|r| format!("{} {}", r.kind, r.label)).collect::<Vec<_>>().join(", ")
                    ));
                }
                if let (Some(dir), true) = (out_dir, (w, h) == (1920, 1080)) {
                    let _ = std::fs::create_dir_all(dir);
                    let safe = layout.replace(' ', "_").replace('/', "-").replace('+', "plus");
                    if let Some(png) = crate::export::png_bytes(&cv) {
                        let _ = std::fs::write(dir.join(format!("{safe}__{lv}.png")), png);
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    (failures, notes)
}
