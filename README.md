# fmite

[FMI 3.0](https://fmi-standard.org/docs/3.0.1/) export for Rust. fmite aims at
the whole standard: Co-Simulation, Model Exchange and Scheduled Execution. A
model is a struct whose field types say what each variable is; fmite writes the
75 `fmi3*` C functions, `modelDescription.xml` and the `.fmu` archive from it,
and checks every importer call before the model sees it. The rules of the
standard's variable table are trait bounds, so an ill-formed model does not
compile.

```rust
use fmite::unit::{Celsius, Watt};
use fmite::{CoSimulation, Error, Fmu, Input, Output, Parameter, Step, StepResult, Tunable};

#[derive(Clone, Default, fmite::Variables)]
pub struct Heater {
    pub setpoint: Input<f64, Celsius>,
    pub temperature: Input<f64, Celsius>,
    pub gain: Parameter<f64, (), Tunable>,
    pub power: Output<f64, Watt>,
}

impl Fmu for Heater {}

impl CoSimulation for Heater {
    fn do_step(&mut self, _: Step) -> Result<StepResult, Error> {
        *self.power = (*self.gain * (*self.setpoint - *self.temperature)).max(0.0);
        Ok(StepResult::Complete)
    }
}

fmite::export!(Heater: CoSimulation + State);
```

`cargo build` makes the `cdylib`; `fmite::package::<Heater>` zips it with the model
description it writes from the type. The [walkthrough](docs/walkthrough.md), which is
also the crate documentation, shows every step, the generated output, and the proofs.

## What the compiler checks

Each is a `compile_fail` test in the walkthrough.

- FMI 3.0 Table 22: an illegal causality, variability or initial combination is a
  type error that cites the table.
- Only floats are `Continuous`; only floats carry a unit; an enumeration is discrete.
- Model code cannot write an `Input`, a `Parameter` or a constant `Output`: no
  `DerefMut`.
- A capability in `export!` is a bound: `State` needs `Clone`, so
  `canGetAndSetFMUState` is never claimed by a model that cannot.
- A duplicate value reference, a reference of 0, a duplicate name, or two units of one
  name fail in a `const`, before an FMU exists.
- A clock is a type, `Clock<P>` over a `Periodic` marker. A clocked variable names its
  clock as a type parameter and must be discrete. A variable on an undeclared clock,
  two clocks with one schedule, a Scheduled Execution FMU without a clock, and a
  Co-Simulation FMU with one all fail to compile.
- Start values come from `Default` and nowhere else; array dimensions from the type;
  enumeration items from the enum.

## What fmite provides

- **No build script, no bindgen.** The ABI is transcribed from the 3.0.1 headers and a
  C test links every symbol against them.
- **Unsafe only at the boundary**, in `log` and `export!`; every pointer is checked
  before a slice is made from it. Your crate keeps `#![deny(unsafe_code)]`.
- **No panic reaches the importer.** Every export runs in `catch_unwind`; a panic is
  `fmi3Fatal` and a log message. An unsupported function answers `fmi3Error` and logs
  why; nothing is `todo!()`.
- **The state machine is enforced.** A call in the wrong mode, an unknown reference, a
  wrong type, a `set` the variable's row forbids, a clock activated out of order:
  refused and logged before your code runs.
- **Preemption is sound.** Under Scheduled Execution every call holds the importer's
  preemption lock, so a partition never runs while another one is running.
- **Zero dependencies by default.** `derive` and `package` are features; the archive
  writer is a function, callable from a code generator.

## Features

| Status          | What                                                                                                                                                                                                                                                                     |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Implemented** | Co-Simulation: instantiate, initialization, `doStep` with a fixed internal step or a variable one, terminate, reset. Scheduled Execution: periodic input clocks with a constant interval and a priority, clocked variables, `activateModelPartition`, the preemption lock. All scalar types, `Boolean`, `Enumeration`, fixed-size arrays. FMU state get/set/free (`State`). The full `modelDescription.xml`: units, enumeration types, log categories, default experiment, model structure with explicit dependencies. Packaging. `#[derive(Variables, Enumeration)]`. |
| **Planned**     | Model Exchange. Output, countdown, triggered and tunable clocks; intervals as fractions; preemptible partitions; clocks under Co-Simulation. `String` and `Binary` variables. FMU state serialization. Finer `<ModelStructure>` dependencies. Directional and adjoint derivatives. Event mode and early return. Annotations and `terminalsAndIcons`. Import. |
| **Not planned** | FMI 2.0. Layered standards.                                                                                                                                                                                                                                              |

A function in the planned or unplanned rows is still exported, and answers
`fmi3Error` with a log message.

## Verification

`cargo test --all-features` runs the unit tests, the walkthrough's doctests, a C test
that assigns all 75 exported symbols to the official headers' function-pointer types
(skipped without a C compiler), and an `xmllint` validation of every generated model
description against the official XSD (skipped without `xmllint`). The headers and
schema are vendored under `tests/`, with their licenses.
[`examples/battery`](examples/battery) is the packaged showcase FMU; it has been run
in an independent importer by hand. [`examples/cruise`](examples/cruise) is a
two-rate Scheduled Execution FMU, driven through its exported symbols by its tests.

## License

Apache-2.0 OR MIT, at your option. Any contribution intentionally submitted for
inclusion is dual-licensed the same way, with no additional terms.
