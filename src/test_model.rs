//! Small models shared by the unit tests. `Gain` is Co-Simulation: a gain with a
//! tunable parameter, a calculated parameter, a unit, an array output, and an
//! enumeration. `Ticker` is Scheduled Execution: two clocks, a variable clocked by
//! each, and one by neither.

use crate::export::Exported;
use crate::unit::Volt;
use core::time::Duration;

use crate::{
    Activation, Calculated, CalculatedParameter, Clock, CoSimulation, Discrete, Enumeration, Error,
    Experiment, Fmu, Input, Interface, Output, Parameter, Periodic, ScheduledExecution, Step,
    StepResult, Tunable, ValueReference, Values, ValuesMut, Variable,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sign {
    #[default]
    Zero,
    Positive,
}

impl Enumeration for Sign {
    const NAME: &'static str = "Sign";
    const ITEMS: &'static [(&'static str, i64)] = &[("Zero", 1), ("Positive", 2)];

    fn to_i64(self) -> i64 {
        match self {
            Self::Zero => 1,
            Self::Positive => 2,
        }
    }

    fn from_i64(value: i64) -> Option<Self> {
        match value {
            1 => Some(Self::Zero),
            2 => Some(Self::Positive),
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
pub struct Gain {
    pub u: Input<f64, Volt>,
    pub k: Parameter<f64, (), Tunable>,
    pub twice_k: CalculatedParameter<f64, (), Tunable>,
    pub y: Output<[f64; 2], Volt>,
    pub steps: Output<u32>,
    pub sign: Output<Sign>,
}

impl crate::Variables for Gain {
    const MODEL_NAME: &'static str = "gain-test";
    const INSTANTIATION_TOKEN: &'static str = "{gain}";
    const VARIABLES: &'static [Variable] = &[
        Variable::new::<Input<f64, Volt>>("u", 1),
        Variable::new::<Parameter<f64, (), Tunable>>("k", 2),
        Variable::new::<CalculatedParameter<f64, (), Tunable>>("twice_k", 3),
        Variable::new::<Output<[f64; 2], Volt>>("y", 4),
        Variable::new::<Output<u32>>("steps", 5),
        Variable::new::<Output<Sign>>("sign", 6),
    ];

    fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
        match vr.0 {
            1 => self.u.get_into(out),
            2 => self.k.get_into(out),
            3 => self.twice_k.get_into(out),
            4 => self.y.get_into(out),
            5 => self.steps.get_into(out),
            6 => self.sign.get_into(out),
            _ => Err(vr.unknown()),
        }
    }

    fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
        match vr.0 {
            1 => self.u.importer_set(values),
            2 => self.k.importer_set(values),
            _ => Err(vr.unknown()),
        }
    }
}

impl Fmu for Gain {
    const DESCRIPTION: Option<&'static str> = Some("y = 2k·u");
    const DEFAULT_EXPERIMENT: Experiment = Experiment {
        stop: Some(1.0),
        ..Experiment::NONE
    };

    fn exit_initialization(&mut self) -> Result<(), Error> {
        *self.twice_k = 2.0 * *self.k;
        Ok(())
    }
}

impl CoSimulation for Gain {
    const FIXED_INTERNAL_STEP_SIZE: Option<f64> = Some(0.1);

    fn do_step(&mut self, _: Step) -> Result<StepResult, Error> {
        *self.twice_k = 2.0 * *self.k;
        *self.y = [*self.u * *self.twice_k, -*self.u];
        *self.steps += 1;
        *self.sign = if *self.u > 0.0 {
            Sign::Positive
        } else {
            Sign::Zero
        };
        Ok(StepResult::Complete)
    }
}

impl Exported for Gain {
    const INTERFACE: Interface = Interface::CoSimulation;
    const FIXED_INTERNAL_STEP_SIZE: Option<f64> = <Self as CoSimulation>::FIXED_INTERNAL_STEP_SIZE;
    const STATE: bool = true;
}

pub enum Fast {}

impl Periodic for Fast {
    const INTERVAL: Duration = Duration::from_millis(10);
    const PRIORITY: u32 = 0;
}

pub enum Slow {}

impl Periodic for Slow {
    const INTERVAL: Duration = Duration::from_millis(50);
    const PRIORITY: u32 = 1;
}

/// The fast partition adds `u` to what the slow one last held. The slow partition
/// holds `u`.
#[derive(Clone, Default)]
pub struct Ticker {
    pub fast: Clock<Fast>,
    pub slow: Clock<Slow>,
    pub u: Input<f64, (), Discrete, Fast>,
    pub y: Output<f64, (), Discrete, Calculated, Fast>,
    pub held: Output<f64, (), Discrete, Calculated, Slow>,
    pub ticks: Output<u32>,
}

impl crate::Variables for Ticker {
    const MODEL_NAME: &'static str = "ticker-test";
    const INSTANTIATION_TOKEN: &'static str = "{ticker}";
    const VARIABLES: &'static [Variable] = &[
        Variable::new::<Clock<Fast>>("fast", 1),
        Variable::new::<Clock<Slow>>("slow", 2),
        Variable::new::<Input<f64, (), Discrete, Fast>>("u", 3),
        Variable::new::<Output<f64, (), Discrete, Calculated, Fast>>("y", 4),
        Variable::new::<Output<f64, (), Discrete, Calculated, Slow>>("held", 5),
        Variable::new::<Output<u32>>("ticks", 6),
    ];

    fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
        match vr.0 {
            1 => self.fast.get_into(out),
            2 => self.slow.get_into(out),
            3 => self.u.get_into(out),
            4 => self.y.get_into(out),
            5 => self.held.get_into(out),
            6 => self.ticks.get_into(out),
            _ => Err(vr.unknown()),
        }
    }

    fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
        match vr.0 {
            3 => self.u.importer_set(values),
            _ => Err(vr.unknown()),
        }
    }
}

impl Fmu for Ticker {}

impl ScheduledExecution for Ticker {
    fn activate(&mut self, activation: Activation) -> Result<(), Error> {
        if activation.is::<Fast>() {
            *self.y = *self.u + *self.held;
        } else if activation.is::<Slow>() {
            *self.held = *self.u;
        }
        *self.ticks += 1;
        Ok(())
    }
}

impl Exported for Ticker {
    const INTERFACE: Interface = Interface::ScheduledExecution;
    const FIXED_INTERNAL_STEP_SIZE: Option<f64> = None;
    const STATE: bool = false;
}
