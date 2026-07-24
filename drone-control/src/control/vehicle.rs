use crate::control::command_unit::{Meters, Telemetry};
use crate::control::command_unit::{SetpointHover, TrajectoryId};
use crate::control::low_level_engine::{Setpoint, Step, StepState};
use crate::control::trajectory::orbit_trajectory::CompressedTrajectory;
use crate::control::trajectory::setpoint_trajectory::Trajectory;
use crate::errors::MissionError::UploadError;
use crate::utils::errors::Res;
use crazyflie_lib::subsystems::high_level_commander::{
    TRAJECTORY_TYPE_POLY4D, TRAJECTORY_TYPE_POLY4D_COMPRESSED,
};
use crazyflie_lib::subsystems::memory::{MemoryType, TrajectoryMemory};
use crazyflie_lib::{Crazyflie, Error};
use std::fmt::{Debug, Formatter};
use std::ops::Add;
use std::time::Duration;
use tokio::sync::{Mutex, watch};
use tokio::time;
use tokio::time::{Instant, sleep};
use tracing::info;

pub struct Vehicle {
    cf: Crazyflie,
    trajectory_state: Mutex<TrajectoryState>,
    pub telemetry: watch::Receiver<Telemetry>,
}
impl Debug for Vehicle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vehicle")
            .field("cf", &"Crazyflie")
            .field("telemetry", &"telemetry_receiver")
            .finish()
    }
}

#[derive(Default)]
struct TrajectoryState {
    current_id: TrajectoryId,
    offset_bytes: usize,
}

impl Vehicle {
    pub fn new(cf: Crazyflie, telemetry: watch::Receiver<Telemetry>) -> Self {
        Self {
            cf,
            telemetry,
            trajectory_state: Mutex::default(),
        }
    }
    pub fn latest_telemetry(&self) -> Telemetry {
        *self.telemetry.borrow()
    }

    pub async fn take_off(&self, height: Meters, duration: Duration) -> Res<()> {
        info!("take off");
        self.cf
            .high_level_commander
            .take_off(height.0, None, duration.as_secs_f32(), None)
            .await?;
        sleep(duration).await;
        Ok(())
    }

    pub async fn go_to(
        &self,
        x: Meters,
        y: Meters,
        z: Meters,
        yaw: f32,
        duration: Duration,
        relative: bool,
        linear: bool,
    ) -> Res<()> {
        info!("go to {x}x {y}y {z}z");
        self.cf
            .high_level_commander
            .go_to(
                x.0,
                y.0,
                z.0,
                yaw,
                duration.as_secs_f32(),
                relative,
                linear,
                None,
            )
            .await?;
        sleep(duration).await;
        Ok(())
    }

    pub async fn land(&self, duration: Duration) -> Res<()> {
        info!("land in place");
        self.cf
            .high_level_commander
            .land(0.0, None, duration.as_secs_f32(), None)
            .await?;
        sleep(duration).await;
        Ok(())
    }

