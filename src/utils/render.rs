use crate::control::command_unit::Telemetry;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Write;


// PURELY AI WRITTEN RENDERING FOR FUN


// 4m x 4m x 2m box, centred on takeoff (origin).
const X_MIN: f64 = -2.0;
const X_MAX: f64 = 2.0;
const Y_MIN: f64 = -2.0;
const Y_MAX: f64 = 2.0;
const Z_MIN: f64 = 0.0;
const Z_MAX: f64 = 2.0;

const MAP_W: usize = 44;
const MAP_H: usize = 20;
const MAX_SPEED: f64 = 1.0; // m/s that maps to "full red"
const TRAIL_LEN: usize = 48;
const GAUGE_W: usize = 30;

thread_local! {
    // (x, y, speed) of recent samples, oldest at front.
    static TRAIL: RefCell<VecDeque<(f64, f64, f64)>> = RefCell::new(VecDeque::new());
}

pub fn render_telemetry(t: &Telemetry) {
    let speed = t.speed();

    TRAIL.with(|tr| {
        let mut tr = tr.borrow_mut();
        tr.push_back((t.x(), t.y(), speed));
        while tr.len() > TRAIL_LEN {
            tr.pop_front();
        }
    });

    // char + optional RGB colour per cell
    let mut grid = vec![vec![(' ', None::<(u8, u8, u8)>); MAP_W]; MAP_H];

    // trail: oldest first so the newest samples overwrite, dimmer with age
    TRAIL.with(|tr| {
        let tr = tr.borrow();
        let n = tr.len().max(1);
        for (i, (x, y, s)) in tr.iter().enumerate() {
            let (row, col) = (to_row(*y), to_col(*x));
            let age = i as f64 / n as f64; // 0 = oldest, ~1 = newest
            let (r, g, b) = speed_color(*s);
            let dim = 0.25 + 0.75 * age;
            let c = (scale(r, dim), scale(g, dim), scale(b, dim));
            let ch = if age > 0.85 { '∙' } else { '·' };
            grid[row][col] = (ch, Some(c));
        }
    });

    // the drone itself: heading arrow, full-strength speed colour
    grid[to_row(t.y())][to_col(t.x())] = (heading(t.yaw()), Some(speed_color(speed)));

    let z_t = norm(t.z(), Z_MIN, Z_MAX);
    let z_fill_from = ((1.0 - z_t) * (MAP_H as f64 - 1.0)).round() as usize;

    let mut out = String::with_capacity(8 * 1024);
    out.push_str("\x1b[2J\x1b[H");
    out.push_str("  \x1b[1mCRAZYFLIE · LIVE\x1b[0m   (4m × 4m × 2m box, top-down)      z\n");

    // top border
    out.push_str("  ┌");
    for _ in 0..MAP_W {
        out.push('─');
    }
    out.push_str("┐  ┌─┐\n");

    for row in 0..MAP_H {
        out.push_str("  │");
        for &(ch, color) in &grid[row] {
            match color {
                Some((r, g, b)) => {
                    out.push_str(&format!("\x1b[38;2;{r};{g};{b}m{ch}\x1b[0m"));
                }
                None => out.push(ch),
            }
        }
        out.push('│');

        // altitude bar (fills from the bottom up to z)
        let bar = if row >= z_fill_from { '█' } else { ' ' };
        out.push_str(&format!("  │\x1b[38;2;90;170;255m{bar}\x1b[0m│"));
        if row == z_fill_from {
            out.push_str(&format!(" {:.2}m", t.z()));
        }
        out.push('\n');
    }

    // bottom border
    out.push_str("  └");
    for _ in 0..MAP_W {
        out.push('─');
    }
    out.push_str("┘  └─┘ 0m\n");

    // speed gauge
    let filled = (norm(speed, 0.0, MAX_SPEED) * GAUGE_W as f64).round() as usize;
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
    out.push_str(&format!("  {speed:.2} m/s\n"));

    // numeric readout
    out.push_str(&format!(
        "  x {:+.2}  y {:+.2}  z {:.2}   yaw {:+.0}°   v = ({:+.2}, {:+.2}, {:+.2})\n",
        t.x(),
        t.y(),
        t.z(),
        t.yaw(),
        t.vx(),
        t.vy(),
        t.vz(),
    ));

    print!("{out}");
    let _ = std::io::stdout().flush();
}

fn norm(v: f64, min: f64, max: f64) -> f64 {
    ((v - min) / (max - min)).clamp(0.0, 1.0)
}

fn to_col(x: f64) -> usize {
    (norm(x, X_MIN, X_MAX) * (MAP_W as f64 - 1.0)).round() as usize
}

fn to_row(y: f64) -> usize {
    // higher y toward the top of the terminal
    ((1.0 - norm(y, Y_MIN, Y_MAX)) * (MAP_H as f64 - 1.0)).round() as usize
}

fn heading(yaw_deg: f64) -> char {
    let arrows = ['→', '↗', '↑', '↖', '←', '↙', '↓', '↘'];
    let idx = (((yaw_deg.rem_euclid(360.0) + 22.5) / 45.0) as usize) % 8;
    arrows[idx]
}

// green (slow) → yellow → red (fast)
fn speed_color(speed: f64) -> (u8, u8, u8) {
    let t = norm(speed, 0.0, MAX_SPEED);
    if t < 0.5 {
        (lerp(0, 255, t / 0.5), 255, 0)
    } else {
        (255, lerp(255, 0, (t - 0.5) / 0.5), 0)
    }
}

fn lerp(a: u8, b: u8, t: f64) -> u8 {
    (a as f64 + (b as f64 - a as f64) * t).round() as u8
}

fn scale(c: u8, factor: f64) -> u8 {
    (c as f64 * factor).round() as u8
}
