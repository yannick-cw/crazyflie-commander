// grid with cells - each cell either free or occupied
// start with 5cm*5cm
// this crate exposes the map in real time in scanning mode
// the TUI renders it -> p(m_i) = 1 => black box
// each t:
// for each cell in grid
// is this cell in current laser beam?
// if yes: l := log odds
// set cell to: l(current value) + l(inv_sensor_model(m_i;x_t,z_t)) - l(m_i)
// works only really with known poses, sensor is 27°
// only take within 1m to be safe -> or at least within one m meaning free space, further away
// keep no knowlege of being[[f32; 120]; 120] free space and then 0.8 occupied or so, prob decreases the further away

use crate::utils::math::get_angle_in_180;
use datalink::domain_types::{Meters, Telemetry};

// cell is 5*5cm => 120*120 cells for 6m2
// values are all in ln
#[derive(Debug, Clone, PartialEq)]
pub struct OccupancyGrid([[Cell; GRID_SIZE]; GRID_SIZE]);
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Cell {
    pub ln_ods: f32,
    pub x: Meters,
    pub y: Meters,
}
// l(x) = log (p(x) / 1-p(x)) is my l(m_i|z_1-t,x_1:t) <= cell occupied given all measures and pos
impl Cell {
    fn p_to_ln(p: f32) -> f32 {
        (p / (1. - p)).ln()
    }

    // TODO make sure never growth above +- 5 to keep adjustable on drift
    fn with_occ(mut self) -> Cell {
        // 0.8 is prob that is added if I assume occupied
        let p_occ = 0.8_f32;
        self.ln_ods += Self::p_to_ln(p_occ);
        self
    }
    fn with_free(mut self) -> Cell {
        // 0.3 is prob that is added if I assume free
        let p_free = 0.3_f32;
        self.ln_ods += Self::p_to_ln(p_free);
        self
    }
    fn with_occluded(mut self) -> Cell {
        // 0.6 is prob that is added if I assume close behind occupied
        let p_occluded = 0.6_f32;
        self.ln_ods += Self::p_to_ln(p_occluded);
        self
    }
}

const GRID_SIZE: usize = 120;
const CELL_SIZE: Meters = Meters(0.05);
impl Default for OccupancyGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl OccupancyGrid {
    pub fn new() -> OccupancyGrid {
        let mut cells = [[Cell::default(); GRID_SIZE]; GRID_SIZE];
        cells.iter_mut().enumerate().for_each(|(y, inner)| {
            inner.iter_mut().enumerate().for_each(|(x, value)| {
                // missing shift to middle of cell...
                // move (0,0) drone pos into middle of grid for ease of use
                let cell_mx =
                    (Meters(x as f32) * CELL_SIZE.0) - Meters(GRID_SIZE as f32 * CELL_SIZE.0 / 2.0);
                let cell_my =
                    (Meters(y as f32) * CELL_SIZE.0) - Meters(GRID_SIZE as f32 * CELL_SIZE.0 / 2.0);

                value.x = cell_mx;
                value.y = cell_my;
            });
        });

        OccupancyGrid(cells)
    }

    pub fn to_array(&self) -> &[[Cell; GRID_SIZE]; GRID_SIZE] {
        &self.0
    }

    fn update_each<F>(&mut self, f: F)
    where
        F: Fn(Cell) -> Cell,
    {
        self.0.iter_mut().for_each(|rows| {
            rows.iter_mut().for_each(|cell| {
                let new_cell = f(*cell);
                *cell = new_cell;
            })
        });
    }
}

