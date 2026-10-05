//! A small model shared by the unit tests: a gain with a tunable parameter, a calculated
//! parameter, a unit, an array output, and an enumeration.

use crate::export::Exported;
use crate::unit::Volt;
use crate::{
    CalculatedParameter, CoSimulation, Enumeration, Error, Experiment, Fmu, Input, Output,
    Parameter, Step, StepResult, Tunable, ValueReference, Values, ValuesMut, Variable,
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
    const STATE: bool = true;
}
