# fmite design

Status: a proposal for review. Nothing here is implemented. Names and signatures are
expected to change.

## Scope

fmite is the FMU side of FMI 3.0, built in this order:

1. **Co-Simulation** (v0.1). The importer instantiates the FMU, sets its parameters,
   initializes it, and calls `fmi3DoStep` for each communication step. The FMU
   advances itself with its own solver.
2. **Scheduled Execution.** The importer activates clocked model partitions, which is
   how a synchronous, multi-rate model is driven.
3. **Model Exchange.** The importer integrates the model's derivatives using the global solver.

All three traits are designed now and implemented in that order. Model Exchange ships
last, but its shape is fixed before v0.1. If it does not fit under the core trait,
the core is factored wrong, and that has to be found out before v0.1 freezes it.

## Why a synchronous model fits Co-Simulation

FMI is often read as an ODE standard. Its ODE machinery lives in Model Exchange:
continuous states, derivatives, event indicators. Co-Simulation treats the model as a
black box that is initialized once and then stepped, which is the interface of a
synchronous dataflow model with an `init` and a `step` over fixed ticks. Such a model
is the simplest kind of Co-Simulation FMU there is. FMI 3.0's clocks and Scheduled
Execution were added for synchronous, multi-rate models, so the second milestone
moves further in the same direction.

## The principle: types check the author, the instance checks the importer

Two parties can call an FMU wrongly, and they need different tools.

- **The FMU's author** writes Rust. Whatever the author can get wrong should be a
  compile error: writing to an input, mutating a fixed parameter, declaring a
  variable whose FMI type disagrees with its Rust type, handing an integer to a
  partition that expects a clock, claiming a capability the type does not have.
- **The importer** sends calls across a C ABI at runtime. Its call order and its value
  references are data, and no type can check them. `Instance<T>` checks them instead:
  the mode state machine, unknown value references, type mismatches, a `set` on a
  variable the current mode does not allow. It answers each one with `fmi3Error` and
  a log message before `T` sees the call.

The rest of this document applies that split layer by layer. Wherever a fact about the
model can be carried by a Rust type, it is, and the XML is derived from that type.
Attributes carry only what has no type to live in, such as a description string.

## What a hand-written FMU looks like

```rust
use fmite::unit::Celsius;
use fmite::{CoSimulation, Error, Fmu, Input, Output, Parameter, Step, StepResult};

#[derive(fmite::Enumeration, Clone, Copy, Default)]
enum Mode {
    #[default]
    Off,
    Heat,
}

#[derive(fmite::Variables, Clone, Default)]
struct Thermostat {
    setpoint: Input<f64, Celsius>,
    temperature: Input<f64, Celsius>,
    gain: Parameter<f64>,
    heater: Output<f64>,
    mode: Output<Mode>,
    history: Output<[f64; 8], Celsius>,

    integral: f64, // a plain field is private state, not an FMI variable
}

impl Fmu for Thermostat {
    type Log = ();
}

impl CoSimulation for Thermostat {
    fn do_step(&mut self, step: Step) -> Result<StepResult, Error> {
        let error = *self.setpoint - *self.temperature;
        self.integral += error * step.size();
        *self.heater = *self.gain * error + self.integral;
        *self.mode = if *self.heater > 0.0 { Mode::Heat } else { Mode::Off };
        Ok(StepResult::Complete)
    }
}

fmite::export!(Thermostat: CoSimulation + State);
```

Then, on the host:

```rust
fmite::package::<Thermostat>(&binaries, "thermostat.fmu")?;
```

From those lines the author gets:

- **No get/set code.** The derive writes the value-reference table.
- **No start values written twice.** `Default` gives them.
- **No unit strings.** `<UnitDefinitions>` and every `unit="degC"` are written from
  `Celsius`.
- **No capability flags set by hand.** The `export!` list gives them, and the
  compiler checks the list.
- **A model that cannot write `*self.setpoint = …`.** An `Input` has no `DerefMut`,
  so the line does not compile.

## Variables are typed fields

### Causality is the wrapper; variability and initial are its parameters

FMI 3.0 Table 22 lists which causality and variability pairs are legal, and which
`initial` values each legal pair allows. fmite writes that table once, as trait
impls, and the wrapper types are bounded by it. An illegal combination is a compile
error that cites the table.

Three columns of the table never reach the author:

