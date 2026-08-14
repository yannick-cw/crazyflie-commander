// use datalink::domain_types::Cell;
// use datalink::downlink::better_grid::ListOfCells;
// use datalink::downlink::delta_quant_cells;
// use datalink::downlink::{
//     BetterGrid, ChangedCells, DeltaQuantCells, OccupancyGrid, QuantizedCells, QuantizedGrid,
//     quantized_cells, quantized_grid,
// };
// use datalink::downlink::{ZstdCompressedGrid, changed_cells};
// use prost::Message;
// use std::error::Error;
// use std::path::Path;
// use zstd::dict::from_samples;
//
// #[tokio::test]
// async fn reduce_grid_size() -> Result<(), Box<dyn Error>> {
//     let file_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/unoptimised_grid.jsonl");
//     let raw_grid = tokio::fs::read_to_string(file_path).await?;
//     let unoptimised_raw_grid: Result<Vec<Vec<Vec<Cell>>>, _> =
//         serde_json::Deserializer::from_str(&raw_grid)
//             .into_iter::<Vec<Vec<Cell>>>()
//             .collect();
//
//     let unopt_occupancy = unoptimised_raw_grid?.into_iter().map(OccupancyGrid::from);
//     let recording_time_s = unopt_occupancy.len() / 10;
//     let unoptimised_size: usize = unopt_occupancy.clone().map(|g| g.encoded_len()).sum();
//
//     let cells_no_pos = unopt_occupancy.clone().map(|grid| BetterGrid {
//         lists: grid
//             .lists
//             .iter()
//             .map(|inner| ListOfCells {
//                 cell: inner.cell.iter().map(|c| c.ln_ods).collect(),
//             })
//             .collect(),
//     });
//     let cells_no_pos_size: usize = cells_no_pos.clone().map(|g| g.encoded_len()).sum();
//
//     let mut changed_cells_grid = cells_no_pos.clone();
//     let first_full_grid = changed_cells_grid.next().unwrap();
//     let changed_cells = changed_cells_grid
//         .scan(first_full_grid.clone(), |last_grid, next_grid| {
//             // todo expensive diffing here to bench?
//             let mut changed_cells = vec![];
//             for (i, inner) in last_grid.lists.iter().enumerate() {
//                 for (j, old_val) in inner.cell.iter().enumerate() {
//                     let new_val = next_grid.lists[i].cell[j];
//                     if *old_val != new_val {
//                         changed_cells.push(changed_cells::ChangedCell {
//                             ln_ods: new_val,
//                             i: i as i32,
//                             j: j as i32,
//                         })
//                     }
//                 }
//             }
//             *last_grid = next_grid;
//             Some(changed_cells)
//         })
//         .map(|changed_for_one_grid| ChangedCells {
//             cell: changed_for_one_grid,
//         });
//
//     let restored_final_grid =
//         changed_cells
//             .clone()
//             .fold(first_full_grid.clone(), |full_grid, changes| {
//                 changes.cell.iter().fold(full_grid, |mut g, cell| {
//                     g.lists[cell.i as usize].cell[cell.j as usize] = cell.ln_ods;
//                     g
//                 })
//             });
//
//     assert_eq!(cells_no_pos.clone().last().unwrap(), restored_final_grid);
//
//     // last has data
//     let last_full_grid = cells_no_pos.clone().last().unwrap();
//     let last_grid_size = last_full_grid.encoded_len();
//     // adds one keyframe per second with full data
//     let cells_changed_size = (recording_time_s * last_grid_size)
//         + changed_cells
//             .clone()
//             .map(|g| g.encoded_len())
//             .sum::<usize>();
//
//     let last_grid_quantized = QuantizedGrid {
//         lists: last_full_grid
//             .clone()
//             .lists
//             .iter()
//             .map(|cells| quantized_grid::ListOfCells {
//                 cell: cells.cell.iter().map(|c| (c * 10.0) as i32).collect(),
//             })
//             .collect(),
//     };
//
//     // ~todo~ actually changed needs to be calculated after quantisation / drops a bunch of changes
//     // nope - as my steps are too big to be filered out
//     let diffs_quantized = changed_cells.clone().map(|changed| QuantizedCells {
//         cell: changed
//             .cell
//             .iter()
//             .map(|c| quantized_cells::ChangedCell {
//                 i: c.i,
//                 j: c.j,
//                 ln_ods: (c.ln_ods * 10.0) as i32,
//             })
//             .collect(),
//     });
//
//     let last_q_grid_size = last_grid_quantized.encoded_len();
//     // adds one keyframe per second with full data
//     let q_cells_changed_size = (recording_time_s * last_q_grid_size)
//         + diffs_quantized
//             .clone()
//             .map(|g| g.encoded_len())
//             .sum::<usize>();
//
//     let diffs_delta_quant = diffs_quantized.map(|cells| DeltaQuantCells {
//         cell: cells
//             .cell
//             .iter()
//             .scan(0, |last_pos, next_cell| {
//                 let abs_pos = next_cell.i + (next_cell.j + 120);
//                 let delta = abs_pos - *last_pos;
//                 *last_pos = abs_pos;
//
//                 Some(delta_quant_cells::ChangedCell {
//                     ln_ods: next_cell.ln_ods,
//                     delta,
//                 })
//             })
//             .collect(),
//     });
//     // adds one keyframe per second with full data
//     let delta_q_cells_changed_size = (recording_time_s * last_q_grid_size)
//         + diffs_delta_quant
//             .clone()
//             .map(|g| g.encoded_len())
//             .sum::<usize>();
//
//     let zstd_compressed_diffs = diffs_delta_quant.clone().map(|diff| {
//         let vec1 = diff.encode_to_vec();
//         let encoded = zstd::encode_all(vec1.as_slice(), 0).unwrap();
//         ZstdCompressedGrid { inner: encoded }
//     });
//
//     let zstd_compressed_grid = ZstdCompressedGrid {
//         inner: {
//             let bytes = last_grid_quantized.encode_to_vec();
//             zstd::encode_all(bytes.as_slice(), 0)?
//         },
//     };
//     // adds one keyframe per second with full data
//     let zstd_compressed_size = (recording_time_s * zstd_compressed_grid.encoded_len())
//         + zstd_compressed_diffs
//             .clone()
//             .map(|g| g.encoded_len())
//             .sum::<usize>();
//
//     // DICT experiment
//     let all_quantized_grids: Vec<_> = cells_no_pos
//         .clone()
//         .map(|grid| {
//             QuantizedGrid {
//                 lists: grid
//                     .clone()
//                     .lists
//                     .iter()
//                     .map(|cells| quantized_grid::ListOfCells {
//                         cell: cells.cell.iter().map(|c| (c * 10.0) as i32).collect(),
//                     })
//                     .collect(),
//             }
//             .encode_to_vec()
//         })
//         .collect();
//     let dict_full_data = from_samples(all_quantized_grids.as_slice(), 32 * 1024)?;
//
//     let samples: Vec<_> = diffs_delta_quant
//         .clone()
//         .map(|d_cells| d_cells.encode_to_vec())
//         .collect();
//     let dict_diff_data = from_samples(samples.as_slice(), 32 * 1024)?;
//
//     let zstd_dict_diffs = diffs_delta_quant.clone().map(|diff| {
//         let vec1 = diff.encode_to_vec();
//         let encoded = zstd::bulk::Compressor::with_dictionary(0, dict_diff_data.as_slice())
//             .unwrap()
//             .compress(vec1.as_slice())
//             .unwrap();
//
//         ZstdCompressedGrid { inner: encoded }
//     });
//
//     let zstd_dict_grid = ZstdCompressedGrid {
//         inner: {
//             let bytes = last_grid_quantized.encode_to_vec();
//             zstd::bulk::Compressor::with_dictionary(0, dict_full_data.as_slice())?
//                 .compress(bytes.as_slice())?
//         },
//     };
//
//     // adds one keyframe per second with full data
//     let zstd_dict_size = (recording_time_s * zstd_dict_grid.encoded_len())
//         + zstd_dict_diffs
//             .clone()
//             .map(|g| g.encoded_len())
//             .sum::<usize>();
//
//     // delta encoding with snapshot every x seconds
//     // this anyways regardless of the others and the other applied to this additionally
//
//     // uniform scalar quantization, stored as fixed-point
//     // so in terms of protobuf whats the datatype? -> sint32 with range -50..50 <- 1 byte as varint
//     // OR i pack in one byte myself
//     // this applies both to snapshot and delta
//
//     // delta encoding (offset from last instead of ij coordinates)
//     // so what do I save here, instead of two i32 I just send one i32, right?
//     // this applies only to the delta encoding (instead of ij)
//
//     // neighbor difference + some encoding to get rid of 0s (differences of ln value to neighbor value)
//     // will create many 0s, these 0s need to be compressed down - how to do that in terms of protobuf?
//     // can I just NOT send sth if the diff is 0? What are options here with huffman and so on?
//     // -> I run length encode (one byte array of the values and one about run length (e.g. how often is value in other array repeated)
//     // -> bytes field and compress with zstd
//     // this applies to snapshot - does it also apply to changes send? Can I just say diff to last neighbor
//     // -> for changes diff to last value - not as important - but can do as well
//     // on the left basically? in terms of i,j
//
//     // quadtree for full snapshot sending
//     // split into 4 squares <- when all empty (or all same)? send only that quad value once, else recurse
//     // DO AFTER quantization step!
//
//     // Steps
//     // 1. snapshot in delta encoding every 1 second
//     // 2. scalar quantization I pack in one sint32 for snapshot and diff
//     // 3. replace (i,j) with delta encoding
//     // 4.1 store as bytes and apply zstd to keyframe
//     // 4.2 neighbor difference for keyframe with zstd compression and bytes field
//     // 5. quadtree optional as alternative to neighbor difference (they dont play together?)
//     // ---------------------------------------------------------------------------------------------
//
//     // quantized + no absolute pos i j stored in keyframe + keyframe (+ zstd with dict optional) and update sending is the takeaway
//
//     println!(
//         "Average payload size raw: {}KB for {}s of recording",
//         unoptimised_size / 1024,
//         recording_time_s
//     );
//
//     println!(
//         "Average payload size no_cells: {}KB for {}s of recording",
//         cells_no_pos_size / 1024,
//         recording_time_s
//     );
//
//     println!(
//         "Average payload size changes: {}KB for {}s of recording",
//         cells_changed_size / 1024,
//         recording_time_s
//     );
//
//     println!(
//         "Average payload size changes + quantized: {}KB for {}s of recording",
//         q_cells_changed_size / 1024,
//         recording_time_s
//     );
//
//     println!(
//         "Average payload size changes + quantized + diff delta encoding: {}KB for {}s of recording",
//         delta_q_cells_changed_size / 1024,
//         recording_time_s
//     );
//
//     println!(
//         "Average payload size changes + quantized + diff delta + zstd encoding: {}KB for {}s of recording",
//         zstd_compressed_size / 1024,
//         recording_time_s
//     );
//
//     println!(
//         "Average payload size changes + quantized + diff delta + zstd encoding with dict: {}KB for {}s of recording",
//         zstd_dict_size / 1024,
//         recording_time_s
//     );
//
//     // compression to compare: send full grid once and then only values that have changed
//     // drop precision? -> would also lead to potentially less changes
//     // something about float32 more compressed - is already var length though
//
//     Ok(())
// }
