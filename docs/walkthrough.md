# fmite

FMI 3.0 Co-Simulation export for Rust. A model is a struct whose field types say what
each variable is. From that struct fmite writes the 75 `fmi3*` C functions,
`modelDescription.xml` and the `.fmu` archive, and it checks every call the importer
makes before the model sees it.

This page is the crate documentation and a test suite in one. Every code block
compiles and runs under `cargo test`; every block marked `compile_fail` is a rule of
FMI 3.0 that the compiler enforces.

1. [What your model implements](#1-what-your-model-implements)
2. [What gets generated](#2-what-gets-generated)
3. [What the compiler proves](#3-what-the-compiler-proves)
4. [What the instance checks at runtime](#4-what-the-instance-checks-at-runtime)
5. [Compared with attribute-driven exporters](#5-compared-with-attribute-driven-exporters)

## 1. What your model implements

### The struct

One field per variable, typed by its role. A field without a wrapper is private state.

```
use fmite::unit::{Celsius, Kelvin, Watt};
use fmite::{
    CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output, Parameter, Step,
    StepResult, Tunable, ValueReference, Values, ValuesMut, Variable, Variables,
};

/// An FMI `Enumeration`: a Rust enum and the value of each item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Heating,
}

impl Enumeration for Mode {
    const NAME: &'static str = "Mode";
    const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1), ("Heating", 2)];

    fn to_i64(self) -> i64 {
        match self {
            Self::Off => 1,
            Self::Heating => 2,
        }
    }

    fn from_i64(value: i64) -> Option<Self> {
        match value {
            1 => Some(Self::Off),
            2 => Some(Self::Heating),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct Heater {
    pub setpoint: Input<f64, Celsius>,
    pub temperature: Input<f64, Celsius>,
    /// Watts per kelvin of error.
    pub gain: Parameter<f64, (), Tunable>,
    pub limit: Parameter<f64, Watt>,
    /// The error at which the heater saturates: `limit / gain`.
    pub saturation: CalculatedParameter<f64, Kelvin>,
    pub power: Output<f64, Watt>,
    pub mode: Output<Mode>,
    /// A plain field is private state. The importer never sees it.
    elapsed: f64,
}

impl Default for Heater {
    fn default() -> Self {
        Self {
            setpoint: Input::new(20.0),
            temperature: Input::new(20.0),
            gain: Parameter::new(100.0),
            limit: Parameter::new(2000.0),
            saturation: CalculatedParameter::default(),
            power: Output::default(),
            mode: Output::default(),
            elapsed: 0.0,
        }
    }
}
```

Each wrapper is one row of FMI 3.0 Table 22, with the unit, variability and initial
as type parameters. The defaults are the standard's: floats are `Continuous`, every
other type `Discrete`, parameters `Fixed`, and `initial` is the first the row allows.

| Field type                     | `causality`           | `variability`                                            | `initial`                                   | Model code may |
| ------------------------------ | --------------------- | -------------------------------------------------------- | ------------------------------------------- | -------------- |
| `Input<T, U, V>`               | `input`               | `discrete`, `continuous`                                 | `exact`                                     | read           |
| `Parameter<T, U, V>`           | `parameter`           | `fixed`, `tunable`                                       | `exact`                                     | read           |
| `CalculatedParameter<T, U, V>` | `calculatedParameter` | `fixed`, `tunable`                                       | `calculated`                                | read, write    |
| `Output<T, U, V, I>`           | `output`              | `constant`, `discrete`, `continuous`                     | `exact`; or `calculated` unless constant    | read; write unless constant |
| `Local<T, U, V, I>`            | `local`               | `constant`, `fixed`, `tunable`, `discrete`, `continuous` | as `Output`; `calculated` if fixed, tunable | read; write unless constant |

`T` is any FMI type: `f32`, `f64`, `i8` to `u64`, `bool`, an [`Enumeration`], or a
fixed-size array of one, which writes one `<Dimension>` per rank. `U` is a unit type
from [`unit`](mod@unit), or one of your own: a [`unit::UnitT`] impl naming a `const` [`unit::Unit`].

`Default` is the start values: `start="20"` in the model description is the `20.0` in
`Default`, and there is no second place to write it. `Clone` is the FMU state
capability: `fmi3GetFMUState` is a clone.

### `Variables`: the value-reference table

The list the model description declares, and `get` and `set` by value reference.
`Variable::new::<FieldType>` reads the field's row of Table 22, so an illegal type
fails here exactly as it fails on the field. The instance looks every reference up
before calling, so `set` lists only what the importer may set.

```
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{
#     CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output, Parameter, Step,
#     StepResult, Tunable, ValueReference, Values, ValuesMut, Variable, Variables,
# };
#
# /// An FMI `Enumeration`: a Rust enum and the value of each item.
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
# pub enum Mode {
#     #[default]
#     Off,
#     Heating,
# }
#
# impl Enumeration for Mode {
#     const NAME: &'static str = "Mode";
#     const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1), ("Heating", 2)];
#
#     fn to_i64(self) -> i64 {
#         match self {
#             Self::Off => 1,
#             Self::Heating => 2,
#         }
#     }
#
#     fn from_i64(value: i64) -> Option<Self> {
#         match value {
#             1 => Some(Self::Off),
#             2 => Some(Self::Heating),
#             _ => None,
#         }
#     }
# }
#
# #[derive(Clone)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     /// Watts per kelvin of error.
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     /// The error at which the heater saturates: `limit / gain`.
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     /// A plain field is private state. The importer never sees it.
#     elapsed: f64,
# }
#
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0),
#             temperature: Input::new(20.0),
#             gain: Parameter::new(100.0),
#             limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(),
#             power: Output::default(),
#             mode: Output::default(),
#             elapsed: 0.0,
#         }
#     }
# }
impl Variables for Heater {
    const MODEL_NAME: &'static str = "heater";
    const INSTANTIATION_TOKEN: &'static str = "{heater-1}";
    const VARIABLES: &'static [Variable] = &[
        Variable::new::<Input<f64, Celsius>>("setpoint", 1),
        Variable::new::<Input<f64, Celsius>>("temperature", 2),
        Variable::new::<Parameter<f64, (), Tunable>>("gain", 3),
        Variable::new::<Parameter<f64, Watt>>("limit", 4),
        Variable::new::<CalculatedParameter<f64, Kelvin>>("saturation", 5),
        Variable::new::<Output<f64, Watt>>("power", 6),
        Variable::new::<Output<Mode>>("mode", 7),
    ];

    fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
        match vr.0 {
            1 => self.setpoint.get_into(out),
            2 => self.temperature.get_into(out),
            3 => self.gain.get_into(out),
            4 => self.limit.get_into(out),
            5 => self.saturation.get_into(out),
            6 => self.power.get_into(out),
            7 => self.mode.get_into(out),
            _ => Err(vr.unknown()),
        }
    }

    fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
        match vr.0 {
            1 => self.setpoint.importer_set(values),
            2 => self.temperature.importer_set(values),
            3 => self.gain.importer_set(values),
            4 => self.limit.importer_set(values),
            _ => Err(vr.unknown()),
        }
    }
}
```

With the `derive` feature, `#[derive(fmite::Variables)]` writes this impl from the
struct: variables numbered in field order from 1 (`time` is 0), `MODEL_NAME` from the
package name, and an `INSTANTIATION_TOKEN` hashed from the declarations.

### `Fmu` and `CoSimulation`: the hooks

Every `Fmu` hook has a default. A calculated parameter is computed in
`exit_initialization`, once the importer has set the parameters; a tunable one may be
computed again in `do_step`, where a tunable parameter may have changed.

```
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{
#     CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output, Parameter, Step,
#     StepResult, Tunable, ValueReference, Values, ValuesMut, Variable, Variables,
# };
#
# /// An FMI `Enumeration`: a Rust enum and the value of each item.
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
# pub enum Mode {
#     #[default]
#     Off,
#     Heating,
# }
#
# impl Enumeration for Mode {
#     const NAME: &'static str = "Mode";
#     const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1), ("Heating", 2)];
#
#     fn to_i64(self) -> i64 {
#         match self {
#             Self::Off => 1,
#             Self::Heating => 2,
#         }
#     }
#
#     fn from_i64(value: i64) -> Option<Self> {
#         match value {
#             1 => Some(Self::Off),
#             2 => Some(Self::Heating),
#             _ => None,
#         }
#     }
# }
#
# #[derive(Clone)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     /// Watts per kelvin of error.
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     /// The error at which the heater saturates: `limit / gain`.
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     /// A plain field is private state. The importer never sees it.
#     elapsed: f64,
# }
#
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0),
#             temperature: Input::new(20.0),
#             gain: Parameter::new(100.0),
#             limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(),
#             power: Output::default(),
#             mode: Output::default(),
#             elapsed: 0.0,
#         }
#     }
# }
# impl Variables for Heater {
#     const MODEL_NAME: &'static str = "heater";
#     const INSTANTIATION_TOKEN: &'static str = "{heater-1}";
#     const VARIABLES: &'static [Variable] = &[
#         Variable::new::<Input<f64, Celsius>>("setpoint", 1),
#         Variable::new::<Input<f64, Celsius>>("temperature", 2),
#         Variable::new::<Parameter<f64, (), Tunable>>("gain", 3),
#         Variable::new::<Parameter<f64, Watt>>("limit", 4),
#         Variable::new::<CalculatedParameter<f64, Kelvin>>("saturation", 5),
#         Variable::new::<Output<f64, Watt>>("power", 6),
#         Variable::new::<Output<Mode>>("mode", 7),
#     ];
#
#     fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.get_into(out),
#             2 => self.temperature.get_into(out),
#             3 => self.gain.get_into(out),
#             4 => self.limit.get_into(out),
#             5 => self.saturation.get_into(out),
#             6 => self.power.get_into(out),
#             7 => self.mode.get_into(out),
#             _ => Err(vr.unknown()),
#         }
#     }
#
#     fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.importer_set(values),
#             2 => self.temperature.importer_set(values),
#             3 => self.gain.importer_set(values),
#             4 => self.limit.importer_set(values),
#             _ => Err(vr.unknown()),
#         }
#     }
# }
impl Fmu for Heater {
    const DESCRIPTION: Option<&'static str> = Some("A proportional heater");

    fn exit_initialization(&mut self) -> Result<(), Error> {
        if *self.gain <= 0.0 {
            return Err(Error::new("the gain must be positive"));
        }
        *self.saturation = *self.limit / *self.gain;
        Ok(())
    }
}

impl CoSimulation for Heater {
    fn do_step(&mut self, step: Step) -> Result<StepResult, Error> {
        let error = *self.setpoint - *self.temperature;
        *self.power = (*self.gain * error).clamp(0.0, *self.limit);
        *self.mode = if *self.power > 0.0 { Mode::Heating } else { Mode::Off };
        self.elapsed += step.size;
        Ok(StepResult::Complete)
    }
}
```

The hooks, in the order the importer reaches them: `instantiate` (default:
`Self::default()`; `fmi3Reset` calls it again), `enter_initialization(start, stop)`,
`exit_initialization`, `do_step`, `terminate`. Each returns `Result<_, Error>`, and an
`Err` is logged to the importer and answered with `fmi3Error`.

### `export!`: the symbols

Once, in the crate that builds the `cdylib`:

```
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{
#     CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output, Parameter, Step,
#     StepResult, Tunable, ValueReference, Values, ValuesMut, Variable, Variables,
# };
#
# /// An FMI `Enumeration`: a Rust enum and the value of each item.
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
# pub enum Mode {
#     #[default]
#     Off,
#     Heating,
# }
#
# impl Enumeration for Mode {
#     const NAME: &'static str = "Mode";
#     const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1), ("Heating", 2)];
#
#     fn to_i64(self) -> i64 {
#         match self {
#             Self::Off => 1,
#             Self::Heating => 2,
#         }
#     }
#
#     fn from_i64(value: i64) -> Option<Self> {
#         match value {
#             1 => Some(Self::Off),
#             2 => Some(Self::Heating),
#             _ => None,
#         }
#     }
# }
#
# #[derive(Clone)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     /// Watts per kelvin of error.
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     /// The error at which the heater saturates: `limit / gain`.
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     /// A plain field is private state. The importer never sees it.
#     elapsed: f64,
# }
#
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0),
#             temperature: Input::new(20.0),
#             gain: Parameter::new(100.0),
#             limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(),
#             power: Output::default(),
#             mode: Output::default(),
#             elapsed: 0.0,
#         }
#     }
# }
#
# impl Variables for Heater {
#     const MODEL_NAME: &'static str = "heater";
#     const INSTANTIATION_TOKEN: &'static str = "{heater-1}";
#     const VARIABLES: &'static [Variable] = &[
#         Variable::new::<Input<f64, Celsius>>("setpoint", 1),
#         Variable::new::<Input<f64, Celsius>>("temperature", 2),
#         Variable::new::<Parameter<f64, (), Tunable>>("gain", 3),
#         Variable::new::<Parameter<f64, Watt>>("limit", 4),
#         Variable::new::<CalculatedParameter<f64, Kelvin>>("saturation", 5),
#         Variable::new::<Output<f64, Watt>>("power", 6),
#         Variable::new::<Output<Mode>>("mode", 7),
#     ];
#
#     fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.get_into(out),
#             2 => self.temperature.get_into(out),
#             3 => self.gain.get_into(out),
#             4 => self.limit.get_into(out),
#             5 => self.saturation.get_into(out),
#             6 => self.power.get_into(out),
#             7 => self.mode.get_into(out),
#             _ => Err(vr.unknown()),
#         }
#     }
#
#     fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.importer_set(values),
#             2 => self.temperature.importer_set(values),
#             3 => self.gain.importer_set(values),
#             4 => self.limit.importer_set(values),
#             _ => Err(vr.unknown()),
#         }
#     }
# }
#
# impl Fmu for Heater {
#     const DESCRIPTION: Option<&'static str> = Some("A proportional heater");
#
#     fn exit_initialization(&mut self) -> Result<(), Error> {
#         if *self.gain <= 0.0 {
#             return Err(Error::new("the gain must be positive"));
#         }
#         *self.saturation = *self.limit / *self.gain;
#         Ok(())
#     }
# }
#
# impl CoSimulation for Heater {
#     fn do_step(&mut self, step: Step) -> Result<StepResult, Error> {
#         let error = *self.setpoint - *self.temperature;
#         *self.power = (*self.gain * error).clamp(0.0, *self.limit);
#         *self.mode = if *self.power > 0.0 { Mode::Heating } else { Mode::Off };
#         self.elapsed += step.size;
#         Ok(StepResult::Complete)
#     }
# }
fmite::export!(Heater: CoSimulation + State);
```

The list after the colon is checked: `State` needs `Heater: Clone`, and without it the
line does not compile ([proof](#3-what-the-compiler-proves)). Every function of a
capability left out is still exported; it answers `fmi3Error` and logs why.

### `package`: the archive

With the `package` feature, on the host, after `cargo build`:

```no_run
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{
#     CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output, Parameter, Step,
#     StepResult, Tunable, ValueReference, Values, ValuesMut, Variable, Variables,
# };
#
# /// An FMI `Enumeration`: a Rust enum and the value of each item.
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
# pub enum Mode {
#     #[default]
#     Off,
#     Heating,
# }
#
# impl Enumeration for Mode {
#     const NAME: &'static str = "Mode";
#     const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1), ("Heating", 2)];
#
#     fn to_i64(self) -> i64 {
#         match self {
#             Self::Off => 1,
#             Self::Heating => 2,
#         }
#     }
#
#     fn from_i64(value: i64) -> Option<Self> {
#         match value {
#             1 => Some(Self::Off),
#             2 => Some(Self::Heating),
#             _ => None,
#         }
#     }
# }
#
# #[derive(Clone)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     /// Watts per kelvin of error.
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     /// The error at which the heater saturates: `limit / gain`.
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     /// A plain field is private state. The importer never sees it.
#     elapsed: f64,
# }
#
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0),
#             temperature: Input::new(20.0),
#             gain: Parameter::new(100.0),
#             limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(),
#             power: Output::default(),
#             mode: Output::default(),
#             elapsed: 0.0,
#         }
#     }
# }
#
# impl Variables for Heater {
#     const MODEL_NAME: &'static str = "heater";
#     const INSTANTIATION_TOKEN: &'static str = "{heater-1}";
#     const VARIABLES: &'static [Variable] = &[
#         Variable::new::<Input<f64, Celsius>>("setpoint", 1),
#         Variable::new::<Input<f64, Celsius>>("temperature", 2),
#         Variable::new::<Parameter<f64, (), Tunable>>("gain", 3),
#         Variable::new::<Parameter<f64, Watt>>("limit", 4),
#         Variable::new::<CalculatedParameter<f64, Kelvin>>("saturation", 5),
#         Variable::new::<Output<f64, Watt>>("power", 6),
#         Variable::new::<Output<Mode>>("mode", 7),
#     ];
#
#     fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.get_into(out),
#             2 => self.temperature.get_into(out),
#             3 => self.gain.get_into(out),
#             4 => self.limit.get_into(out),
#             5 => self.saturation.get_into(out),
#             6 => self.power.get_into(out),
#             7 => self.mode.get_into(out),
#             _ => Err(vr.unknown()),
#         }
#     }
#
#     fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.importer_set(values),
#             2 => self.temperature.importer_set(values),
#             3 => self.gain.importer_set(values),
#             4 => self.limit.importer_set(values),
#             _ => Err(vr.unknown()),
#         }
#     }
# }
#
# impl Fmu for Heater {
#     const DESCRIPTION: Option<&'static str> = Some("A proportional heater");
#
#     fn exit_initialization(&mut self) -> Result<(), Error> {
#         if *self.gain <= 0.0 {
#             return Err(Error::new("the gain must be positive"));
#         }
#         *self.saturation = *self.limit / *self.gain;
#         Ok(())
#     }
# }
#
# impl CoSimulation for Heater {
#     fn do_step(&mut self, step: Step) -> Result<StepResult, Error> {
#         let error = *self.setpoint - *self.temperature;
#         *self.power = (*self.gain * error).clamp(0.0, *self.limit);
#         *self.mode = if *self.power > 0.0 { Mode::Heating } else { Mode::Off };
#         self.elapsed += step.size;
#         Ok(StepResult::Complete)
#     }
# }
# fmite::export!(Heater: CoSimulation + State);
# #[cfg(feature = "package")]
# fn main() -> Result<(), fmite::Error> {
use std::path::Path;
use fmite::package::{Binary, package};

let library = Path::new("target/release/libheater.dylib");
package::<Heater>(&[Binary::host(library)?], Path::new("heater.fmu"))?;
# Ok(())
# }
# #[cfg(not(feature = "package"))]
# fn main() {}
```

`package` links the model as an rlib, writes the description from the type, and zips
it with each binary under `binaries/<platform>/`. `Binary::host` names the platform
this runs on; a cross-compiled library is a `Binary { platform, path }`.

## 2. What gets generated

### The symbols

`export!` makes all 75 functions of `fmi3Functions.h`, each `extern "C"`, each inside
`catch_unwind`, each checking its pointers before making a slice. A C test in the
repository assigns every one to the header's function-pointer type, so a missing or
misspelled symbol fails to link.

| Group                                                                                              | Count | Answer                              |
| -------------------------------------------------------------------------------------------------- | ----- | ----------------------------------- |
| `GetVersion`, `SetDebugLogging`, `InstantiateCoSimulation`, `FreeInstance`                         | 4     | implemented                         |
| `EnterInitializationMode`, `ExitInitializationMode`, `Terminate`, `Reset`, `DoStep`                | 5     | implemented                         |
| `Get`/`Set` for `Float32` to `UInt64` and `Boolean`                                                | 22    | implemented                         |
| `GetFMUState`, `SetFMUState`, `FreeFMUState`                                                       | 3     | implemented with `State`; else refused |
| `InstantiateModelExchange`, `InstantiateScheduledExecution`                                        | 2     | refused: null instance, logged      |
| `String` and `Binary` get/set, state serialization, dependencies, derivatives, clocks, event mode, Model Exchange | 39 | refused: `fmi3Error`, logged |

A refused function never panics and never calls `todo!()`. An importer that ignores
the model description's capability flags gets an error with a message, not a crash.

### The model description

Nothing in it is written by hand. This is the heater's, checked by this page's tests:

```
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{
#     CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output, Parameter, Step,
#     StepResult, Tunable, ValueReference, Values, ValuesMut, Variable, Variables,
# };
#
# /// An FMI `Enumeration`: a Rust enum and the value of each item.
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
# pub enum Mode {
#     #[default]
#     Off,
#     Heating,
# }
#
# impl Enumeration for Mode {
#     const NAME: &'static str = "Mode";
#     const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1), ("Heating", 2)];
#
#     fn to_i64(self) -> i64 {
#         match self {
#             Self::Off => 1,
#             Self::Heating => 2,
#         }
#     }
#
#     fn from_i64(value: i64) -> Option<Self> {
#         match value {
#             1 => Some(Self::Off),
#             2 => Some(Self::Heating),
#             _ => None,
#         }
#     }
# }
#
# #[derive(Clone)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     /// Watts per kelvin of error.
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     /// The error at which the heater saturates: `limit / gain`.
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     /// A plain field is private state. The importer never sees it.
#     elapsed: f64,
# }
#
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0),
#             temperature: Input::new(20.0),
#             gain: Parameter::new(100.0),
#             limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(),
#             power: Output::default(),
#             mode: Output::default(),
#             elapsed: 0.0,
#         }
#     }
# }
#
# impl Variables for Heater {
#     const MODEL_NAME: &'static str = "heater";
#     const INSTANTIATION_TOKEN: &'static str = "{heater-1}";
#     const VARIABLES: &'static [Variable] = &[
#         Variable::new::<Input<f64, Celsius>>("setpoint", 1),
#         Variable::new::<Input<f64, Celsius>>("temperature", 2),
#         Variable::new::<Parameter<f64, (), Tunable>>("gain", 3),
#         Variable::new::<Parameter<f64, Watt>>("limit", 4),
#         Variable::new::<CalculatedParameter<f64, Kelvin>>("saturation", 5),
#         Variable::new::<Output<f64, Watt>>("power", 6),
#         Variable::new::<Output<Mode>>("mode", 7),
#     ];
#
#     fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.get_into(out),
#             2 => self.temperature.get_into(out),
#             3 => self.gain.get_into(out),
#             4 => self.limit.get_into(out),
#             5 => self.saturation.get_into(out),
#             6 => self.power.get_into(out),
#             7 => self.mode.get_into(out),
#             _ => Err(vr.unknown()),
#         }
#     }
#
#     fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.importer_set(values),
#             2 => self.temperature.importer_set(values),
#             3 => self.gain.importer_set(values),
#             4 => self.limit.importer_set(values),
#             _ => Err(vr.unknown()),
#         }
#     }
# }
#
# impl Fmu for Heater {
#     const DESCRIPTION: Option<&'static str> = Some("A proportional heater");
#
#     fn exit_initialization(&mut self) -> Result<(), Error> {
#         if *self.gain <= 0.0 {
#             return Err(Error::new("the gain must be positive"));
#         }
#         *self.saturation = *self.limit / *self.gain;
#         Ok(())
#     }
# }
#
# impl CoSimulation for Heater {
#     fn do_step(&mut self, step: Step) -> Result<StepResult, Error> {
#         let error = *self.setpoint - *self.temperature;
#         *self.power = (*self.gain * error).clamp(0.0, *self.limit);
#         *self.mode = if *self.power > 0.0 { Mode::Heating } else { Mode::Off };
#         self.elapsed += step.size;
#         Ok(StepResult::Complete)
#     }
# }
# fmite::export!(Heater: CoSimulation + State);
# let xml = fmite::description::model_description::<Heater>().unwrap();
# assert_eq!(format!("\n{xml}"), concat!(r#"
<?xml version="1.0" encoding="UTF-8"?>
<fmiModelDescription fmiVersion="3.0" modelName="heater" instantiationToken="{heater-1}" description="A proportional heater" generationTool="fmite 0.1.0" variableNamingConvention="structured">
  <CoSimulation modelIdentifier="heater" canGetAndSetFMUState="true" canHandleVariableCommunicationStepSize="true"/>
  <UnitDefinitions>
    <Unit name="degC">
      <BaseUnit K="1" offset="273.15"/>
    </Unit>
    <Unit name="W">
      <BaseUnit kg="1" m="2" s="-3"/>
    </Unit>
    <Unit name="K">
      <BaseUnit K="1"/>
    </Unit>
  </UnitDefinitions>
  <TypeDefinitions>
    <EnumerationType name="Mode">
      <Item name="Off" value="1"/>
      <Item name="Heating" value="2"/>
    </EnumerationType>
  </TypeDefinitions>
  <LogCategories>
    <Category name="logStatusError" description="A refused call, and why"/>
    <Category name="logStatusFatal" description="A panic inside the FMU; the instance takes no more calls"/>
  </LogCategories>
  <ModelVariables>
    <Float64 name="time" valueReference="0" causality="independent" variability="continuous"/>
    <Float64 name="setpoint" valueReference="1" causality="input" variability="continuous" unit="degC" start="20"/>
    <Float64 name="temperature" valueReference="2" causality="input" variability="continuous" unit="degC" start="20"/>
    <Float64 name="gain" valueReference="3" causality="parameter" variability="tunable" start="100"/>
    <Float64 name="limit" valueReference="4" causality="parameter" variability="fixed" unit="W" start="2000"/>
    <Float64 name="saturation" valueReference="5" causality="calculatedParameter" variability="fixed" unit="K"/>
    <Float64 name="power" valueReference="6" causality="output" variability="continuous" unit="W"/>
    <Enumeration name="mode" valueReference="7" declaredType="Mode" causality="output" variability="discrete"/>
  </ModelVariables>
  <ModelStructure>
    <Output valueReference="6" dependencies=""/>
    <Output valueReference="7" dependencies=""/>
    <InitialUnknown valueReference="5" dependencies="1 2 3 4"/>
    <InitialUnknown valueReference="6" dependencies="1 2 3 4"/>
    <InitialUnknown valueReference="7" dependencies="1 2 3 4"/>
  </ModelStructure>
</fmiModelDescription>
# "#));
```

Where each part comes from:

- `modelName`, `instantiationToken`: `Variables`. `description`: `Fmu::DESCRIPTION`.
  `modelIdentifier`: the model name with `-` as `_`, the file name of the `cdylib`.
- `canGetAndSetFMUState`: the `export!` list. `fixedInternalStepSize`:
  `CoSimulation::FIXED_INTERNAL_STEP_SIZE`, when set.
- `<UnitDefinitions>`: one `<Unit>` per distinct unit type the fields name, its
  `<BaseUnit>` from the `Unit`'s exponents, factor and offset, defaults left out.
- `<TypeDefinitions>`: one `<EnumerationType>` per `Enumeration` type, items from
  `ITEMS`.
- `<LogCategories>`: the two fmite logs under. fmite has no debug messages.
- `<DefaultExperiment>`: `Fmu::DEFAULT_EXPERIMENT`, left out when it has nothing.
- Each `<ModelVariables>` entry: the element from `T`, the attributes from the field's
  row of Table 22, `initial` only where it is not the row's default, `start` from
  `Default` where the row has one, `<Dimension>` from an array's length. `time` is
  fmite's.
- `<ModelStructure>`: an `<Output>` depends on no input, because model code writes
  outputs in `do_step` and never in `set`, so there is no direct feedthrough. An
  `<InitialUnknown>` depends on every variable the importer may set during
  initialization, the coarsest truthful answer.

The XSD of the standard is vendored under `tests/schema`, and the tests validate the
descriptions against it with `xmllint`.

### The archive

```text
heater.fmu
├── modelDescription.xml
└── binaries/
    └── aarch64-darwin/
        └── heater.dylib
```

Entries are stored uncompressed with a fixed timestamp, so the same inputs give the
same bytes.

## 3. What the compiler proves

Table 22 is written once, as trait impls, and the wrappers are bounded by it. The
compiler's message cites the table.

**A parameter is fixed or tunable, never continuous.**

```compile_fail,E0277
let _: fmite::Parameter<f64, (), fmite::Continuous>;
```

**A constant output is exact; it cannot be calculated.**

```compile_fail,E0277
let _: fmite::Output<f64, (), fmite::Constant, fmite::Calculated>;
```

**Only floats are continuous.**

```compile_fail,E0277
let _: fmite::Output<i32, (), fmite::Continuous>;
```

**Only floats carry a unit.** FMI 3.0 puts `unit` in the float attributes alone.

```compile_fail,E0277
let _: fmite::Output<i32, fmite::unit::Meter>;
```

**An enumeration is discrete, and has no unit.**

```compile_fail,E0277
# #[derive(Clone, Copy)]
# enum Mode { Off }
# impl fmite::Enumeration for Mode {
#     const NAME: &'static str = "Mode";
#     const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1)];
#     fn to_i64(self) -> i64 { 1 }
#     fn from_i64(_: i64) -> Option<Self> { Some(Self::Off) }
# }
let _: fmite::Output<Mode, (), fmite::Continuous>;
```

```compile_fail,E0277
# #[derive(Clone, Copy)]
# enum Mode { Off }
# impl fmite::Enumeration for Mode {
#     const NAME: &'static str = "Mode";
#     const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1)];
#     fn to_i64(self) -> i64 { 1 }
#     fn from_i64(_: i64) -> Option<Self> { Some(Self::Off) }
# }
let _: fmite::Output<Mode, fmite::unit::Celsius>;
```

**Model code cannot write an input.** `Input` has `Deref` and no `DerefMut`.

```compile_fail,E0594
let mut setpoint = fmite::Input::<f64>::new(20.0);
*setpoint = 21.0;
```

**Model code cannot write a parameter.** The importer sets it; the model reads it.

```compile_fail,E0594
let mut gain = fmite::Parameter::<f64>::new(100.0);
*gain = 200.0;
```

**Model code cannot write a constant output.** Its start value is its value.

```compile_fail,E0594
let mut version = fmite::Output::<u32, (), fmite::Constant>::new(3);
*version = 4;
```

**A capability in the export list is a bound.** `State` is `Clone`; a model that is
not `Clone` cannot claim it, so `canGetAndSetFMUState="true"` is never a lie.

```compile_fail,E0277
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{
#     CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output, Parameter, Step,
#     StepResult, Tunable, ValueReference, Values, ValuesMut, Variable, Variables,
# };
#
# /// An FMI `Enumeration`: a Rust enum and the value of each item.
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
# pub enum Mode {
#     #[default]
#     Off,
#     Heating,
# }
#
# impl Enumeration for Mode {
#     const NAME: &'static str = "Mode";
#     const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1), ("Heating", 2)];
#
#     fn to_i64(self) -> i64 {
#         match self {
#             Self::Off => 1,
#             Self::Heating => 2,
#         }
#     }
#
#     fn from_i64(value: i64) -> Option<Self> {
#         match value {
#             1 => Some(Self::Off),
#             2 => Some(Self::Heating),
#             _ => None,
#         }
#     }
# }
#
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     /// Watts per kelvin of error.
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     /// The error at which the heater saturates: `limit / gain`.
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     /// A plain field is private state. The importer never sees it.
#     elapsed: f64,
# }
#
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0),
#             temperature: Input::new(20.0),
#             gain: Parameter::new(100.0),
#             limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(),
#             power: Output::default(),
#             mode: Output::default(),
#             elapsed: 0.0,
#         }
#     }
# }
#
# impl Variables for Heater {
#     const MODEL_NAME: &'static str = "heater";
#     const INSTANTIATION_TOKEN: &'static str = "{heater-1}";
#     const VARIABLES: &'static [Variable] = &[
#         Variable::new::<Input<f64, Celsius>>("setpoint", 1),
#         Variable::new::<Input<f64, Celsius>>("temperature", 2),
#         Variable::new::<Parameter<f64, (), Tunable>>("gain", 3),
#         Variable::new::<Parameter<f64, Watt>>("limit", 4),
#         Variable::new::<CalculatedParameter<f64, Kelvin>>("saturation", 5),
#         Variable::new::<Output<f64, Watt>>("power", 6),
#         Variable::new::<Output<Mode>>("mode", 7),
#     ];
#
#     fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.get_into(out),
#             2 => self.temperature.get_into(out),
#             3 => self.gain.get_into(out),
#             4 => self.limit.get_into(out),
#             5 => self.saturation.get_into(out),
#             6 => self.power.get_into(out),
#             7 => self.mode.get_into(out),
#             _ => Err(vr.unknown()),
#         }
#     }
#
#     fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.importer_set(values),
#             2 => self.temperature.importer_set(values),
#             3 => self.gain.importer_set(values),
#             4 => self.limit.importer_set(values),
#             _ => Err(vr.unknown()),
#         }
#     }
# }
#
# impl Fmu for Heater {
#     const DESCRIPTION: Option<&'static str> = Some("A proportional heater");
#
#     fn exit_initialization(&mut self) -> Result<(), Error> {
#         if *self.gain <= 0.0 {
#             return Err(Error::new("the gain must be positive"));
#         }
#         *self.saturation = *self.limit / *self.gain;
#         Ok(())
#     }
# }
#
# impl CoSimulation for Heater {
#     fn do_step(&mut self, step: Step) -> Result<StepResult, Error> {
#         let error = *self.setpoint - *self.temperature;
#         *self.power = (*self.gain * error).clamp(0.0, *self.limit);
#         *self.mode = if *self.power > 0.0 { Mode::Heating } else { Mode::Off };
#         self.elapsed += step.size;
#         Ok(StepResult::Complete)
#     }
# }
fmite::export!(Heater: CoSimulation + State);
```

**A variable list the description could not state fails at compile time.** The
instance evaluates `check` in a `const`, so a shared value reference, a reference of 0
(which is `time`'s), a shared name, or two units of one name with different
definitions stop the build of the FMU.

```compile_fail,E0080
use fmite::{Input, Output, Variable};

const _: () = fmite::check(&[
    Variable::new::<Input<f64>>("u", 1),
    Variable::new::<Output<f64>>("y", 1),
]);
```

```compile_fail,E0080
use fmite::unit::{Celsius, Unit, UnitT};
use fmite::{Input, Output, Variable};

struct Fahrenheit;

impl UnitT for Fahrenheit {
    const NAME: &'static str = "degC"; // the name is taken, by a different unit
    const UNIT: Unit = Unit::kelvin().scaled(5.0 / 9.0).offset(255.372);
}

const _: () = fmite::check(&[
    Variable::new::<Input<f64, Celsius>>("ambient", 1),
    Variable::new::<Output<f64, Fahrenheit>>("display", 2),
]);
```

**What the types do not check.** A `fixed` calculated parameter or local must not
change after initialization, since the importer may cache it; a `tunable` one may
change at any communication point. Both have `DerefMut`, so a write to a fixed one in
`do_step` compiles. Checking it would need a token argument that only
`exit_initialization` receives, and the cost in every model was judged higher than the
rule. It is the one rule of Table 22 left to review.

## 4. What the instance checks at runtime

The importer's calls arrive over a C ABI, in an order and with references no type can
see. `Instance<T>` checks them, and answers `fmi3Error` with a logged message before
`T` sees the call:

- the state machine: `fmi3DoStep` before `fmi3ExitInitializationMode`, `fmi3Get`
  before initialization, `fmi3Terminate` twice;
- the instantiation token against `T::INSTANTIATION_TOKEN`;
- the value reference against `T::VARIABLES`: unknown, of another type than the
  function called, or with fewer values than the variable has;
- the mode against the variable's row: an output or calculated parameter is never set,
  a fixed parameter only before Step Mode, an input or tunable parameter in Step Mode
  too;
- an enumeration value with no item;
- a communication step that does not start where the last one ended, or, with a fixed
  internal step, is not a whole number of them;
- a panic in model code: caught, logged as `fmi3Fatal`, and the instance refuses every
  call but `fmi3FreeInstance` from then on.

The same checks, driven from Rust:

```
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{
#     CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output, Parameter, Step,
#     StepResult, Tunable, ValueReference, Values, ValuesMut, Variable, Variables,
# };
#
# /// An FMI `Enumeration`: a Rust enum and the value of each item.
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
# pub enum Mode {
#     #[default]
#     Off,
#     Heating,
# }
#
# impl Enumeration for Mode {
#     const NAME: &'static str = "Mode";
#     const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1), ("Heating", 2)];
#
#     fn to_i64(self) -> i64 {
#         match self {
#             Self::Off => 1,
#             Self::Heating => 2,
#         }
#     }
#
#     fn from_i64(value: i64) -> Option<Self> {
#         match value {
#             1 => Some(Self::Off),
#             2 => Some(Self::Heating),
#             _ => None,
#         }
#     }
# }
#
# #[derive(Clone)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     /// Watts per kelvin of error.
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     /// The error at which the heater saturates: `limit / gain`.
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     /// A plain field is private state. The importer never sees it.
#     elapsed: f64,
# }
#
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0),
#             temperature: Input::new(20.0),
#             gain: Parameter::new(100.0),
#             limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(),
#             power: Output::default(),
#             mode: Output::default(),
#             elapsed: 0.0,
#         }
#     }
# }
#
# impl Variables for Heater {
#     const MODEL_NAME: &'static str = "heater";
#     const INSTANTIATION_TOKEN: &'static str = "{heater-1}";
#     const VARIABLES: &'static [Variable] = &[
#         Variable::new::<Input<f64, Celsius>>("setpoint", 1),
#         Variable::new::<Input<f64, Celsius>>("temperature", 2),
#         Variable::new::<Parameter<f64, (), Tunable>>("gain", 3),
#         Variable::new::<Parameter<f64, Watt>>("limit", 4),
#         Variable::new::<CalculatedParameter<f64, Kelvin>>("saturation", 5),
#         Variable::new::<Output<f64, Watt>>("power", 6),
#         Variable::new::<Output<Mode>>("mode", 7),
#     ];
#
#     fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.get_into(out),
#             2 => self.temperature.get_into(out),
#             3 => self.gain.get_into(out),
#             4 => self.limit.get_into(out),
#             5 => self.saturation.get_into(out),
#             6 => self.power.get_into(out),
#             7 => self.mode.get_into(out),
#             _ => Err(vr.unknown()),
#         }
#     }
#
#     fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
#         match vr.0 {
#             1 => self.setpoint.importer_set(values),
#             2 => self.temperature.importer_set(values),
#             3 => self.gain.importer_set(values),
#             4 => self.limit.importer_set(values),
#             _ => Err(vr.unknown()),
#         }
#     }
# }
#
# impl Fmu for Heater {
#     const DESCRIPTION: Option<&'static str> = Some("A proportional heater");
#
#     fn exit_initialization(&mut self) -> Result<(), Error> {
#         if *self.gain <= 0.0 {
#             return Err(Error::new("the gain must be positive"));
#         }
#         *self.saturation = *self.limit / *self.gain;
#         Ok(())
#     }
# }
#
# impl CoSimulation for Heater {
#     fn do_step(&mut self, step: Step) -> Result<StepResult, Error> {
#         let error = *self.setpoint - *self.temperature;
#         *self.power = (*self.gain * error).clamp(0.0, *self.limit);
#         *self.mode = if *self.power > 0.0 { Mode::Heating } else { Mode::Off };
#         self.elapsed += step.size;
#         Ok(StepResult::Complete)
#     }
# }
use fmite::abi::{Logger, Status};
use fmite::{Instance, Instantiation};

let context = Instantiation { instance_name: "demo".to_owned(), resource_path: None };
let mut heater = Instance::<Heater>::instantiate("{heater-1}", context, Logger::silent()).unwrap();

// No step before initialization.
assert_eq!(heater.do_step(0.0, 1.0).0, Status::Error);
assert_eq!(heater.enter_initialization(0.0, None), Status::Ok);
assert_eq!(heater.exit_initialization(), Status::Ok);

// An output is never set; a fixed parameter is not set in Step Mode; `power` is a
// Float64, not an Int32; reference 9 does not exist.
assert_eq!(heater.set(&[6], Values::Float64(&[1.0])), Status::Error);
assert_eq!(heater.set(&[4], Values::Float64(&[1.0])), Status::Error);
assert_eq!(heater.get(&[6], ValuesMut::Int32(&mut [0])), Status::Error);
assert_eq!(heater.get(&[9], ValuesMut::Float64(&mut [0.0])), Status::Error);

// A step, and what it computed: 100 W/K over 5 K.
assert_eq!(heater.set(&[1], Values::Float64(&[25.0])), Status::Ok);
assert_eq!(heater.do_step(0.0, 1.0), (Status::Ok, false, 1.0));
let mut power = [0.0];
assert_eq!(heater.get(&[6], ValuesMut::Float64(&mut power)), Status::Ok);
assert_eq!(power, [500.0]);
```

## 5. Compared with attribute-driven exporters

The usual exporter annotates a struct: `#[variable(causality = "input", start = 1.0)]`.
fmite moves each of those facts into a type, which changes what can go wrong.

- **Causality is the field's type, not an attribute.** Under an attribute the field is
  still a plain `f64`, so model code can write an input. `Input<f64>` has no
  `DerefMut`.
- **One start value.** `start = 1.0` beside `#[derive(Default)]` declares the value
  twice, and the two can disagree, one in the XML and one in the running model. fmite
  reads `Default` and nothing else.
- **Capabilities are checked bounds.** The export list names them and the compiler
  checks each, so a flag in the XML is never set by hand and never wrong.
- **Illegal combinations do not compile.** Table 22, the float-only rules for
  `continuous` and `unit`, and the uniqueness of value references are refused before
  an FMU exists, not by an importer after it is shipped.
- **A function the FMU does not implement answers, it does not abort.** Every export
  is `catch_unwind`ed, every unsupported one logs and returns `fmi3Error`.
- **No build script, no bindgen.** The ABI is transcribed from the headers once and
  checked against them by a test, so building an FMU needs no C toolchain of its own.
- **Zero dependencies by default.** The derive and the archive writer are features.