- **`independent`.** fmite declares `time` itself.
- **`structuralParameter`.** A structural parameter sizes a dimension, and dimensions
  are const generics, fixed at compile time. It returns with variable-size arrays, if
  those ever come.
- **`initial="approx"`.** It only seeds the importer's solver for algebraic loops
  during Model Exchange initialization. It is deferred with Model Exchange.

What is left:

| Field type                     | `causality`           | `variability`                                            | `initial`                                                                     | Model code can                        |
| ------------------------------ | --------------------- | -------------------------------------------------------- | ----------------------------------------------------------------------------- | ------------------------------------- |
| `Input<T, U, V>`               | `input`               | `discrete`, `continuous`                                 | `exact`                                                                       | read                                  |
| `Parameter<T, U, V>`           | `parameter`           | `fixed`, `tunable`                                       | `exact`                                                                       | read                                  |
| `CalculatedParameter<T, U, V>` | `calculatedParameter` | `fixed`, `tunable`                                       | `calculated`                                                                  | read                                  |
| `Output<T, U, V, I>`           | `output`              | `constant`, `discrete`, `continuous`                     | constant: `exact`; else `exact` or `calculated`                               | read; write if discrete or continuous |
| `Local<T, U, V, I>`            | `local`               | `constant`, `fixed`, `tunable`, `discrete`, `continuous` | constant: `exact`; fixed, tunable: `calculated`; else `exact` or `calculated` | read; write if discrete or continuous |
| plain field                    | not exposed           | —                                                        | —                                                                             | anything                              |

