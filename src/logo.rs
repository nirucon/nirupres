//! The NIRUPRES window icon (the same drawing as assets/nirupres.svg):
//! a dark rounded slide with a rule, a bold N and a short footer bar.

fn inside_poly(px: f32, py: f32, pts: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        let (xi, yi) = pts[i];
        let (xj, yj) = pts[j];
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn in_round_rect(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    if x < x0 || y < y0 || x > x1 || y > y1 {
        return false;
    }
    let cx = x.clamp(x0 + r, x1 - r);
    let cy = y.clamp(y0 + r, y1 - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

/// ARGB icons at 16, 32, 48 and 64 pixels.
pub fn icon() -> Vec<(u32, u32, Vec<u32>)> {
    const N: [(f32, f32); 10] = [
        (17.0, 45.0),
        (17.0, 23.0),
        (22.0, 23.0),
        (38.0, 38.0),
        (38.0, 23.0),
        (47.0, 23.0),
        (47.0, 45.0),
        (42.0, 45.0),
        (26.0, 30.0),
        (26.0, 45.0),
    ];
    [16u32, 32, 48, 64]
        .iter()
        .map(|&s| {
            let k = 64.0 / s as f32;
            let mut px = vec![0u32; (s * s) as usize];
            for y in 0..s {
                for x in 0..s {
                    let mut acc = [0f32; 4];
                    let n = 4;
                    for sy in 0..n {
                        for sx in 0..n {
                            let ux = (x as f32 + (sx as f32 + 0.5) / n as f32) * k;
                            let uy = (y as f32 + (sy as f32 + 0.5) / n as f32) * k;
                            let col: Option<u32> = if !in_round_rect(ux, uy, 5.0, 7.0, 59.0, 57.0, 8.0) {
                                None
                            } else if !in_round_rect(ux, uy, 7.0, 9.0, 57.0, 55.0, 6.0) {
                                Some(0x6f6f6f)
                            } else if inside_poly(ux, uy, &N) {
                                Some(0xf2f2f2)
                            } else if (13.0..51.0).contains(&ux) && (15.0..17.0).contains(&uy) {
                                Some(0xe8e8e8)
                            } else if (17.0..31.0).contains(&ux) && (49.0..51.0).contains(&uy) {
                                Some(0x8f8f8f)
                            } else {
                                Some(0x111111)
                            };
                            if let Some(c) = col {
                                acc[0] += ((c >> 16) & 0xff) as f32;
                                acc[1] += ((c >> 8) & 0xff) as f32;
                                acc[2] += (c & 0xff) as f32;
                                acc[3] += 255.0;
                            }
                        }
                    }
                    let a = acc[3] / (n * n) as f32;
                    if a > 0.0 {
                        let cov = acc[3] / 255.0;
                        let r = (acc[0] / cov) as u32;
                        let g = (acc[1] / cov) as u32;
                        let b = (acc[2] / cov) as u32;
                        px[(y * s + x) as usize] = ((a as u32) << 24) | (r << 16) | (g << 8) | b;
                    }
                }
            }
            (s, s, px)
        })
        .collect()
}
