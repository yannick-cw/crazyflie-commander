use crate::domain_types::OccupancyGrid;
use crate::downlink::changed_cells::ChangedCell;
use crate::downlink::{ChangedCells, KeyframeGrid, occupancy_grid};
use crate::{domain_types, downlink};
use tonic::codegen::tokio_stream::{Stream, StreamExt};

struct LastSeenGrid {
    grid: domain_types::OccupancyGrid,
    diffs_since: i32,
}
impl LastSeenGrid {
    fn new(grid: domain_types::OccupancyGrid) -> Self {
        LastSeenGrid {
            grid,
            diffs_since: 0,
        }
    }
}

pub fn compressed_grid_stream<S: Stream<Item = domain_types::OccupancyGrid>>(
    grid_stream: S,
) -> impl Stream<Item = downlink::OccupancyGrid> {
    let mut last_grid: Option<LastSeenGrid> = None;

    grid_stream
        .map(move |next_grid| match &mut last_grid {
            None => {
                last_grid = Some(LastSeenGrid::new(next_grid.clone()));
                occupancy_grid::Msg::Keyframe(KeyframeGrid::from(next_grid))
            }
            Some(LastSeenGrid { diffs_since, grid }) if *diffs_since >= 9 => {
                *grid = next_grid.clone();
                *diffs_since = 0;
                occupancy_grid::Msg::Keyframe(KeyframeGrid::from(next_grid))
            }
            Some(LastSeenGrid {
                grid, diffs_since, ..
            }) => {
                let changed_cells = grid_to_changed_cells(&next_grid, grid);

                *diffs_since += 1;
                *grid = next_grid;

                occupancy_grid::Msg::Changes(ChangedCells {
                    cells: changed_cells,
                })
            }
        })
        .map(|msg| downlink::OccupancyGrid { msg: Some(msg) })
}

pub fn grid_to_changed_cells(next_grid: &OccupancyGrid, grid: &OccupancyGrid) -> Vec<ChangedCell> {
    let zipped_rows = grid.iter().zip(next_grid.clone()).enumerate();
    let changed_cells: Vec<ChangedCell> = zipped_rows
        .flat_map(|(i, (old_cell_row, new_cell_row))| {
            let zipped_cells = old_cell_row.iter().zip(new_cell_row).enumerate();
            zipped_cells.filter_map(move |(j, (old_cell_at_ij, new_cell_at_ij))| {
                (old_cell_at_ij.ln_ods != new_cell_at_ij.ln_ods)
                    .then_some((new_cell_at_ij, i, j).into())
            })
        })
        .collect();
    changed_cells
}

pub fn decompressed_grid_stream<S: Stream<Item = downlink::OccupancyGrid>>(
    grid_stream: S,
) -> impl Stream<Item = domain_types::OccupancyGrid> {
    let mut current_grid: Option<domain_types::OccupancyGrid> = None;
    grid_stream
        .filter_map(|msg| msg.msg)
        .filter_map(move |update| match (update, &current_grid) {
            (occupancy_grid::Msg::Keyframe(keyframe), _) => {
                let occupancy_grid = Some(keyframe.into());
                current_grid = occupancy_grid.clone();
                occupancy_grid
            }
            (occupancy_grid::Msg::Changes(changed_cells), Some(to_update_grid)) => {
                let mut to_update_grid: domain_types::OccupancyGrid = to_update_grid.clone();
                for ChangedCell {
                    quantized_odds,
                    i,
                    j,
                } in changed_cells.cells
                {
                    to_update_grid[i as usize][j as usize] = quantized_odds.into();
                }
                let updated_grid = Some(to_update_grid);
                current_grid = updated_grid.clone();
                updated_grid
            }
            (_, _) => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::collection::vec;
    use proptest::prelude::*;
    use test_strategy::proptest;

    prop_compose! {
        fn arb_grids ()
            (grids in 10usize..30, n in 2usize..10)
            (s in vec(vec(vec((-3..3i32).prop_map(|ln_ods|domain_types::Cell{ln_ods: ln_ods as f32}), n), n), grids)
        ) -> Vec<domain_types::OccupancyGrid> { s }
    }

    #[proptest(async = "tokio")]
    async fn round_trip(#[strategy(arb_grids())] grids: Vec<domain_types::OccupancyGrid>) {
        let round_tripped: Vec<_> =
            decompressed_grid_stream(compressed_grid_stream(tokio_stream::iter(grids.clone())))
                .collect()
                .await;

        let quantized_input: Vec<_> = grids
            .into_iter()
            .map(KeyframeGrid::from)
            .map(domain_types::OccupancyGrid::from)
            .collect();

        prop_assert_eq!(quantized_input, round_tripped)
    }
}
