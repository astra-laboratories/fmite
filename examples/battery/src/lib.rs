//! A battery pack of four cells in series, as a Co-Simulation FMU.
//!
//! Every field shows one rule fmite puts in a type:
//!
//! - an input is read-only, so `*self.current = …` does not compile;
//! - a unit is a type: shipped ones (`Ampere`, `Celsius`, `AmpereHour`), and three of
//!   this crate's own, `MilliOhm`, `Percent` and `KelvinPerSecond`, each one impl;
//! - a calculated parameter is written only where fmite hands over a `Calculate`;
//! - a start value is `Default`, once;
//! - an array output gets its `<Dimension>` from its length;
//! - an enumeration is a Rust enum, and an importer cannot set a value it lacks;
//! - a plain field is private state, invisible to the importer.
//!
//! `#[derive(Variables)]` numbers the variables in field order and writes the value
//! reference table; `tests/derive.rs` holds the hand-written impl it replaces.

use fmite::unit::{Ampere, AmpereHour, Celsius, Unit, UnitT, Volt, WattHour};
use fmite::{
    Calculate, CalculatedParameter, CoSimulation, Discrete, Enumeration, Error, Exact, Experiment,
    Fmu, Input, Output, Parameter, Step, StepResult, Tunable, Variables,
};

/// Milliohm: a shipped unit, scaled.
pub struct MilliOhm;

impl UnitT for MilliOhm {
    const NAME: &'static str = "mOhm";
    const UNIT: Unit = Unit::ohm().scaled(1e-3);
}

/// Percent: dimensionless, scaled.
pub struct Percent;

impl UnitT for Percent {
    const NAME: &'static str = "%";
    const UNIT: Unit = Unit::one().scaled(0.01);
}

/// Kelvin per second: two shipped units, composed.
pub struct KelvinPerSecond;

impl UnitT for KelvinPerSecond {
    const NAME: &'static str = "K/s";
    const UNIT: Unit = Unit::kelvin().per(Unit::second());
}

/// What the pack is doing. The discriminants are the item values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Enumeration)]
pub enum Mode {
    #[default]
    Idle = 1,
    Charging = 2,
    Discharging = 3,
}

/// Cells in series.
pub const CELLS: usize = 4;
/// Each cell's resistance relative to the pack parameter: no two cells are alike.
const SPREAD: [f64; CELLS] = [1.0, 1.05, 0.95, 1.1];
/// Open-circuit voltage of an empty cell, and its rise to full.
const EMPTY_VOLTS: f64 = 3.0;
const SPAN_VOLTS: f64 = 1.2;
/// The nominal cell voltages the rated energy is computed at.
const NOMINAL_VOLTS: [f64; CELLS] = [3.6; CELLS];
/// The pack's heat capacity, J/K, and its cooling time constant, s.
const HEAT_CAPACITY: f64 = 2000.0;
const COOLING: f64 = 600.0;
/// Below this, in amperes, the pack is idle.
const IDLE_CURRENT: f64 = 0.1;
/// The model's own step, s.
const DT: f64 = 0.1;

#[derive(Clone, Variables)]
pub struct Battery {
    /// Positive discharges.
    pub current: Input<f64, Ampere>,
    pub ambient: Input<f64, Celsius>,
    pub capacity: Parameter<f64, AmpereHour>,
    pub resistance: Parameter<f64, MilliOhm, Tunable>,
    pub energy: CalculatedParameter<f64, WattHour>,
    /// Exact, so its start value is the importer's to choose.
    pub soc: Output<f64, Percent, Discrete, Exact>,
    pub cells: Output<[f64; CELLS], Volt>,
    pub temperature: Output<f64, Celsius, Discrete>,
    pub heat_rate: Output<f64, KelvinPerSecond, Discrete>,
    pub mode: Output<Mode>,
    pub charges: Output<u32>,
    /// Ampere-hours left: private state.
    charge: f64,
}

impl Default for Battery {
    fn default() -> Self {
        Self {
            current: Input::new(0.0),
            ambient: Input::new(25.0),
            capacity: Parameter::new(50.0),
            resistance: Parameter::new(2.0),
            energy: CalculatedParameter::default(),
            soc: Output::new(100.0),
            cells: Output::default(),
            temperature: Output::default(),
            heat_rate: Output::default(),
            mode: Output::default(),
            charges: Output::default(),
            charge: 0.0,
        }
    }
}

impl Fmu for Battery {
    type Log = ();

    const DESCRIPTION: Option<&'static str> = Some("A four-cell battery pack");
    const DEFAULT_EXPERIMENT: Experiment = Experiment {
        start: Some(0.0),
        stop: Some(600.0),
        step: Some(DT),
        ..Experiment::NONE
    };

    fn calculate(&mut self, calculate: &Calculate) -> Result<(), Error> {
        if *self.capacity <= 0.0 {
            return Err(Error::new("the capacity must be positive"));
        }
        *self.energy.calculate(calculate) = *self.capacity * NOMINAL_VOLTS.iter().sum::<f64>();
        Ok(())
    }

    fn exit_initialization(&mut self) -> Result<(), Error> {
        self.charge = *self.capacity * (*self.soc * Percent::UNIT.factor).clamp(0.0, 1.0);
        *self.temperature = *self.ambient;
        self.update(0.0);
        Ok(())
    }
}

impl Battery {
    /// The outputs at the present charge and current, with the pack heating at
    /// `heat` watts.
    fn update(&mut self, heat: f64) {
        let fill = self.charge / *self.capacity;
        let ohms = *self.resistance * MilliOhm::UNIT.factor;
        let open_circuit = EMPTY_VOLTS + SPAN_VOLTS * fill;
        *self.soc = fill / Percent::UNIT.factor;
        *self.cells = SPREAD.map(|spread| open_circuit - *self.current * ohms * spread);
        *self.heat_rate = heat / HEAT_CAPACITY - (*self.temperature - *self.ambient) / COOLING;
        let mode = match *self.current {
            i if i > IDLE_CURRENT => Mode::Discharging,
            i if i < -IDLE_CURRENT => Mode::Charging,
            _ => Mode::Idle,
        };
        if mode == Mode::Charging && *self.mode != Mode::Charging {
            *self.charges += 1;
        }
        *self.mode = mode;
    }
}

impl CoSimulation for Battery {
    const FIXED_INTERNAL_STEP_SIZE: Option<f64> = Some(DT);

    fn do_step(&mut self, step: Step) -> Result<StepResult, Error> {
        let ohms = *self.resistance * MilliOhm::UNIT.factor;
        let heat = *self.current * *self.current * ohms * SPREAD.iter().sum::<f64>();
        for _ in 0..step.ticks(DT)? {
            let hours = DT / 3600.0;
            self.charge = (self.charge - *self.current * hours).clamp(0.0, *self.capacity);
            *self.temperature += *self.heat_rate * DT;
            self.update(heat);
        }
        Ok(if self.charge <= 0.0 {
            StepResult::Terminate
        } else {
            StepResult::Complete
        })
    }
}

fmite::export!(Battery: CoSimulation + State);
