use arrow::array::{AsArray, RecordBatch};
use arrow::datatypes::Float32Type;
use arrow::ipc::reader::StreamReader;
use datalink::domain_types::{Meters, MetersPerSecond, Telemetry};
use flight_recorder::FlightRecorder;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use proptest::collection::vec;
use proptest::{prop_assert, prop_assert_eq, prop_compose};
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use test_strategy::proptest;

prop_compose! { fn arb_meters()(m in -100.0f32..100.0) -> Meters { Meters(m) } }
prop_compose! { fn arb_m_per_s()(m in -100.0f32..100.0) -> MetersPerSecond { MetersPerSecond(m) } }
prop_compose! {
        fn arb_telemetry()
        (
        x in arb_meters(),
        y in arb_meters(),
        z in arb_meters(),
        x_v in arb_m_per_s(),
        y_v in arb_m_per_s(),
        yaw_degrees in 0f32..360f32,
        range_front in arb_meters(),
        range_back in arb_meters(),
        range_right in arb_meters(),
        range_left in arb_meters(),
        range_up in arb_meters(),
        ) -> Telemetry { Telemetry { x, y, z, x_v, y_v, yaw_degrees, range_front, range_back, range_right, range_left, range_up } }
}
prop_compose! { fn arb_telemetries()(n in 2usize..50)(v in vec(arb_telemetry(),n)) -> Vec<Telemetry> { v } }

#[proptest(async = "tokio")]
async fn record_telemetry(#[strategy(arb_telemetries())] t: Vec<Telemetry>) {
    let entries = t.len();
    let recording_file = NamedTempFile::new()?;
    let mut recorder = FlightRecorder::new(recording_file.path().to_path_buf(), PathBuf::new(), 2);
    let tele_stream = tokio_stream::iter(t);
    recorder.record(tele_stream).await.expect("records");

    let file = File::open(recording_file.path())?;
    let reader = StreamReader::try_new(file, None)?;
    let batches: Vec<_> = reader.collect();
    let num_batches = batches.len();

    let expected_batches = entries.div_ceil(2);
    let written_entries: usize = batches
        .into_iter()
        .filter_map(|b| b.ok())
        .map(|b| b.num_rows())
        .sum();

    // write the correct amount of batches
    prop_assert_eq!(num_batches, expected_batches);
    // write the correct amount of total rows
    prop_assert_eq!(written_entries, entries);
}

#[proptest(async = "tokio")]
async fn persist_telemetry(#[strategy(arb_telemetries())] t: Vec<Telemetry>) {
    let arrow_file = NamedTempFile::new()?;
    let parquet_file = NamedTempFile::new()?;
    let mut recorder = FlightRecorder::new(
        arrow_file.path().to_path_buf(),
        parquet_file.path().to_path_buf(),
        2,
    );
    let tele_stream = tokio_stream::iter(t.clone());
    recorder.record(tele_stream).await.expect("records");
    recorder.persist().expect("persisted");

    let file = File::open(parquet_file.path())?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;

    let batches: Vec<_> = reader.collect();

    let stored_telemetry: Vec<_> = batches
        .into_iter()
        .filter_map(|b| b.ok())
        .flat_map(batch_to_tele)
        .collect();

    // file cleaned up
    prop_assert!(fs::exists(arrow_file.path())? == false);
    prop_assert_eq!(stored_telemetry, t);
}

fn batch_to_tele(b: RecordBatch) -> Vec<Telemetry> {
    let read_f32 = |name| {
        b.column_by_name(name)
            .unwrap()
            .as_primitive::<Float32Type>()
            .values()
            .iter()
            .map(|&x| x)
    };
    let read_meters = |name| read_f32(name).map(Meters);
    let read_meters_s = |name| read_f32(name).map(MetersPerSecond);

    let teles = read_meters("x")
        .zip(read_meters("y"))
        .zip(read_meters("z"))
        .zip(read_meters_s("x_v"))
        .zip(read_meters_s("y_v"))
        .zip(read_f32("yaw_degrees"))
        .zip(read_meters("range_front"))
        .zip(read_meters("range_back"))
        .zip(read_meters("range_right"))
        .zip(read_meters("range_left"))
        .zip(read_meters("range_up"))
        .map(
            |(
                (
                    (
                        (((((((x, y), z), x_v), y_v), yaw_degrees), range_front), range_back),
                        range_right,
                    ),
                    range_left,
                ),
                range_up,
            )| Telemetry {
                x,
                y,
                z,
                x_v,
                y_v,
                yaw_degrees,
                range_front,
                range_back,
                range_right,
                range_left,
                range_up,
            },
        );
    teles.collect::<Vec<_>>()
}
