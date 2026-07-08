use crate::control::command_unit::{Meters, Telemetry};
use crate::control::low_level_engine::{Setpoint, Step, StepState};
use crate::utils::errors::Res;
use crazyflie_lib::Crazyflie;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;
use tokio::time::{Instant, sleep};

pub struct Autopilot {
    cf: Crazyflie,
    pub telemetry: watch::Receiver<Telemetry>,
}

impl Autopilot {
    pub fn new(cf: Crazyflie, telemetry: watch::Receiver<Telemetry>) -> Self {
        Self { cf, telemetry }
    }

    pub async fn reset_estimator(&self) -> Res<()> {
        // Reset the x,y,z,yaw estimated values before a new flight
        self.cf
            .param
            .set_lossy("kalman.resetEstimation", 1.0)
            .await?;
        sleep(Duration::from_millis(50)).await;
        self.cf
            .param
            .set_lossy("kalman.resetEstimation", 0.0)
            .await?;
        Ok(())
    }

    pub async fn take_off(&self, height: Meters, duration: Duration) -> Res<()> {
        self.cf
            .high_level_commander
            .take_off(height.0, None, duration.as_secs_f32(), None)
            .await?;
        Ok(sleep(duration).await)
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
        Ok(sleep(duration).await)
    }

    pub async fn land(&self, duration: Duration) -> Res<()> {
        self.cf
            .high_level_commander
            .land(0.0, None, duration.as_secs_f32(), None)
            .await?;
        Ok(sleep(duration).await)
    }

    pub async fn send_setpoint(&self, setpoint: Setpoint) -> Res<()> {
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

    pub async fn notify_setpoint_stop(&self) -> Res<()> {
        self.cf.commander.notify_setpoint_stop(0).await?;
        Ok(())
    }

    pub async fn emergency_stop(&self) -> Res<()> {
        self.cf.localization.emergency.send_emergency_stop().await?;
        sleep(Duration::from_secs(1)).await;
        Ok(())
    }

    pub async fn return_home(&self) -> Res<()> {
        self.cf.commander.notify_setpoint_stop(0).await?;
        self.go_to(
            Meters(0.0),
            Meters(0.0),
            Meters(0.5),
            0.0,
            Duration::from_secs(3),
            false,
            false,
        )
        .await?;
        self.land(Duration::from_secs(3)).await?;
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
            telemetry: *(&self.telemetry).borrow(),
            time_elapsed: Instant::now() - start_time,
            command_state,
        }) {
            command_state = next_cmd_state;
            self.send_setpoint(setpoint).await?;
            ticks.tick().await;
        }

        // stop low level commander
        (&self.cf.commander).notify_setpoint_stop(0).await?;

        Ok(())
    }
}
