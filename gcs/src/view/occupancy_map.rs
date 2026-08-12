//! Floor-plan style background wash for the flight map.
//!
//! One grey level per terminal cell: pale where space was observed free, dark where it
//! was observed occupied, untouched where nothing is known yet.
//!
//! Renders *after* the map canvas and only ever sets the background, so the braille
//! route/drone overlay keeps its own characters and colours on top. (The canvas resets
//! the background across its whole area, so this cannot be drawn underneath it.)

use mission_computer::OccupancyGrid;
use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};
use std::ops::RangeInclusive;

// AI GENERATED

/// Log-odds magnitude below which a cell counts as "no real evidence yet" and stays blank.
/// 0.4 is roughly p = 0.6 / 0.4, i.e. about one observation.
const EVIDENCE: f32 = 0.4;
/// Log-odds magnitude treated as fully confident, matching the clamp in `mission_computer`.
const CONFIDENT: f32 = 5.0;

/// Greys assume a *light* terminal background, like a floor plan on paper: pale rooms,
/// dark walls. Each pair ramps from "just one observation" to "fully confident".
/// Swap the pairs around for a dark terminal.
const OCCUPIED_GREY: (f32, f32) = (140.0, 25.0);
const FREE_GREY: (f32, f32) = (238.0, 214.0);

pub struct OccupancyMap<'a> {
    grid: &'a OccupancyGrid,
    /// Half-extents (horizontal, vertical) of the viewport in metres. Must be the same
    /// pair the map canvas was given, or the wash drifts out of register with the drone
    /// and route drawn on top of it.
    bounds: (f64, f64),
}

impl<'a> OccupancyMap<'a> {
    pub fn new(grid: &'a OccupancyGrid, bounds: (f64, f64)) -> Self {
        Self { grid, bounds }
    }
}

impl Widget for OccupancyMap<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let cells = self.grid.to_array();
        if area.is_empty() || cells.len() < 2 {
            return;
        }
        // grid geometry, read off the cells themselves so it stays in step with the grid
        let count = cells.len();
        let origin = f64::from(cells[0][0].x.0);
        let cell_m = f64::from(cells[0][1].x.0) - origin;
        let to_index = |m: f64| ((m - origin) / cell_m).floor();

        let (w, h) = (f64::from(area.width), f64::from(area.height));
        let (half_h, half_v) = self.bounds;

        // Walk terminal cells (not grid cells): the panel is usually finer than the 5cm
        // grid in x and coarser in y, so going the other way leaves columns unpainted.
        for row in 0..area.height {
            // world x (forward) runs up the screen
            let x_hi = half_v - f64::from(row) / h * 2.0 * half_v;
            let x_lo = half_v - f64::from(row + 1) / h * 2.0 * half_v;
            let xs = span(to_index(x_lo), to_index(x_hi), count);

            for col in 0..area.width {
                // world y (left) runs left across the screen
                let y_hi = half_h - f64::from(col) / w * 2.0 * half_h;
                let y_lo = half_h - f64::from(col + 1) / w * 2.0 * half_h;
                let ys = span(to_index(y_lo), to_index(y_hi), count);

                // strongest evidence either way over the covered patch
                let (mut occupied, mut free) = (f32::MIN, f32::MAX);
                for iy in ys.clone() {
                    for ix in xs.clone() {
                        let l = cells[iy][ix].ln_ods;
                        occupied = occupied.max(l);
                        free = free.min(l);
                    }
                }
                if let Some(color) = shade(occupied, free) {
                    buf[(area.left() + col, area.top() + row)].set_bg(color);
                }
            }
        }
    }
}

/// Inclusive grid-index range covering `lo..=hi`, clamped into the grid; empty when the
/// span falls entirely outside it.
fn span(lo: f64, hi: f64, count: usize) -> RangeInclusive<usize> {
    let last = count as f64 - 1.0;
    if hi < 0.0 || lo > last {
        #[expect(clippy::reversed_empty_ranges, reason = "an empty range is the point")]
        return 1..=0;
    }
    (lo.max(0.0) as usize)..=(hi.min(last) as usize)
}

/// Occupied wins over free, so a thin wall sharing a terminal cell with free space is
/// never averaged away.
fn shade(strongest_occupied: f32, strongest_free: f32) -> Option<Color> {
    let grey = |l: f32, (from, to): (f32, f32)| {
        let t = ((l - EVIDENCE) / (CONFIDENT - EVIDENCE)).clamp(0.0, 1.0);
        let v = (from + t * (to - from)) as u8;
        Color::Rgb(v, v, v)
    };
    if strongest_occupied >= EVIDENCE {
        Some(grey(strongest_occupied, OCCUPIED_GREY))
    } else if strongest_free <= -EVIDENCE {
        Some(grey(-strongest_free, FREE_GREY))
    } else {
        None
    }
}
