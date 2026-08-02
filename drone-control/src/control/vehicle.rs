use crate::MetersPerSecond;
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
        // safe time to always wait before further action
        sleep(duration.max(Duration::from_secs(5))).await;
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

    pub fn is_too_close(
        &Telemetry {
            range_front,
            range_back,
            range_right,
            range_left,
            range_up,
            ..
        }: &Telemetry,
    ) -> bool {
        let safe_distance = Meters(0.20);
        range_front < safe_distance
            || range_back < safe_distance
            || range_left < safe_distance
            || range_right < safe_distance
            || range_up < safe_distance
    }

    pub fn avoid_obstacle_move(
        target_z: Meters,
        t @ &Telemetry {
            range_front,
            range_back,
            range_right,
            range_left,
            ..
        }: &Telemetry,
    ) -> SetpointHover {
        info!("too-close - sending reverse setpoint... tele: {:?}", t);
        let safe_distance = Meters(0.20);

        let v_change = |d: Meters| MetersPerSecond(1. + d.0 * (-0.8 / 0.3));

        let mut vx = MetersPerSecond(0.);
        if range_front < safe_distance {
            vx -= v_change(range_front);
        }
        if range_back < safe_distance {
            vx += v_change(range_back);
        }

        let mut vy = MetersPerSecond(0.);
        if range_left < safe_distance {
            vy -= v_change(range_left);
        }
        if range_right < safe_distance {
            vy += v_change(range_right);
        }

        let s = SetpointHover {
            vx,
            vy,
            z: target_z,
            yaw_rate: 0.0,
        };
        info!("too-close - sending  setpoint... setpoint {:?}", s);
        s
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
#[cfg(test)]
mod tests {
    use super::*;
    fn t() -> Telemetry {
        Telemetry {
            x: Default::default(),
            y: Default::default(),
            z: Default::default(),
            x_v: Default::default(),
            y_v: Default::default(),
            yaw_degrees: 0.0,
            battery_level: Default::default(),
            range_front: Default::default(),
            range_back: Default::default(),
            range_right: Default::default(),
            range_left: Default::default(),
            range_up: Default::default(),
        }
    }

    #[test]
    fn accelerate_away_max_speed() {
        let too_close_front_right = Telemetry {
            range_back: Meters(1.0),
            range_left: Meters(1.0),
            ..t()
        };

        let z = Meters(0.5);
        let avoidance = Vehicle::avoid_obstacle_move(z, &too_close_front_right);

        assert_eq!(avoidance.vx, MetersPerSecond(-1.0));
        assert_eq!(avoidance.vy, MetersPerSecond(1.0));
    }

    #[test]
    fn accelerate_away_half_speed() {
        let too_close_front_right = Telemetry {
            range_back: Meters(1.0),
            range_left: Meters(1.0),
            range_front: Meters(0.1875),
            range_right: Meters(0.1875),
            ..t()
        };

        let z = Meters(0.5);
        let avoidance = Vehicle::avoid_obstacle_move(z, &too_close_front_right);

        assert_eq!(avoidance.vx, MetersPerSecond(-0.5));
        assert_eq!(avoidance.vy, MetersPerSecond(0.5));
    }

    #[test]
    fn freeze_equidistant() {
        let too_close_front_right = Telemetry {
            range_back: Meters(0.15),
            range_left: Meters(0.15),
            range_front: Meters(0.15),
            range_right: Meters(0.15),
            ..t()
        };

        let z = Meters(0.5);
        let avoidance = Vehicle::avoid_obstacle_move(z, &too_close_front_right);

        assert_eq!(avoidance.vx, MetersPerSecond(0.0));
        assert_eq!(avoidance.vy, MetersPerSecond(0.0));
    }
}
