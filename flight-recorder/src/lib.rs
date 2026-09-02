use arrow::array::{Float32Builder, UInt64Builder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use datalink::domain_types::Telemetry;
use futures::Stream;
use parquet::arrow::ArrowWriter;
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio_stream::StreamExt;

pub struct FlightRecorder {
    arrow_file: PathBuf,
    parquet_file: PathBuf,
    batch_size: usize,
    schema: Arc<Schema>,
}

impl FlightRecorder {
    pub fn new(arrow_file: PathBuf, parquet_file: PathBuf, batch_size: usize) -> Self {
        let telemetry_schema = Arc::new(Schema::new(vec![
            Field::new("elapsed_ms", DataType::UInt64, false),
            Field::new("x", DataType::Float32, false),
            Field::new("y", DataType::Float32, false),
            Field::new("z", DataType::Float32, false),
            Field::new("x_v", DataType::Float32, false),
            Field::new("y_v", DataType::Float32, false),
            Field::new("yaw_degrees", DataType::Float32, false),
            Field::new("range_front", DataType::Float32, false),
            Field::new("range_back", DataType::Float32, false),
            Field::new("range_right", DataType::Float32, false),
            Field::new("range_left", DataType::Float32, false),
            Field::new("range_up", DataType::Float32, false),
        ]));
        Self {
            arrow_file,
            parquet_file,
            batch_size,
            schema: telemetry_schema,
        }
    }
    pub fn persist(self) -> anyhow::Result<()> {
        let arrow_reader = StreamReader::try_new(File::open(&self.arrow_file)?, None)?;
        let mut parquet_writer =
            ArrowWriter::try_new(File::create(&self.parquet_file)?, self.schema.clone(), None)?;

        for batch in arrow_reader {
            parquet_writer.write(&batch?)?;
        }

        parquet_writer.close()?;
        fs::remove_file(&self.arrow_file)?;
        Ok(())
    }
    pub async fn record(&mut self, telemetry: impl Stream<Item = Telemetry>) -> anyhow::Result<()> {
        let start_time = Instant::now();
        let file = File::create(&self.arrow_file)?;
        let mut writer = StreamWriter::try_new(file, &self.schema)?;
        let _res = telemetry
            .map(|t| (t, start_time.elapsed()))
            .chunks_timeout(self.batch_size, Duration::from_secs(1))
            .map(|batch| self.store_batch(batch, &mut writer))
            .collect::<anyhow::Result<Vec<()>>>()
            .await?;
        writer.finish()?;
        Ok(())
    }

    fn store_batch(
        &self,
        batch: Vec<(Telemetry, Duration)>,
        writer: &mut StreamWriter<File>,
    ) -> anyhow::Result<()> {
        let mut elapsed = UInt64Builder::with_capacity(self.batch_size);
        let mut x = Float32Builder::with_capacity(self.batch_size);
        let mut y = Float32Builder::with_capacity(self.batch_size);
        let mut z = Float32Builder::with_capacity(self.batch_size);
        let mut xv = Float32Builder::with_capacity(self.batch_size);
        let mut yv = Float32Builder::with_capacity(self.batch_size);
        let mut yaw = Float32Builder::with_capacity(self.batch_size);
        let mut range_f = Float32Builder::with_capacity(self.batch_size);
        let mut range_b = Float32Builder::with_capacity(self.batch_size);
        let mut range_r = Float32Builder::with_capacity(self.batch_size);
        let mut range_l = Float32Builder::with_capacity(self.batch_size);
        let mut range_u = Float32Builder::with_capacity(self.batch_size);
        for (t, dur) in batch {
            elapsed.append_value(dur.as_millis() as u64);
            x.append_value(t.x.0);
            y.append_value(t.y.0);
            z.append_value(t.z.0);
            xv.append_value(t.x_v.0);
            yv.append_value(t.y_v.0);
            yaw.append_value(t.yaw_degrees);
            range_f.append_value(t.range_front.0);
            range_b.append_value(t.range_back.0);
            range_r.append_value(t.range_right.0);
            range_l.append_value(t.range_left.0);
            range_u.append_value(t.range_up.0);
        }

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(elapsed.finish()),
                Arc::new(x.finish()),
                Arc::new(y.finish()),
                Arc::new(z.finish()),
                Arc::new(xv.finish()),
                Arc::new(yv.finish()),
                Arc::new(yaw.finish()),
                Arc::new(range_f.finish()),
                Arc::new(range_b.finish()),
                Arc::new(range_r.finish()),
                Arc::new(range_l.finish()),
                Arc::new(range_u.finish()),
            ],
        )?;

        writer.write(&batch)?;
        Ok(())
    }
}
