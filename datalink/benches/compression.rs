use datalink::compression_adapter::grid_to_changed_cells;
use datalink::domain_types::{Cell, OccupancyGrid};
use divan::Bencher;
use rand::Rng;

fn main() {
    divan::main();
}

#[divan::bench(
    args = [0.0, 0.5, 1.0],
    sample_count = 1000, // will do 1000 samples
    max_time = 10, // stops if everything takes more than 10s
    sample_size = 100, // each sample will process 100
    items_count = 14400u32 // one iteration processes 14400 cells -> this is just for rendering
)]
fn compression(bencher: Bencher, diff_percentage: f32) {
    let mut rng = rand::rng();
    let [old, mut next] = read_examples();
    next.iter_mut().for_each(|v| {
        v.iter_mut().for_each(|cell| {
            let random_number: f32 = rng.random_range(0.0..1.0);
            if random_number < diff_percentage {
                cell.ln_ods = random_number;
            }
        })
    });
    bencher.bench(|| grid_to_changed_cells(&next, &old))
}

fn read_examples() -> [OccupancyGrid; 2] {
    let examples = include_str!("frames.json");
    let grid: [Vec<Vec<f32>>; 2] = serde_json::from_str(examples).expect("Should be a grid here");
    grid.map(|i| {
        i.iter()
            .map(|value| value.iter().map(|&ln_ods| Cell { ln_ods }).collect())
            .collect()
    })
}
