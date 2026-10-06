# fmite

An FMI 3.0 implementation for Rust. `fmite` aims to cover the whole standard:
Co-Simulation, Model Exchange and Scheduled Execution. It implements
Co-Simulation, where the model ships with its own solver, and Scheduled
Execution, where the importer runs the model's clocks. A model is a struct
whose field types say what kind of FMI variable each field is. From that struct
fmite generates the 75 `fmi3*` C functions, `modelDescription.xml` and the
`.fmu` archive, and it checks every call the importer makes before the model
sees it.

This page is also a test suite. Every code block runs under `cargo test`, and
every `compile_fail` block is an FMI 3.0 rule the compiler enforces.

1. [Writing a model](#1-writing-a-model)
2. [What gets generated](#2-what-gets-generated)
3. [What the compiler checks](#3-what-the-compiler-checks)
4. [What the instance checks at runtime](#4-what-the-instance-checks-at-runtime)
5. [Scheduled Execution](#5-scheduled-execution)
6. [Why types and not attributes](#6-why-types-and-not-attributes)

## 1. Writing a model

### The struct

One field per variable. The field's type says its role. A field without a wrapper
is private state.

```
use fmite::unit::{Celsius, Kelvin, Watt};
use fmite::{CalculatedParameter, Enumeration, Input, Output, Parameter, Tunable, Variables};

/// An FMI `Enumeration`. Each item's value is its discriminant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Enumeration)]
pub enum Mode {
    #[default]
    Off = 1,
    Heating = 2,
}

#[derive(Clone, Variables)]
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
    /// Private state. The importer never sees it.
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

Each wrapper is one row of Table 22 in the FMI 3.0 standard. Unit, variability and
initial are type parameters, with the standard's defaults: floats are `Continuous`,
other types `Discrete`, parameters `Fixed`, and `initial` is the first the row
allows.

| Field type                     | `causality`           | `variability`                                            | `initial`                                   | Model code may |
| ------------------------------ | --------------------- | -------------------------------------------------------- | ------------------------------------------- | -------------- |
| `Input<T, U, V>`               | `input`               | `discrete`, `continuous`                                 | `exact`                                     | read           |
| `Parameter<T, U, V>`           | `parameter`           | `fixed`, `tunable`                                       | `exact`                                     | read           |
| `CalculatedParameter<T, U, V>` | `calculatedParameter` | `fixed`, `tunable`                                       | `calculated`                                | read, write    |
| `Output<T, U, V, I>`           | `output`              | `constant`, `discrete`, `continuous`                     | `exact`; or `calculated` unless constant    | read; write unless constant |
| `Local<T, U, V, I>`            | `local`               | `constant`, `fixed`, `tunable`, `discrete`, `continuous` | as `Output`; `calculated` if fixed, tunable | read; write unless constant |

`T` is `f32`, `f64`, `i8` to `u64`, `bool`, an [`Enumeration`], or a fixed-size array
of one of these. `U` is a unit from [`unit`](mod@unit), or your own [`unit::UnitT`].

`Default` gives the start values: `start="20"` in the model description comes from
the `20.0` above. `Clone` gives FMU state: `fmi3GetFMUState` is a clone.

### What the derives write

`#[derive(Variables)]` numbers the variables in field order from 1 (`time` is 0),
takes `MODEL_NAME` from the package name, and hashes the declarations into
`INSTANTIATION_TOKEN`, so the token changes when they do. `#[derive(Enumeration)]`
values each variant by its discriminant.

You can write the same impls by hand:

```
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{
#     CalculatedParameter, Enumeration, Error, Input, Output, Parameter, Tunable,
#     ValueReference, Values, ValuesMut, Variable, Variables,
# };
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
# pub enum Mode { #[default] Off = 1, Heating = 2 }
# #[derive(Clone)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     elapsed: f64,
# }
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

impl Variables for Heater {
    const MODEL_NAME: &'static str = env!("CARGO_PKG_NAME");
    const INSTANTIATION_TOKEN: &'static str = "{1f4a7687-20dd-7a82-37dc-878688a85983}";
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
            5 => self.saturation.importer_set(values),
            6 => self.power.importer_set(values),
            7 => self.mode.importer_set(values),
            _ => Err(vr.unknown()),
        }
    }
}
```

`Variable::new::<FieldType>` checks the field type against Table 22, so an illegal
type fails here too. `set` may list outputs and calculated parameters: the instance
refuses those before `set` is called.

### The hooks and `export!`

Every `Fmu` hook has a default. Compute a calculated parameter in
`exit_initialization`, after the importer has set the parameters. A tunable one
can be computed again in `do_step`.

```
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output};
# use fmite::{Parameter, Step, StepResult, Tunable, Values, ValuesMut, Variables};
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Enumeration)]
# pub enum Mode { #[default] Off = 1, Heating = 2 }
# #[derive(Clone, Variables)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     elapsed: f64,
# }
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0), temperature: Input::new(20.0),
#             gain: Parameter::new(100.0), limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(), power: Output::default(),
#             mode: Output::default(), elapsed: 0.0,
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

fmite::export!(Heater: CoSimulation + State);
```

The hooks, in call order: `instantiate` (defaults to `Self::default()`, and
`fmi3Reset` calls it again), `enter_initialization(start, stop)`,
`exit_initialization`, `do_step`, `terminate`. An `Err` is logged and returned to
the importer as `fmi3Error`.

`export!` goes once in the crate that builds the `cdylib`. The capabilities after the
colon are checked: `State` needs `Heater: Clone`
([proof](#3-what-the-compiler-checks)). The functions of a capability you leave out
are still exported, and return `fmi3Error` with a log message.

### `package`: the archive

With the `package` feature, on the host, after `cargo build`:

```no_run
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output};
# use fmite::{Parameter, Step, StepResult, Tunable, Values, ValuesMut, Variables};
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Enumeration)]
# pub enum Mode { #[default] Off = 1, Heating = 2 }
# #[derive(Clone, Variables)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     elapsed: f64,
# }
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0), temperature: Input::new(20.0),
#             gain: Parameter::new(100.0), limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(), power: Output::default(),
#             mode: Output::default(), elapsed: 0.0,
#         }
#     }
# }
# impl Fmu for Heater {
#     const DESCRIPTION: Option<&'static str> = Some("A proportional heater");
#     fn exit_initialization(&mut self) -> Result<(), Error> {
#         if *self.gain <= 0.0 { return Err(Error::new("the gain must be positive")); }
#         *self.saturation = *self.limit / *self.gain;
#         Ok(())
#     }
# }
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

`package` writes the model description from the type and zips it with each binary
under `binaries/<platform>/`. `Binary::host` uses the current platform. For a
cross-compiled library, write `Binary { platform, path }`.

## 2. What gets generated

### The symbols

`export!` defines all 75 functions of `fmi3Functions.h`. Each is `extern "C"`, runs
inside `catch_unwind`, and checks its pointers before using them. A C test assigns
each one to the header's function-pointer type, so a missing or misspelled symbol
fails to link.

| Group                                                                                                                              | Count | Answer                                                 |
| ---------------------------------------------------------------------------------------------------------------------------------- | ----- | ------------------------------------------------------ |
| `GetVersion`, `SetDebugLogging`, `FreeInstance`, `EnterInitializationMode`, `ExitInitializationMode`, `Terminate`, `Reset`         | 7     | implemented                                            |
| `Get`/`Set` for `Float32` to `UInt64` and `Boolean`                                                                                | 22    | implemented                                            |
| `InstantiateCoSimulation`, `DoStep`                                                                                                | 2     | implemented for `CoSimulation`; else refused           |
| `InstantiateScheduledExecution`, `ActivateModelPartition`, `GetIntervalDecimal`, `GetShiftDecimal`                                 | 4     | implemented for `ScheduledExecution`; else refused     |
| `GetFMUState`, `SetFMUState`, `FreeFMUState`                                                                                       | 3     | implemented with `State`; else refused                 |
| `InstantiateModelExchange`                                                                                                         | 1     | refused: null instance, logged                         |
| `String` and `Binary` get/set, state serialization, dependencies, derivatives, the other clock functions, configuration, event mode, Model Exchange | 36 | refused: `fmi3Error`, logged |

A refused function never panics. An importer that ignores the capability flags gets
an error message, not a crash. A refused instantiation returns a null instance.

### The model description

All of it is generated. This is the heater's. The doctests run in the `fmite`
package, so the model name here is `fmite`; in a crate named `heater` it would be
`heater`.

```
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output};
# use fmite::{Parameter, Step, StepResult, Tunable, Values, ValuesMut, Variables};
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Enumeration)]
# pub enum Mode { #[default] Off = 1, Heating = 2 }
# #[derive(Clone, Variables)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     elapsed: f64,
# }
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0), temperature: Input::new(20.0),
#             gain: Parameter::new(100.0), limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(), power: Output::default(),
#             mode: Output::default(), elapsed: 0.0,
#         }
#     }
# }
# impl Fmu for Heater {
#     const DESCRIPTION: Option<&'static str> = Some("A proportional heater");
#     fn exit_initialization(&mut self) -> Result<(), Error> {
#         if *self.gain <= 0.0 { return Err(Error::new("the gain must be positive")); }
#         *self.saturation = *self.limit / *self.gain;
#         Ok(())
#     }
# }
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
<fmiModelDescription fmiVersion="3.0" modelName="fmite" instantiationToken="{1f4a7687-20dd-7a82-37dc-878688a85983}" description="A proportional heater" generationTool="fmite 0.2.0" variableNamingConvention="structured">
  <CoSimulation modelIdentifier="fmite" canGetAndSetFMUState="true" canHandleVariableCommunicationStepSize="true"/>
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
  `modelIdentifier`: the model name with `-` as `_`, which is the `cdylib`'s file
  name.
- `canGetAndSetFMUState`: the `export!` list. `fixedInternalStepSize`:
  `CoSimulation::FIXED_INTERNAL_STEP_SIZE`, if set.
- `<UnitDefinitions>`: one `<Unit>` per unit the fields use.
- `<TypeDefinitions>`: one `<EnumerationType>` per `Enumeration`.
- `<LogCategories>`: the two categories fmite logs to. There are no debug messages.
- `<DefaultExperiment>`: `Fmu::DEFAULT_EXPERIMENT`, omitted when empty.
- `<ModelVariables>`: the element from `T`, the attributes from the field's row of
  Table 22, `start` from `Default`, `<Dimension>` from an array's length. `time` is
  added by fmite.
- `<ModelStructure>`: outputs depend on no input, because model code writes outputs
  only in `do_step`. Each `<InitialUnknown>` depends on everything the importer can
  set during initialization.

The tests validate the descriptions against the standard's XSD, vendored under
`tests/schema`, with `xmllint`.

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

## 3. What the compiler checks

Table 22 is written once as trait impls, and the wrappers are bounded by it. The
error message cites the table.

**A parameter is fixed or tunable, never continuous.**

```compile_fail,E0277
let _: fmite::Parameter<f64, (), fmite::Continuous>;
```

**A constant output cannot be calculated.**

```compile_fail,E0277
let _: fmite::Output<f64, (), fmite::Constant, fmite::Calculated>;
```

**Only floats are continuous.**

```compile_fail,E0277
let _: fmite::Output<i32, (), fmite::Continuous>;
```

**Only floats have a unit.**

```compile_fail,E0277
let _: fmite::Output<i32, fmite::unit::Meter>;
```

**An enumeration is discrete and has no unit.**

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

**Model code cannot write an input, a parameter, or a constant output.** They
implement `Deref` but not `DerefMut`.

```compile_fail,E0594
let mut setpoint = fmite::Input::<f64>::new(20.0);
*setpoint = 21.0;
```

```compile_fail,E0594
let mut gain = fmite::Parameter::<f64>::new(100.0);
*gain = 200.0;
```

```compile_fail,E0594
let mut version = fmite::Output::<u32, (), fmite::Constant>::new(3);
*version = 4;
```

**`State` needs `Clone`,** so `canGetAndSetFMUState="true"` is always true.

```compile_fail,E0277
use fmite::{CoSimulation, Error, Fmu, Output, Step, StepResult, Variables};

#[derive(Default, Variables)] // no `Clone`
pub struct Heater {
    pub power: Output<f64>,
}

impl Fmu for Heater {}

impl CoSimulation for Heater {
    fn do_step(&mut self, _: Step) -> Result<StepResult, Error> {
        Ok(StepResult::Complete)
    }
}

fmite::export!(Heater: CoSimulation + State);
```

**An invalid variable list fails to compile.** The instance runs `check` in a
`const`. A repeated value reference, a reference of 0 (that is `time`), a repeated
name, or two different units with one name all stop the build.

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
    const NAME: &'static str = "degC"; // taken by a different unit
    const UNIT: Unit = Unit::kelvin().scaled(5.0 / 9.0).offset(255.372);
}

const _: () = fmite::check(&[
    Variable::new::<Input<f64, Celsius>>("ambient", 1),
    Variable::new::<Output<f64, Fahrenheit>>("display", 2),
]);
```

**Not checked: writing a fixed calculated value after initialization.** A `fixed`
calculated parameter or local must not change after initialization, because the
importer may cache it. Both have `DerefMut`, so a write in `do_step` compiles.
Checking it would cost every model an extra argument, so this one rule of Table 22
is left to you.

## 4. What the instance checks at runtime

The importer's calls come over a C ABI, so their order and references cannot be
typed. `Instance<T>` checks each call, and returns `fmi3Error` with a log message
before `T` sees it. It refuses:

- a call in the wrong state: `fmi3DoStep` before `fmi3ExitInitializationMode`,
  `fmi3Get` before initialization, `fmi3Terminate` twice;
- an instantiation token other than `T::INSTANTIATION_TOKEN`;
- a value reference that is unknown, of the wrong type, or given too few values;
- a `set` the variable's row forbids: outputs and calculated parameters never,
  fixed parameters only before Step Mode;
- an enumeration value with no item;
- a step that does not start where the last one ended, or that is not a whole
  number of fixed internal steps.

A panic in model code is caught and logged as `fmi3Fatal`. After that the instance
refuses every call except `fmi3FreeInstance`.

The same checks, driven from Rust:

```
# use fmite::unit::{Celsius, Kelvin, Watt};
# use fmite::{CalculatedParameter, CoSimulation, Enumeration, Error, Fmu, Input, Output};
# use fmite::{Parameter, Step, StepResult, Tunable, Values, ValuesMut, Variables};
# #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Enumeration)]
# pub enum Mode { #[default] Off = 1, Heating = 2 }
# #[derive(Clone, Variables)]
# pub struct Heater {
#     pub setpoint: Input<f64, Celsius>,
#     pub temperature: Input<f64, Celsius>,
#     pub gain: Parameter<f64, (), Tunable>,
#     pub limit: Parameter<f64, Watt>,
#     pub saturation: CalculatedParameter<f64, Kelvin>,
#     pub power: Output<f64, Watt>,
#     pub mode: Output<Mode>,
#     elapsed: f64,
# }
# impl Default for Heater {
#     fn default() -> Self {
#         Self {
#             setpoint: Input::new(20.0), temperature: Input::new(20.0),
#             gain: Parameter::new(100.0), limit: Parameter::new(2000.0),
#             saturation: CalculatedParameter::default(), power: Output::default(),
#             mode: Output::default(), elapsed: 0.0,
#         }
#     }
# }
# impl Fmu for Heater {
#     const DESCRIPTION: Option<&'static str> = Some("A proportional heater");
#     fn exit_initialization(&mut self) -> Result<(), Error> {
#         if *self.gain <= 0.0 { return Err(Error::new("the gain must be positive")); }
#         *self.saturation = *self.limit / *self.gain;
#         Ok(())
#     }
# }
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
use fmite::abi::Status;
use fmite::log::Logger;
use fmite::{Instance, Instantiation, Interface};

let context = Instantiation { instance_name: "demo".to_owned(), resource_path: None };
let mut heater = Instance::<Heater>::instantiate(Heater::INSTANTIATION_TOKEN, context, Interface::CoSimulation, Logger::silent()).unwrap();

// No step before initialization.
assert_eq!(heater.do_step(0.0, 1.0).0, Status::Error);
assert_eq!(heater.enter_initialization(0.0, None), Status::Ok);
assert_eq!(heater.exit_initialization(), Status::Ok);

// Refused: setting an output, setting a fixed parameter in Step Mode, reading a
// Float64 as Int32, an unknown reference.
assert_eq!(heater.set(&[6], Values::Float64(&[1.0])), Status::Error);
assert_eq!(heater.set(&[4], Values::Float64(&[1.0])), Status::Error);
assert_eq!(heater.get(&[6], ValuesMut::Int32(&mut [0])), Status::Error);
assert_eq!(heater.get(&[9], ValuesMut::Float64(&mut [0.0])), Status::Error);

// One step: 100 W/K times 5 K.
assert_eq!(heater.set(&[1], Values::Float64(&[25.0])), Status::Ok);
assert_eq!(heater.do_step(0.0, 1.0), (Status::Ok, false, 1.0));
let mut power = [0.0];
assert_eq!(heater.get(&[6], ValuesMut::Float64(&mut power)), Status::Ok);
assert_eq!(power, [500.0]);
```

## 5. Scheduled Execution

Under Scheduled Execution the importer is the scheduler. Each clock the model
declares is a _model partition_: a piece of the model the importer runs, with
`fmi3ActivateModelPartition`, each time that clock ticks. An importer that
simulates the tasks of a controller, with their periods and priorities, runs
the FMU this way.

### Clocks are types too

A clock's schedule is a marker type that implements [`Periodic`]: an interval
and a priority. A [`Clock`] field declares the clock. A variable that belongs to
a partition takes the marker as its last type parameter, and it is discrete.

```
use core::time::Duration;
use fmite::{
    Activation, Calculated, Clock, Discrete, Error, Fmu, Input, Output, Periodic,
    ScheduledExecution, Variables,
};

pub struct Every10ms;

impl Periodic for Every10ms {
    const INTERVAL: Duration = Duration::from_millis(10);
    const PRIORITY: u32 = 0; // a lower number runs first
}

pub struct Every100ms;

impl Periodic for Every100ms {
    const INTERVAL: Duration = Duration::from_millis(100);
    const PRIORITY: u32 = 1;
}

#[derive(Clone, Default, Variables)]
pub struct Filter {
    pub fast: Clock<Every10ms>,
    pub slow: Clock<Every100ms>,
    pub sample: Input<f64, (), Discrete, Every10ms>,
    /// The fast partition's running mean.
    pub mean: Output<f64, (), Discrete, Calculated, Every10ms>,
    /// The mean, as the slow partition last read it.
    pub report: Output<f64, (), Discrete, Calculated, Every100ms>,
}

impl Fmu for Filter {}

impl ScheduledExecution for Filter {
    fn activate(&mut self, activation: Activation) -> Result<(), Error> {
        if activation.is::<Every10ms>() {
            *self.mean += (*self.sample - *self.mean) / 10.0;
        } else if activation.is::<Every100ms>() {
            *self.report = *self.mean;
        }
        Ok(())
    }
}

fmite::export!(Filter: ScheduledExecution + State);

use fmite::abi::Status;
use fmite::log::Logger;
use fmite::{Instance, Instantiation, Interface, Values, ValuesMut};

let context = Instantiation { instance_name: "demo".to_owned(), resource_path: None };
let mut filter = Instance::<Filter>::instantiate(
    Filter::INSTANTIATION_TOKEN, context, Interface::ScheduledExecution, Logger::silent(),
).unwrap();
assert_eq!(filter.enter_initialization(0.0, None), Status::Ok);
assert_eq!(filter.exit_initialization(), Status::Ok);
assert_eq!(filter.set(&[3], Values::Float64(&[10.0])), Status::Ok);
assert_eq!(filter.activate(1, 0.0), Status::Ok); // the fast clock, value reference 1
assert_eq!(filter.activate(2, 0.0), Status::Ok); // then the slow one
let mut report = [0.0];
assert_eq!(filter.get(&[5], ValuesMut::Float64(&mut report)), Status::Ok);
assert_eq!(report, [1.0]);
# let xml = fmite::description::model_description::<Filter>().unwrap();
# assert_eq!(format!("\n{xml}"), concat!(r#"
<?xml version="1.0" encoding="UTF-8"?>
<fmiModelDescription fmiVersion="3.0" modelName="fmite" instantiationToken="{3c082e27-c787-031c-ee29-375949c0a05d}" generationTool="fmite 0.2.0" variableNamingConvention="structured">
  <ScheduledExecution modelIdentifier="fmite" canGetAndSetFMUState="true"/>
  <LogCategories>
    <Category name="logStatusError" description="A refused call, and why"/>
    <Category name="logStatusFatal" description="A panic inside the FMU; the instance takes no more calls"/>
  </LogCategories>
  <ModelVariables>
    <Float64 name="time" valueReference="0" causality="independent" variability="continuous"/>
    <Clock name="fast" valueReference="1" causality="input" variability="discrete" intervalVariability="constant" intervalDecimal="0.01" priority="0"/>
    <Clock name="slow" valueReference="2" causality="input" variability="discrete" intervalVariability="constant" intervalDecimal="0.1" priority="1"/>
    <Float64 name="sample" valueReference="3" causality="input" variability="discrete" clocks="1" start="0"/>
    <Float64 name="mean" valueReference="4" causality="output" variability="discrete" clocks="1"/>
    <Float64 name="report" valueReference="5" causality="output" variability="discrete" clocks="2"/>
  </ModelVariables>
  <ModelStructure>
    <Output valueReference="4" dependencies=""/>
    <Output valueReference="5" dependencies=""/>
  </ModelStructure>
</fmiModelDescription>
# "#));
```

`activate` is told which clock ticked and when. `Activation::is` compares it with
a marker, so the partitions are an `if` over types, never over value references.
The model description writes each clock as a `<Clock>` with a constant interval
and its priority, and each clocked variable's `clocks` attribute names its clock.
A clocked variable is not listed as an `<InitialUnknown>`: the standard makes
that optional, and leaving it out is the form every importer accepts.

### What the importer sees

Two partitions never run at once on one instance. When two clocks tick at the
same time, the importer runs the lower priority number first. So, in the
example, a slow partition at a tick both share reads the mean the fast
partition has just written. In the other direction, a fast partition that read
a slow partition's output would read the value from the slow partition's
previous tick. That is what separate tasks on a real scheduler compute, too.

**Each partition holds the importer's preemption lock.** The standard lets an
importer interrupt a running partition from another thread, to run a more
urgent one. Two calls into one Rust value at once would be undefined behavior,
so every call takes the lock (`lockPreemption`, `unlockPreemption`) that the
importer passed at instantiation before it touches the instance. Partitions
still start in priority order, but a running partition is never interrupted.
The standard allows this, and it is sound.

**The instance checks the calls the standard adds.**

- `fmi3ActivateModelPartition` is for a clock, in Clock Activation Mode, at a time
  later than that clock's previous activation. Clocks are not ordered among
  themselves.
- After an `fmi3Set`, an `fmi3Get` waits for the next activation.
- A failed call fails every call after it, until `fmi3Terminate` or `fmi3Reset`.
- `fmi3GetIntervalDecimal` answers each clock's interval, as
  `fmi3IntervalUnchanged`, and `fmi3GetShiftDecimal` answers 0.

**What is not implemented:** output, countdown, triggered, and tunable clocks;
intervals as fractions; and configuration modes.

### What the compiler checks

**A clocked variable is discrete.**

```compile_fail,E0277
# pub struct Every10ms;
# impl fmite::Periodic for Every10ms {
#     const INTERVAL: core::time::Duration = core::time::Duration::from_millis(10);
#     const PRIORITY: u32 = 0;
# }
let _: fmite::Input<f64, (), fmite::Continuous, Every10ms>;
```

**A variable's clock is declared, and two clocks differ.**

```compile_fail,E0080
# use core::time::Duration;
use fmite::{Calculated, Clock, Discrete, Output, Periodic, Variable};

pub struct Fast;
impl Periodic for Fast {
    const INTERVAL: Duration = Duration::from_millis(10);
    const PRIORITY: u32 = 0;
}

pub struct Slow;
impl Periodic for Slow {
    const INTERVAL: Duration = Duration::from_millis(100);
    const PRIORITY: u32 = 1;
}

const _: () = fmite::check(&[
    Variable::new::<Clock<Fast>>("fast", 1),
    Variable::new::<Output<f64, (), Discrete, Calculated, Slow>>("y", 2), // no Clock<Slow>
]);
```

```compile_fail,E0080
# use core::time::Duration;
use fmite::{Clock, Periodic, Variable};

pub struct Fast;
impl Periodic for Fast {
    const INTERVAL: Duration = Duration::from_millis(10);
    const PRIORITY: u32 = 0;
}

pub struct Twin; // the same schedule: the same partition
impl Periodic for Twin {
    const INTERVAL: Duration = Duration::from_millis(10);
    const PRIORITY: u32 = 0;
}

const _: () = fmite::check(&[
    Variable::new::<Clock<Fast>>("fast", 1),
    Variable::new::<Clock<Twin>>("twin", 2),
]);
```

**A Scheduled Execution FMU has a clock.**

```compile_fail,E0080
use fmite::{Activation, Error, Fmu, Output, ScheduledExecution, Variables};

#[derive(Default, Variables)]
pub struct Clockless {
    pub y: Output<f64>,
}

impl Fmu for Clockless {}

impl ScheduledExecution for Clockless {
    fn activate(&mut self, _: Activation) -> Result<(), Error> {
        Ok(())
    }
}

fmite::export!(Clockless: ScheduledExecution);
```

**A Co-Simulation FMU has none.** The standard gives a Co-Simulation FMU with
clocks Event Mode, which fmite does not implement.

```compile_fail,E0080
# use core::time::Duration;
use fmite::{Clock, CoSimulation, Error, Fmu, Periodic, Step, StepResult, Variables};

pub struct Every10ms;
impl Periodic for Every10ms {
    const INTERVAL: Duration = Duration::from_millis(10);
    const PRIORITY: u32 = 0;
}

#[derive(Default, Variables)]
pub struct Ticking {
    pub tick: Clock<Every10ms>,
}

impl Fmu for Ticking {}

impl CoSimulation for Ticking {
    fn do_step(&mut self, _: Step) -> Result<StepResult, Error> {
        Ok(StepResult::Complete)
    }
}

fmite::export!(Ticking: CoSimulation);
```

## 6. Why types and not attributes

A common design annotates a struct: `#[variable(causality = "input", start = 1.0)]`.
fmite puts each of those facts in a type instead.

- **Inputs are read-only.** Under an attribute the field is a plain `f64` that model
  code can write. `Input<f64>` has no `DerefMut`.
- **One start value.** An attribute's `start` and `Default` can disagree. fmite reads
  only `Default`.
- **Mistakes fail to compile.** Illegal Table 22 combinations, wrong capabilities and
  repeated value references are caught before an FMU exists.
- **No build script, no bindgen, no C toolchain.** The ABI is copied from the headers
  once, and a test checks it against them.