    pub async fn send_setpoint(&self, setpoint: Setpoint) -> Res<()> {
        // info!("sending setpoint {setpoint:?}");
        match setpoint {
            Setpoint::VelocityPoint {
                vx,
                vy,
                vz,
                yaw_rate,
            } => {
                self.cf
                    .commander
                    .setpoint_velocity_world(vx.0, vy.0, vz.0, yaw_rate)
                    .await?;
            }
            Setpoint::PositionPoint {
                x,
                y,
                z,
                yaw_degrees: yaw,
            } => {
                self.cf
                    .commander
                    .setpoint_position(x.0, y.0, z.0, yaw)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn send_relative_speed(
        &self,
        SetpointHover {
            vx,
            vy,
            yaw_rate,
            z,
        }: SetpointHover,
    ) -> Res<()> {
        self.cf
            .commander
            .setpoint_hover(vx.0, vy.0, yaw_rate, z.0)
            .await?;
        Ok(())
    }

    pub async fn notify_setpoint_stop(&self) -> Res<()> {
        info!("setpoint stop - low level commander out.");
        self.cf.commander.notify_setpoint_stop(0).await?;
        Ok(())
    }

    pub async fn emergency_stop(&self) -> Res<()> {
        info!("emergency stop!");
        self.cf.supervisor.send_emergency_stop().await?;
        sleep(Duration::from_secs(1)).await;
        Ok(())
    }

    pub async fn return_home(&self) -> Res<()> {
        info!("returning home!");
        self.notify_setpoint_stop().await?;
        self.go_to(
            Meters(0.0),
            Meters(0.0),
            Meters(0.5),
            0.0,
            Duration::from_secs(2),
            false,
            false,
        )
        .await?;
        self.land(Duration::from_millis(2050)).await?;
        Ok(())
    }

    pub async fn run_steps<S>(
        &self,
        init: S,
        next_step: impl Fn(StepState<S>) -> Step<S>,
    ) -> Res<()> {
        let start_time = Instant::now();

        let mut command_state = init;
        let mut ticks = time::interval(Duration::from_millis(10));

        while let Step::Continue(setpoint, next_cmd_state) = next_step(StepState {
            telemetry: self.latest_telemetry(),
            time_elapsed: start_time.elapsed(),
            command_state,
        }) {
            command_state = next_cmd_state;
            self.send_setpoint(setpoint).await?;
            ticks.tick().await;
        }

        // stop low level commander
        self.notify_setpoint_stop().await?;

        Ok(())
    }

    async fn write_to_mem<F>(&self, write_t: F) -> Res<usize>
    where
        // this AsyncFnOnce ensures the passed in mem outlives the Future returned from F
        // if using FnOnce instead e.g. it would basically need lifetime so we do not drop
        // the mem arg before the future is awaited .await
        F: AsyncFnOnce(&TrajectoryMemory) -> Result<usize, Error>,
    {
        // Open the trajectory memory and upload the segments.
        let memory_device = self
            .cf
            .memory
            .get_memories(Some(MemoryType::Trajectory))
            .pop()
            .cloned()
            .ok_or(UploadError(
                "No trajectory memory device found.".to_string(),
            ))?;

        let trajectory_memory: TrajectoryMemory = self
            .cf
            .memory
            .open_memory(memory_device)
            .await
            .ok_or(UploadError(
                "Trajectory memory already open or not found.".to_string(),
            ))??;

        let bytes_written = write_t(&trajectory_memory).await?;

        self.cf.memory.close_memory(trajectory_memory).await?;
        Ok(bytes_written)
    }

    async fn with_trajectory_reservation<F>(&self, f: F) -> Res<TrajectoryId>
    where
        F: AsyncFn(&TrajectoryState) -> Res<usize>,
    {
        let mut trajectory_mutex = self.trajectory_state.lock().await;
        let current_id = TrajectoryId(trajectory_mutex.current_id.0 + 1);
        let mut new_traj = TrajectoryState {
            current_id,
            offset_bytes: trajectory_mutex.offset_bytes,
        };

        let new_bytes_written = f(&new_traj).await?;
        new_traj.offset_bytes += new_bytes_written;
        *trajectory_mutex = new_traj;

        Ok(current_id)
    }

    pub async fn upload_trajectory(&self, trajectory: &Trajectory) -> Res<TrajectoryId> {
        info!("Uploading trajectory...");
        self.with_trajectory_reservation(
            async |&TrajectoryState {
                       current_id,
                       offset_bytes,
                   }| {
                let bytes_written = self
                    .write_to_mem(async |mem| {
                        mem.write_uncompressed(&trajectory.segments, offset_bytes)
                            .await
                    })
                    .await?;

                // Register the uploaded trajectory under an ID the high-level commander can run.
                info!("Defining trajectory...");
                self.cf
                    .high_level_commander
                    .define_trajectory(
                        current_id.0,
                        offset_bytes as u32,
                        trajectory.segments.len() as u8,
                        Some(TRAJECTORY_TYPE_POLY4D),
                    )
                    .await?;

                Ok(bytes_written)
            },
        )
        .await
    }

    pub async fn upload_compressed_trajectory(
        &self,
        CompressedTrajectory {
            start, segments, ..
        }: &CompressedTrajectory,
    ) -> Res<TrajectoryId> {
        info!("Uploading compressed trajectory...");
        self.with_trajectory_reservation(
            async |&TrajectoryState {
                       current_id,
                       offset_bytes,
                   }| {
                let bytes_written = self
                    .write_to_mem(async |mem| {
                        mem.write_compressed(start, segments, offset_bytes).await
                    })
                    .await?;

                // Register the uploaded trajectory under an ID the high-level commander can run.
                info!("Defining trajectory...");
                self.cf
                    .high_level_commander
                    .define_trajectory(
                        current_id.0,
                        offset_bytes as u32,
                        segments.len() as u8,
                        Some(TRAJECTORY_TYPE_POLY4D_COMPRESSED),
                    )
                    .await?;
                Ok(bytes_written)
            },
        )
        .await
    }

    pub async fn run_trajectory(
        &self,
        trajectory_id: TrajectoryId,
        trajectory_duration: Duration,
    ) -> Res<()> {
        info!("Starting trajectory...");
        self.cf
            .high_level_commander
            .start_trajectory(trajectory_id.0, 1.0, true, false, false, None)
            .await?;
        sleep(trajectory_duration.add(Duration::from_millis(200))).await;
        Ok(())
    }
}