`U` is the unit (see [Units are types](#units-are-types)) and defaults to `()`, no
unit. `V` defaults to the standard's default for `T`: `continuous` for floats,
`discrete` otherwise, and `fixed` for the two parameter kinds. `I` defaults to what
the table lists first for the cell: `exact` for a `constant`, `calculated`
otherwise. So `Output<f64>` is a continuous, calculated output, and
`Output<f64, (), Discrete, Exact>` is one that holds its start value until the first
step.

The wrappers are aliases of one struct, `Field<C, T, U, V, I>`, with `C` a causality
marker, `V` a variability marker and `I` an initial marker. Each marker is a type and
nothing else: there is no parallel enum, and the marker's trait carries the attribute
text it writes (`const NAME: &str = "tunable"`).

The table is one macro, one line per row. Each line names the row's legal initials,
default first, and flags who may set or write the variable:

```rust
table! {
    Parameter: Tunable => [Exact] initialization step;
    Output: Constant => [Exact];
    Output: Discrete => [Calculated, Exact] initialization writable;
    // …
}
```

A line expands to three things. `VariabilityFor<C>`, one impl per row, carries the
default initial and the two host flags. `InitialFor<C, V>`, one impl per cell, admits
each listed initial. `Writable<V>`, for rows flagged `writable`, is what `DerefMut`
is bounded by:

```rust
#[diagnostic::on_unimplemented(
    message = "a `{C}` variable cannot have variability `{Self}`",
    note = "FMI 3.0 Table 22"
)]
pub trait VariabilityFor<C: Causality>: Variability {
    type DefaultInitial: InitialFor<C, Self>;
    const INITIALIZATION: bool;
    const STEP: bool;
}

pub trait InitialFor<C, V: ?Sized>: Initial {}

pub type Output<
    T,
    U = (),
    V = <T as FmiType>::DefaultVariability,
    I = <V as VariabilityFor<causality::Output>>::DefaultInitial,
> = Field<causality::Output, T, U, V, I>;
```

`Output<f64, (), Fixed>` fails with "a `Output` variable cannot have variability
`Fixed`" and the table's note. One rule depends on the value type: only floats are
`continuous`, so `Output<i32, (), Continuous>` fails with "`i32` is not a float". The
message sits on the `Float` trait, because a failed blanket impl reports the bound it
missed, not its own message.

Variability also decides who may write. `Field` is `#[repr(transparent)]` over
`T`, and `Deref` is implemented on every wrapper. `DerefMut` is implemented only on
`Output` and `Local`, and only where `V` is `Discrete` or `Continuous`, so
`*self.c = …` on an `Output<f64, (), Constant>` does not compile. A `fixed` or
`tunable` `Local` and a `CalculatedParameter` are written during initialization, and
a tunable one again when a parameter changes. How model code writes them then,
without a `DerefMut` it could also use in `do_step`, is an open question.

The importer's writes to inputs and parameters go through a `#[doc(hidden)]` path
that the derive calls. Hand-written code could call it too. What the types guarantee
is that model code cannot write an input _by accident_; they do not seal it against
deliberate misuse.

A variable with `initial="exact"` gets a start value, read from `Default`. One with
`initial="calculated"` gets none, because the model computes it.

A field without a wrapper is private state. Exposing a variable to the importer is a
deliberate act: you change the field's type.

### The FMI type is the Rust type

A sealed trait, `FmiType`, maps Rust types to FMI types:

| Rust                            | FMI                                 |
| ------------------------------- | ----------------------------------- |
| `f32`, `f64`                    | `Float32`, `Float64`                |
| `i8` … `u64`                    | `Int8` … `UInt64`                   |
| `bool`                          | `Boolean`                           |
| `String`                        | `String`                            |
| `Vec<u8>`                       | `Binary`                            |
| `[T; N]`, `[[T; N]; M]`         | `T` with one `<Dimension>` per rank |
| `#[derive(fmite::Enumeration)]` | `Enumeration` + `EnumerationType`   |

Array dimensions come from const generics, so `<Dimension>` cannot disagree with the
field. An enumeration crosses the C API as `Int64`. The instance converts it with
`TryFrom<i64>`, so an importer that sends an integer with no variant gets `fmi3Error`,
and the model only ever holds a valid `Mode`.

### Units are types

FMI does not fix a list of units. A unit is a name the author chooses, defined by the
exponents of the SI base units and a linear map to them:

```xml
<Unit name="degC"><BaseUnit K="1" offset="273.15"/></Unit>
```

There are eight exponents, not seven: `kg`, `m`, `s`, `A`, `K`, `mol` and `cd`, plus
`rad`, which the standard counts as a base unit so that rad/s and Hz stay distinct.
`factor` and `offset` give the conversion: a value `v` in the unit is
`factor * v + offset` in the base units.

fmite mirrors that. `Unit` is `<BaseUnit>`, one field per attribute, built in `const`:

```rust
pub struct Unit {
    pub kilogram: i8,
    pub meter: i8,
    pub second: i8,
    pub ampere: i8,
    pub kelvin: i8,
    pub mole: i8,
    pub candela: i8,
    pub radian: i8,
    pub factor: f64,
    pub offset: f64,
}

pub trait UnitT {
    const NAME: &'static str;
    const UNIT: Unit;
}
```

The exponents are named fields, not an array indexed by an enum of the eight: nothing
would tie the array's length to the enum's variants, and a field needs no index. A
kilogram is a `Unit` like any other, with one exponent set to 1, so there is no
separate base-unit type. `times` and `per` spell every field out, so a ninth base
unit does not compile until they combine it too. All of `Unit`'s functions are
`const fn`s:

- `Unit::one()` has every exponent zero, and `Unit::meter()`, `Unit::volt()` and the
  rest build the units fmite ships;
- `times` and `per` add and subtract exponents and multiply and divide factors, and
  `pow(n)` is `n` of them;
- `scaled` multiplies the factor, and `offset` sets the offset.

`times` and `per` are methods, not `Mul` and `Div`: a `Unit` is built in a `const`,
and stable Rust cannot call an operator in one.

`times` and `per` drop the offset, because an offset unit does not compose: a degree
Celsius per second is a kelvin per second.

A unit holds all eight exponents, mostly zeros, and that is the smallest form: eight
`i8`s are eight bytes, the size of one pointer, so a list of only the non-zero
exponents would cost more than the zeros it drops. The zeros stay out of the model
description instead. `<BaseUnit>` defaults every exponent to 0, `factor` to 1 and
`offset` to 0, so the writer omits an attribute at its default, and a newton is
`<BaseUnit kg="1" m="1" s="-2"/>`.

The unit is the `U` parameter of every wrapper, `Input<f64, Celsius>`, and
`<UnitDefinitions>` is the set of units the variables name. A type parameter must be
a type, and stable Rust takes no struct as a const generic, so `Unit` cannot be the
parameter itself. `UnitT` is the bridge: a type standing for one `Unit`, and the place
its name lives. The name is not part of `Unit`, because a `const fn` cannot join
strings: `Unit::meter().per(Unit::second())` has exponents, but no name until a type
gives it one.

`fmite::unit` ships a type for each SI base and derived unit, `Volt`, `Ohm` and the
rest, plus `Celsius`, `AmpereHour` and `WattHour`, and that list is the only one. One
line declares both the type and its `const fn` on `Unit`. A derived unit is built from
the others, so its exponents are computed, not typed in:

```rust
Watt, watt = "W", Unit::joule().per(Unit::second());
Volt, volt = "V", Unit::watt().per(Unit::ampere());
Ohm, ohm = "Ohm", Unit::volt().per(Unit::ampere());
```

A battery model names `Output<f64, unit::Volt>` and declares nothing. Any other unit
is one impl:

```rust
struct KmPerHour;

impl UnitT for KmPerHour {
    const NAME: &'static str = "km/h";
    const UNIT: Unit = Unit::meter().scaled(1000.0).per(Unit::second().scaled(3600.0));
}
```

Only float variables carry a unit, because FMI 3.0 puts `unit` in
`fmi3RealBaseAttributes` alone: an integer has `quantity` but no unit, and a boolean,
string or enumeration has neither. `UnitOf<T>` encodes that. It is implemented for
`()` on every type, and for every `U: UnitT` on `f32`, `f64` and arrays of them, so
`Output<i32, Celsius>` fails with "`i32` is not a float". `UnitOf` is a relation, like
`VariabilityOf`, and fmite alone implements it; an author declares a unit through
`UnitT`. It cannot fold into `UnitT`: `()` would then be a `UnitT` too, and the impl
for a unit on a float would overlap the impl for `()`.

The unit is a `PhantomData` marker and does nothing at runtime. In particular it does
no dimensional analysis: `Deref` hands model code a plain `f64`, and
`*self.setpoint - *self.temperature` is not checked for agreeing units. A model that
wants checked arithmetic keeps its quantities in a units crate and converts at the
field. fmite's job is to describe the interface, and dimensional analysis inside
the model is a separate concern with a separate dependency.

Two units with the same `name` and different `base`s would write two contradictory
`<Unit>` elements. The derive emits a `const` assertion over `VARIABLES` that refuses
this at compile time, and `description` checks it again for hand-written impls.

### Start values are `Default`

The start value of a variable is the value the field holds in `T::default()`. There
is no `start = …` attribute. An attribute would be a second declaration of the same
number, and the two could disagree: one would end up in the XML and the other in the
running model. `fmite::package` builds `T::default()` on the host and reads the start
values from it.

### Value references are field order

The derive numbers variables in declaration order, after the independent variable
`time`, which fmite declares itself. A value reference only has to be stable within
one FMU build, because importers resolve references from that build's
`modelDescription.xml`. Reordering fields is therefore safe.

### The derive is sugar over a trait

`#[derive(fmite::Variables)]` implements a trait that can also be written by hand:

```rust
pub trait Variables {
    const MODEL_NAME: &'static str;
    const INSTANTIATION_TOKEN: &'static str;
    const VARIABLES: &'static [Variable]; // name, value reference, FMI type, attributes, start, settability, unit, dims, …

    fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error>;
    fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error>;
}
```

`Values` and `ValuesMut` are enums over typed slices: `Float64(&[f64])`,
`Int32(&[i32])`, and so on. That makes two methods instead of twenty-six, and the
pairing of each reference with its type is still checked: `Instance` looks every
reference up in `VARIABLES` before calling `get` or `set`, so the implementation never
sees an unknown reference or a mismatched type.

The derive expands in the implementor's crate, so it fills `MODEL_NAME` from that
crate's `CARGO_PKG_NAME`. It computes `INSTANTIATION_TOKEN` as a hash of the
declarations it saw, so the token changes exactly when the variable list does. A
hand-written implementation supplies both.

The derive lives in `fmite-derive`, behind the `derive` feature, because it needs
`syn`. The default build keeps zero dependencies.

A code generator does not use the derive. Magnetite's model crate stays FMI-free. Its
generated glue implements `Variables` directly, as a `match` from value references to
the fields of the model's typed input, output and parameter structs.

The glue still gets the Table 22 checks. It never uses the wrappers as fields, but it
names them as type-level descriptions. Every field type implements `Definition`,
which reads its row of the table: the attribute text the model description writes,
whether it has a start value, whether the host may set it in Initialization and in
Step Mode, and its unit. `Variable::new` is a `const fn` that copies those out:

```rust
const VARIABLES: &'static [Variable] = &[
    Variable::new::<Output<f64, Celsius, Discrete>>("heater", 1),
    Variable::new::<Parameter<f64, (), Tunable>>("gain", 2),
];
```

`Variable::new::<Output<f64, (), Fixed>>` fails to compile exactly as the field would,
so the derive and the glue are checked against one copy of the table.

## Traits

### `Fmu`: what every interface shares

```rust
pub trait Fmu: Variables + Default + Sized {
    type Log: LogCategory; // `()` when the FMU logs no categories

    const DESCRIPTION: Option<&'static str> = None;

    fn instantiate(cx: &Instantiation<'_>) -> Result<Self, Error> { Ok(Self::default()) }
    fn enter_initialization(&mut self, start: f64, stop: Option<f64>) -> Result<(), Error> { Ok(()) }
    fn exit_initialization(&mut self) -> Result<(), Error> { Ok(()) }
    fn terminate(&mut self) -> Result<(), Error> { Ok(()) }
    fn reset(&mut self) -> Result<(), Error> { *self = Self::default(); Ok(()) }
}
```

Every method has a default, so a model that needs no setup implements `Fmu` in one
line. `Log` is a type and not a list of strings. The implementor's enum derives
`LogCategory`, `<LogCategories>` is written from it, and `log!(cx, Category::Solver,
…)` cannot name a category the FMU did not declare.

### One trait per interface

```rust
pub trait CoSimulation: Fmu {
    const FIXED_INTERNAL_STEP_SIZE: Option<f64> = None;
    fn do_step(&mut self, step: Step) -> Result<StepResult, Error>;
}

pub trait ScheduledExecution: Fmu {
    type Partition: Partition; // derived enum, one variant per clock
    fn activate(&mut self, partition: Self::Partition, time: f64) -> Result<(), Error>;
}

pub trait ModelExchange: Fmu {
    type States: ContinuousStates; // e.g. [f64; 4], or a derived struct
    type Indicators: EventIndicators;
    fn set_time(&mut self, time: f64);
    fn states(&self) -> Self::States;
    fn set_states(&mut self, x: &Self::States);
    fn derivatives(&self, dx: &mut Self::States);
    fn event_indicators(&self, z: &mut Self::Indicators);
    fn completed_integrator_step(&mut self) -> Result<StepEvent, Error>;
}
```

Associated types replace counts and raw references:

- **`ScheduledExecution::Partition`** is an enum:

  ```rust
  #[derive(fmite::Partition)]
  enum Rate {
      #[clock(period = "1/100")]
      Fast,
      #[clock(period = "1/10")]
      Slow,
  }
  ```

  The derive writes one input clock per variant. Each gets
  `intervalVariability="constant"` and its interval in the standard's exact fraction
  form. `activate` receives a `Rate`, never a `u32`. Magnetite's `@every` lowers to
  exactly this.

- **`ModelExchange::States`** fixes the number of continuous states in the type. That
  number is the `nx` the importer has to pass, so the instance checks it and the model
  never sees a slice of the wrong length.

One FMU may implement several interfaces. An importer picks one when it instantiates,
and the instance stays in that interface's state machine.

### Capabilities cut across interfaces

| Capability       | Requires            | Gives                                                       |
| ---------------- | ------------------- | ----------------------------------------------------------- |
| `State`          | `T: Clone`          | `fmi3Get/Set/FreeFMUState`, `canGetAndSetFMUState`          |
| `SerializeState` | `T: SerializeState` | the three serialize functions, `canSerializeFMUState`       |
| `EventMode`      | later               | `fmi3EnterEventMode`, `fmi3UpdateDiscreteStates`            |
| `Clocks`         | later               | clock get/set, intervals and shifts in Co-Simulation and ME |

Saving FMU state is `Clone`. The instance stores a boxed copy of `T` together with its
own mode and time. Most models get the capability by deriving `Clone`, and the copy is
a typed value, not a byte format. Serializing to bytes is a separate trait, because a
byte format is a decision the implementor has to own:

```rust
pub trait SerializeState: Sized {
    fn save(&self, out: &mut Vec<u8>);
    fn restore(bytes: &[u8]) -> Result<Self, Error>;
}
```

## `export!`: the one place symbols are made

```rust
fmite::export!(Thermostat: CoSimulation + State);
```

The list is explicit, for two reasons.

- **A macro cannot see a type's impls.** A derive receives the tokens of one item and
  nothing else. Detecting impls inside the expansion is possible with autoref
  specialization, but a failed detection falls back silently. An FMU whose `State`
  impl does not quite satisfy the bound would export stubs and write
  `canGetAndSetFMUState="false"`, and nothing would fail until an importer tried it.
  The explicit list turns that into a compile error.
- **The C symbols must exist exactly once.** FMI permits one FMU per shared library,
  because the library name is the `modelIdentifier` and the 75 `fmi3…` symbols are
  global. A derive would emit them wherever it is placed. Two derived types in one
  crate would then fail to link, and every crate depending on the model crate would
  pick them up too. Making the symbols is therefore a separate statement, made once,
  in the cdylib.

`export!` is a `macro_rules!` macro. For every function, the body:

1. Runs inside `catch_unwind`. A panic becomes `fmi3Fatal` and a log message, never an
   unwind across the ABI.
2. Checks the instance pointer, then turns every pointer-and-count pair into a slice.
   A null pointer with a count of zero becomes an empty slice; any other null pointer
   is answered with `fmi3Error`.
3. Calls the matching method on `Instance<T>`.

Functions of interfaces and capabilities that are not in the list are still exported.
They answer `fmi3Error` and log the missing capability, so an importer that ignores
`modelDescription.xml` gets an error, not a crash.

The macro expands to `unsafe` code in the implementor's crate. That crate can keep
`deny(unsafe_code)` but not `forbid`; the documentation will say so.

## Layers

`#![deny(unsafe_code)]` holds for the whole crate. Only the two bottom layers lift it.

```
package      (feature)   FMU archive: modelDescription.xml + binaries/<arch>-<os>/
description              modelDescription.xml as types, written from Variables + Fmu + the export list
fmu, cosim, se, me       Fmu, the interface traits, the capability traits, the wrapper types
instance                 Instance<T>: mode state machine, variable checks, logger, token check
export!      (unsafe)    the 75 #[unsafe(no_mangle)] extern "C" fn fmi3… symbols
abi          (unsafe)    #[repr(C)] types and signatures, hand-written from the headers
fmite-derive (feature)   Variables, Enumeration, Partition, LogCategory
```

### `abi`

This module is a Rust transcription of `fmi3PlatformTypes.h` and
`fmi3FunctionTypes.h`. It covers `fmi3Status`, the scalar typedefs, the instance and
state handles, the callback types, and one type alias per function.

There is no bindgen. The headers are stable, published documents, so transcribing them
once costs less than putting libclang on every user's machine.

Drift is caught by a dev-only test. It compiles a C file against the vendored official
headers, and the C file assigns every symbol of an example FMU to the header's
function-pointer type. If a signature drifts, CI fails; a user's build never does.

### `instance`

`Instance<T>` owns:

- the model `T`;
- the interface it was instantiated as;
- the importer's log callback and its instance environment;
- whether debug logging is on, and which categories are enabled;
- the current mode.

The modes are Instantiated, InitializationMode, StepMode, Terminated and Reset, with
EventMode, ConfigurationMode and the Scheduled Execution and Model Exchange modes
later. The mode is an enum, and each variant holds only what that mode allows. The
state machine is typestate inside fmite, where the compiler can check it, rather than
in the public trait, where the importer's runtime call order would defeat it.

Every call is checked before `T` sees it:

- against the standard's state machine: a call in the wrong mode is answered with
  `fmi3Error` and a log message naming the mode;
- against `T::VARIABLES`: an unknown value reference, a type mismatch, or a `set` that
  the variable's `settable_in_initialization` or `settable_in_step` forbids in the
  current mode. The instance never branches on causality or variability: the table
  answered every question it asks when the model was compiled.

The instance also checks the instantiation token against `T::INSTANTIATION_TOKEN`. A
mismatch refuses instantiation, logged, with a null return.

### Time, and why it is f64

Variable values are not all f64: each has the Rust type of its field. f64 carries only
_time_, because the C API defines `startTime`, `stopTime`,
`currentCommunicationPoint` and `communicationStepSize` as `fmi3Float64`.

Converting importer time into model ticks is the step that can lose precision. 0.1 s
has no exact f64 representation, and ten steps of 0.1 do not add up to 1.0. So fmite:

- passes the raw values in `Step`, so nothing is rounded without the implementor
  seeing it;
- offers `Step::ticks(period) -> Result<u64, Error>`, which rounds to the nearest whole
  number of ticks within a documented relative tolerance, and refuses a step that is
  not close to a whole number of them;
- tracks time as `start + ticks × period` instead of summing step sizes, so error does
  not accumulate over a long run.

Clocks use the standard's _fraction_ form, `counter / resolution` with both values
`u64`, which is exact. That is why `#[clock(period = "1/100")]` takes a fraction.

### `description`

This layer holds `modelDescription.xml` as plain Rust types, and a writer that has no
dependencies. Nothing in it is set by hand. It is assembled from:

- `Variables` and `Fmu`: model attributes, and `<LogCategories>` from `T::Log`;
- `Variables`: `<ModelVariables>` with every type, causality, variability, initial,
  start, and fixed `<Dimension>`s, and `<TypeDefinitions>` for enumerations;
- the interface traits: one `<CoSimulation>`, `<ScheduledExecution>` or
  `<ModelExchange>` element for each one in the export list, with their flags from the
  traits' consts;
- the capability list: `canGetAndSetFMUState`, `canSerializeFMUState`;
- `<ModelStructure>`, with `dependencies` always written explicitly;
- `Variables`: `<UnitDefinitions>`, one `<Unit>` per distinct unit name the variables
  carry, refused if two of them share a name and differ in `Unit`;
- `<DefaultExperiment>`.

CI validates the output against the official XSD.

### `package` (feature)

A packaged FMU needs the compiled shared library for every platform it supports, and
a value in memory is not that library. Cargo has no post-build hook that could hand it
over. Packaging is therefore a host-side call on the type, not a method on an
instance:

```rust
fmite::package::<Thermostat>(&[
    ("x86_64-linux", "target/x86_64-unknown-linux-gnu/release/libthermostat.so"),
    ("aarch64-darwin", "target/aarch64-apple-darwin/release/libthermostat.dylib"),
], "thermostat.fmu")?;
```

The host links the implementor's crate as an rlib, builds the description from the
type, and zips it with the given binaries under `binaries/<arch>-<os>/`. It optionally
adds `documentation/` and `resources/`. The only dependency is `zip`, with default
features off, writing stored (uncompressed) entries.

A later `cargo fmite package --target … --target …` subcommand wraps the builds and
this call into one command.

## Where this differs from rust-fmi

rust-fmi's exporter also derives a model from an annotated struct, and that is the
right instinct. fmite moves the facts out of attributes and into types:

- **Causality is a type, not an attribute value.** Under
  `#[variable(causality = Input)]` the field is still a mutable `f64`, so model code
  can write it. `Input<f64>` has no `DerefMut`.
- **Start values have one source.** An attribute `start = 1.0` next to
  `#[derive(Default)]` declares the start value twice, and the two can disagree.
  fmite reads `Default` and nothing else.
- **Capabilities are checked bounds.** The export list names them, and the compiler
  checks each one. They are not hand-set booleans, and they are not methods with
  `todo!()` bodies.
- **Enumerations, clocks, dimensions and continuous states are types.** An enum, a
  partition enum, `[T; N]` and an associated `States` type replace integers, raw
  references and counts.
- **Variability, initial and units are types.** Type parameters bounded by the
  standard's Table 22 replace attribute values, so an illegal combination is a compile
  error, and a unit's exponents are computed from other units instead of typed in.
- **The derive is optional.** `Variables` is a public trait, so a code generator
  implements it directly and the default build has no dependencies.

## Open questions

- Is the unit the right second parameter? It is likelier than a non-default
  variability, which keeps `Input<f64, Celsius>` short, but it puts `()` in
  `Output<f64, (), Discrete>`.
- Should a unit declare display units (`<DisplayUnit>`, e.g. degF shown for degC), as
  a field of `Unit`, or is that out of scope for v0.1?
- Magnetite's glue declares a fixed-step model's float outputs `Discrete`, because
  they are piecewise constant. Should fmite also default floats to `discrete` when
  the FMU implements only Co-Simulation, or stay with the standard's default?
- How does model code write a `fixed` or `tunable` `Local` and a
  `CalculatedParameter` during initialization, without a `DerefMut` that would also
  let `do_step` write them?
- Should `do_step` receive the importer's `noSetFMUStatePriorToCurrentPoint`, or should
  `Instance` keep that to itself?
- What relative tolerance should `Step::ticks` use, and should the implementor be able
  to choose it?
- `reset` defaults to `*self = Self::default()`. That drops anything `instantiate`
  took from the `Instantiation`, such as the resource path. Should `Instance` keep the
  context and call `instantiate` again instead?
