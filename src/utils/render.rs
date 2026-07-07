use crate::control::command_unit::Telemetry;
use std::io::Write;

// PURELY AI WRITTEN RENDERING FOR FUN
//
// High-res top-down map using Braille glyphs: each terminal cell packs a
// 2x4 dot matrix (U+2800..U+28FF), giving an 8x-denser canvas than one
// char = one pixel. The drone's path is drawn at dot resolution and stays.

// Map viewport (a zoom window centred on takeoff, not the geofence).
const X_MIN: f32 = -1.5;
const X_MAX: f32 = 1.5;
const Y_MIN: f32 = -1.5;
const Y_MAX: f32 = 1.5;
const Z_MIN: f32 = 0.0;
const Z_MAX: f32 = 2.0;

// Terminal cells; Braille multiplies these by 2 (cols) and 4 (rows) in dots.
const CELLS_W: usize = 60;
const CELLS_H: usize = 30;
const DOT_W: usize = CELLS_W * 2; // 120 dots wide
const DOT_H: usize = CELLS_H * 4; // 120 dots tall (≈ square viewport)

const MAX_SPEED: f32 = 1.0; // m/s that maps to "full red"
const GAUGE_W: usize = 40;

// Braille dot -> bit within a cell, indexed [sub_row 0..4][sub_col 0..2].
const BRAILLE: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

/// Persistent dot canvas of the flown path. Owned by the telemetry task so it
/// survives across frames and is immune to task thread-hopping.
pub struct PathTrace {
    dots: Vec<Vec<bool>>, // DOT_H x DOT_W
    last: Option<(usize, usize)>,
}

impl PathTrace {
    pub fn new() -> Self {
        Self {
            dots: vec![vec![false; DOT_W]; DOT_H],
            last: None,
        }
    }

    // light every dot on the segment from the previous point to the new one
    fn extend_to(&mut self, row: usize, col: usize) {
        match self.last {
            Some((r0, c0)) => {
                for (r, c) in line(r0, c0, row, col) {
                    self.dots[r][c] = true;
                }
            }
            None => self.dots[row][col] = true,
        }
        self.last = Some((row, col));
    }
}

impl Default for PathTrace {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_telemetry(t: &Telemetry, trace: &mut PathTrace) {
    let speed = t.speed();
    let (dot_row, dot_col) = (to_dot_row(t.y()), to_dot_col(t.x()));
    trace.extend_to(dot_row, dot_col);
    let drone_cell = (dot_row / 4, dot_col / 2);

    let z_t = norm(t.z() as f32, Z_MIN, Z_MAX);
    let z_fill_from = ((1.0 - z_t) * (CELLS_H as f32 - 1.0)).round() as usize;

    let mut out = String::with_capacity(32 * 1024);
    out.push_str("\x1b[2J\x1b[H");
    out.push_str("  \x1b[1mCRAZYFLIE · LIVE\x1b[0m   (top-down · 3m viewport · z 0-2m)\r\n");

    out.push_str("  ┌");
    for _ in 0..CELLS_W {
        out.push('─');
    }
    out.push_str("┐  ┌─┐\r\n");

    for cell_row in 0..CELLS_H {
        out.push_str("  │");
        for cell_col in 0..CELLS_W {
            if (cell_row, cell_col) == drone_cell {
                // bright, speed-coloured head marker
                let (r, g, b) = speed_color(speed);
                out.push_str(&format!("\x1b[1;38;2;{r};{g};{b}m⣿\x1b[0m"));
                continue;
            }
            let mut bits = 0u8;
            for sub_row in 0..4 {
                for sub_col in 0..2 {
                    if trace.dots[cell_row * 4 + sub_row][cell_col * 2 + sub_col] {
                        bits |= BRAILLE[sub_row][sub_col];
                    }
                }
            }
            if bits == 0 {
                out.push(' ');
            } else {
                let ch = char::from_u32(0x2800 + bits as u32).unwrap_or(' ');
                out.push_str(&format!("\x1b[38;2;0;200;200m{ch}\x1b[0m"));
            }
        }
        out.push('│');

        let bar = if cell_row >= z_fill_from { '█' } else { ' ' };
        out.push_str(&format!("  │\x1b[38;2;90;170;255m{bar}\x1b[0m│"));
        if cell_row == z_fill_from {
            out.push_str(&format!(" {:.2}m", t.z()));
        }
        out.push_str("\r\n");
    }

    out.push_str("  └");
    for _ in 0..CELLS_W {
        out.push('─');
    }
    out.push_str("┘  └─┘ 0m\r\n");

    let filled = (norm(speed, 0.0, MAX_SPEED) * GAUGE_W as f32).round() as usize;
    let (r, g, b) = speed_color(speed);
    out.push_str("  speed ");
    out.push_str(&format!("\x1b[38;2;{r};{g};{b}m"));
    for _ in 0..filled {
        out.push('█');
    }
    out.push_str("\x1b[0m");
    for _ in filled..GAUGE_W {
        out.push('░');
    }
    out.push_str(&format!("  {speed:.2} m/s\r\n"));

    out.push_str(&format!(
        "  x {:+.2}  y {:+.2}  z {:.2}   yaw {:+.0}°   v = ({:+.2}, {:+.2} )\r\n",
        t.x(),
        t.y(),
        t.z(),
        t.yaw(),
        t.vx(),
        t.vy(),
        // t.vz(),
    ));

    print!("{out}");
    let _ = std::io::stdout().flush();
}

fn norm(v: f32, min: f32, max: f32) -> f32 {
    ((v - min) / (max - min)).clamp(0.0, 1.0)
}

fn to_dot_col(x: f32) -> usize {
    (norm(x as f32, X_MIN, X_MAX) * (DOT_W as f32 - 1.0)).round() as usize
}

fn to_dot_row(y: f32) -> usize {
    ((1.0 - norm(y as f32, Y_MIN, Y_MAX)) * (DOT_H as f32 - 1.0)).round() as usize
}

// Bresenham over dot coordinates.
fn line(r0: usize, c0: usize, r1: usize, c1: usize) -> Vec<(usize, usize)> {
    let (mut r0, mut c0) = (r0 as i32, c0 as i32);
    let (r1, c1) = (r1 as i32, c1 as i32);
    let (dr, dc) = ((r1 - r0).abs(), (c1 - c0).abs());
    let (sr, sc) = (if r0 < r1 { 1 } else { -1 }, if c0 < c1 { 1 } else { -1 });
    let mut err = dc - dr;
    let mut cells = Vec::new();
    loop {
        cells.push((r0 as usize, c0 as usize));
        if r0 == r1 && c0 == c1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dr {
            err -= dr;
            c0 += sc;
        }
        if e2 < dc {
            err += dc;
            r0 += sr;
        }
    }
    cells
}

// green (slow) → yellow → red (fast)
fn speed_color(speed: f32) -> (u8, u8, u8) {
    let t = norm(speed, 0.0, MAX_SPEED);
    if t < 0.5 {
        (lerp(0, 255, t / 0.5), 255, 0)
    } else {
        (255, lerp(255, 0, (t - 0.5) / 0.5), 0)
    }
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}
