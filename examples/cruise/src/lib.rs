//! A cruise controller with two rates, as a Scheduled Execution FMU.
//!
//! The importer is the scheduler. Every 100 ms it runs the speed loop, a PI controller
//! that turns the speed error into a torque command. Every 10 ms it runs the actuator
//! loop, which moves the applied torque toward that command at a limited rate. Each
//! loop is a model partition, and each partition's variables tick with its clock.
//!
//! The actuator loop reads the command the speed loop last published. When both are
//! due at once, the importer runs the actuator loop first, because its priority number
//! is lower. So on those ticks it reads the previous command, as a task on a real
//! scheduler would.

use core::time::Duration;

use fmite::{
    Activation, Calculated, Clock, Discrete, Error, Fmu, Input, Output, Parameter, Periodic,
    ScheduledExecution, Tunable, Variables,
};

/// The actuator loop's clock.
pub struct Every10ms;

impl Periodic for Every10ms {
    const INTERVAL: Duration = Duration::from_millis(10);
    const PRIORITY: u32 = 0;
}

/// The speed loop's clock.
pub struct Every100ms;

impl Periodic for Every100ms {
    const INTERVAL: Duration = Duration::from_millis(100);
    const PRIORITY: u32 = 1;
}

#[derive(Clone, Variables)]
pub struct Cruise {
    pub actuator: Clock<Every10ms>,
    pub speed_loop: Clock<Every100ms>,
    /// Metres per second.
    pub setpoint: Input<f64, (), Discrete, Every100ms>,
    pub speed: Input<f64, (), Discrete, Every100ms>,
    /// Newton-metres per metre per second of error. Tunable, and on no clock: the
    /// importer may change it before any activation.
    pub gain: Parameter<f64, (), Tunable>,
    /// Newton-metres per metre of accumulated error.
    pub integral_gain: Parameter<f64>,
    /// Newton-metres per second.
    pub slew: Parameter<f64>,
    pub command: Output<f64, (), Discrete, Calculated, Every100ms>,
    pub torque: Output<f64, (), Discrete, Calculated, Every10ms>,
    /// Private state: the speed loop's accumulated error.
    integral: f64,
}

impl Default for Cruise {
    fn default() -> Self {
        Self {
            actuator: Clock::default(),
            speed_loop: Clock::default(),
            setpoint: Input::new(0.0),
            speed: Input::new(0.0),
            gain: Parameter::new(400.0),
            integral_gain: Parameter::new(40.0),
            slew: Parameter::new(1000.0),
            command: Output::default(),
            torque: Output::default(),
            integral: 0.0,
        }
    }
}

impl Fmu for Cruise {
    const DESCRIPTION: Option<&'static str> = Some("A two-rate cruise controller");
}

impl ScheduledExecution for Cruise {
    fn activate(&mut self, activation: Activation) -> Result<(), Error> {
        if activation.is::<Every100ms>() {
            let dt = Every100ms::INTERVAL.as_secs_f64();
            let error = *self.setpoint - *self.speed;
            self.integral += error * dt;
            *self.command = *self.gain * error + *self.integral_gain * self.integral;
        } else if activation.is::<Every10ms>() {
            let step = *self.slew * Every10ms::INTERVAL.as_secs_f64();
            *self.torque += (*self.command - *self.torque).clamp(-step, step);
        }
        Ok(())
    }
}

fmite::export!(Cruise: ScheduledExecution + State);