pub fn update_grid(
    grid: &mut OccupancyGrid,
    &Telemetry {
        x,
        y,
        z,
        x_v,
        y_v,
        range_front,
        range_right,
        range_back,
        range_left,
        yaw_degrees,
        ..
    }: &Telemetry,
) {
    // identify which cells I see in x dir within 1m
    // based on 27° cone and distance - either assign quadratic smaller free observations
    // or if it hits sth within 1m update linear decreasing prob of hit around that range
    //                No Clue
    //             \              /
    //            --\------------/----<- Further away less likely
    //               \          /        __.-
    //     max h:1m   \Probably/free <-''
    //                 \     .'
    //         `-.      \   /
    //            `-.    \ /
    // Probably Free `-. XXX
    // further less .-' X   X
    // likely     .'     XXX
    //         .-'

    // only update grid from stable-ish positions, high enough, slower than 0.5m/s
    if z < Meters(0.4) || (x_v.0.powi(2) + y_v.0.powi(2)).sqrt() > 0.5 {
        return;
    }

    grid.update_each(|cell| {
        let dx = cell.x - x;
        let dy = cell.y - y;

        let distance = Meters((dx.0.powi(2) + dy.0.powi(2)).sqrt());
        let is_cell_in_distance = distance <= Meters(1.5);

        let update_direction = |c: Cell, obstacle_range: Meters, angle_shift: f32| {
            // first gets angle from drone position to grid cell
            // subtract drone yaw so we have just offset from center of drone yaw
            // that must be less than half the cone at 27°
            let is_cell_in_cone =
                || get_angle_in_180(dx, dy, yaw_degrees + angle_shift).abs() <= 13.5;
            let is_cell_in_view = is_cell_in_distance && is_cell_in_cone();
            let obstacle_in_range = obstacle_range <= Meters(1.0);

            if is_cell_in_view && obstacle_in_range {
                // obstacle in range - update as likely free in front, likely occupied at distance
                // (based on distance) and less likely occupied behind

                if distance < obstacle_range - Meters(0.05) {
                    c.with_free()
                } else if distance < obstacle_range + Meters(0.05) {
                    // TODO maybe add less likely occ if further away
                    c.with_occ()
                } else if distance < obstacle_range + Meters(0.15) {
                    // TODO if doing the above -> this one also based on that
                    c.with_occluded()
                } else {
                    c
                }
                // everything that is in cone and distance, before any eventual obstacle is cleared
                // when there is no obstacle within 1m
            } else if is_cell_in_view && distance < Meters(1.0) {
                // nothing in range we care for - update prob to likely free based on distance
                // TODO add less likely free the further away (e.g. small leg of chair)
                c.with_free()
            } else {
                c
            }
        };

        [
            (range_front, 0.0),
            (range_back, 180.0),
            (range_left, 90.0),
            (range_right, -90.0),
        ]
        .into_iter()
        .fold(cell, |c, (range, angle_shift)| {
            update_direction(c, range, angle_shift)
        })
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    fn t() -> Telemetry {
        Telemetry {
            x: Meters(0.0),
            y: Meters(0.0),
            z: Meters(0.5),
            x_v: Default::default(),
            y_v: Default::default(),
            yaw_degrees: 0.0,
            battery_level: Default::default(),
            range_front: Meters(0.5),
            range_back: Meters(0.8),
            range_right: Meters(32.0),
            range_left: Meters(0.3),
            range_up: Meters(0.0),
        }
    }

    /// `.` untouched, `-` free, `o` mildly occupied (behind), `#` occupied.
    fn render_window(grid: &OccupancyGrid, from: usize, to: usize) -> String {
        (from..=to)
            .rev()
            .map(|y| {
                (from..=to)
                    .map(|x| match grid.0[y][x].ln_ods {
                        l if l == 0.0 => '.',
                        l if l < 0.0 => '-',
                        l if l > 1.0 => '#',
                        _ => 'o',
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn fill_in_occupancies() {
        let mut grid = OccupancyGrid::new();
        update_grid(&mut grid, &t());

        assert_eq!(
            render_window(&grid, 42, 74).lines().collect::<Vec<_>>(),
            vec![
                ".................................",
                ".................................",
                ".................................",
                ".................................",
                ".................................",
                ".................................",
                ".................ooo.............",
                ".................ooo.............",
                ".................###.............",
                ".................###.............",
                "oo................-..............",
                "oo##--............-..............",
                "oo##------........-........##oo..",
                "oo##----------....-....----##oo..",
                "o###-----------------------###o..",
                "oo##----------....-....----##oo..",
                "oo##------........-........##oo..",
                "oo##--............-..............",
                "oo................-..............",
                ".................---.............",
                ".................---.............",
                ".................---.............",
                ".................---.............",
                "................-----............",
                "................-----............",
                "................-----............",
                "................-----............",
                "...............-------...........",
                "...............-------...........",
                "...............-------...........",
                "...............-------...........",
                "..............---------..........",
                "..............---------..........",
            ]
        );
    }
}
