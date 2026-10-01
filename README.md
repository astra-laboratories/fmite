# fmite

[FMI 3.0](https://fmi-standard.org/docs/3.0.1/) in Rust: write a model as a safe
trait, get a conformant FMU.

fmite starts with the FMU side of **Co-Simulation**, where the model carries its own
solver and the importer only chooses communication steps. A fixed-step, discrete-time
model is exactly that, so this is where fmite starts.

- **No build script, no bindgen, no libclang.** The C API is written out by hand from
  the 3.0.1 headers, and a test checks every signature against them.
- **Unsafe code stays at the boundary.** Only the `abi` module and the `export!` macro
  use it. Every pointer is null-checked before a slice is made from it.
- **A panic never reaches the importer.** Every exported function runs inside
  `catch_unwind`. A feature the FMU does not support answers `fmi3Error` and logs
  why; it never calls `todo!()`.
- **fmite tracks FMI's mode state machine.** A call made in the wrong mode is refused
  before your code sees it.
- **The default build has no dependencies.** Packaging an FMU is an optional feature
  and a library call, not a separate binary.

> Status: design. Nothing below is implemented yet. [`docs/design.md`](docs/design.md)
> holds the proposed layers and traits for review.

## Feature matrix

The checklist below follows the FMI 3.0.1 headers (`fmi3FunctionTypes.h`), function by
function.

- **v0.1**: planned for the first release.
- **later**: on the roadmap.
- **—**: not planned. fmite still exports the symbol, and it answers `fmi3Error` with a
  log message.

### Common functions

| Function                                                    | Status |
| ----------------------------------------------------------- | ------ |
| `fmi3GetVersion`                                            | v0.1   |
| `fmi3SetDebugLogging`                                       | v0.1   |
| `fmi3FreeInstance`                                          | v0.1   |
| `fmi3EnterInitializationMode`, `fmi3ExitInitializationMode` | v0.1   |
| `fmi3Terminate`                                             | v0.1   |
| `fmi3Reset`                                                 | v0.1   |
| `fmi3EnterEventMode`                                        | later  |
| `fmi3EnterConfigurationMode`, `fmi3ExitConfigurationMode`   | later  |
| `fmi3GetNumberOfVariableDependencies`                       | later  |
| `fmi3GetVariableDependencies`                               | later  |
| `fmi3GetDirectionalDerivative`, `fmi3GetAdjointDerivative`  | —      |
| `fmi3EvaluateDiscreteStates`                                | later  |
| `fmi3UpdateDiscreteStates`                                  | later  |

The two derivative functions exist for importers that compute Jacobians. A discrete
model has no derivatives to give them.

### Getting and setting values

| Function                                                    | Status |
| ----------------------------------------------------------- | ------ |
| `fmi3Get/SetFloat32`, `fmi3Get/SetFloat64`                  | v0.1   |
| `fmi3Get/SetInt8`, `UInt8`, `Int16`, `UInt16`               | v0.1   |
| `fmi3Get/SetInt32`, `UInt32`, `Int64`, `UInt64`             | v0.1   |
| `fmi3Get/SetBoolean`                                        | v0.1   |
| `fmi3Get/SetString`                                         | v0.1   |
| `fmi3Get/SetBinary`                                         | v0.1   |
| `fmi3Get/SetClock`                                          | later  |
| `fmi3Get/SetIntervalDecimal`, `fmi3Get/SetIntervalFraction` | later  |
| `fmi3Get/SetShiftDecimal`, `fmi3Get/SetShiftFraction`       | later  |

Arrays with fixed `<Dimension>`s are in v0.1. Arrays sized by a structural parameter,
which change in configuration mode, come later.

### FMU state

| Function                                                 | Status |
| -------------------------------------------------------- | ------ |
| `fmi3GetFMUState`, `fmi3SetFMUState`, `fmi3FreeFMUState` | v0.1   |
| `fmi3SerializedFMUStateSize`, `fmi3SerializeFMUState`    | v0.1   |
| `fmi3DeserializeFMUState`                                | v0.1   |

These are available when the model implements `State`. The capability flags in
`modelDescription.xml` follow from whether it does.

### Co-Simulation

| Function or capability                                            | Status |
| ----------------------------------------------------------------- | ------ |
| `fmi3InstantiateCoSimulation`                                     | v0.1   |
| `fmi3DoStep`                                                      | v0.1   |
| `fixedInternalStepSize`, `canHandleVariableCommunicationStepSize` | v0.1   |
| `fmi3EnterStepMode` (event mode)                                  | later  |
| Early return, intermediate update callback                        | later  |
| `fmi3GetOutputDerivatives`                                        | —      |

### Scheduled Execution

| Function                                   | Status |
| ------------------------------------------ | ------ |
| `fmi3InstantiateScheduledExecution`        | later  |
| `fmi3ActivateModelPartition`               | later  |
| Clock update and preemption-lock callbacks | later  |

In Scheduled Execution the importer activates clocked model partitions, so a
synchronous, multi-rate model fits it directly. It is the next interface after
Co-Simulation.

### Model Exchange

| Function                                                                 | Status |
| ------------------------------------------------------------------------ | ------ |
| `fmi3InstantiateModelExchange`                                           | —      |
| `fmi3EnterContinuousTimeMode`, `fmi3CompletedIntegratorStep`             | —      |
| `fmi3SetTime`, `fmi3SetContinuousStates`, `fmi3GetContinuousStates`      | —      |
| `fmi3GetContinuousStateDerivatives`, `fmi3GetNominalsOfContinuousStates` | —      |
| `fmi3GetEventIndicators`, `fmi3GetNumberOfEventIndicators`               | —      |
| `fmi3GetNumberOfContinuousStates`                                        | —      |

Model Exchange hands the model's derivatives to the importer's solver. fmite targets
models that bring their own solver, so Model Exchange is not planned.

### `modelDescription.xml`

| Element                                                             | Status |
| ------------------------------------------------------------------- | ------ |
| `fmiModelDescription` attributes, `CoSimulation` capability flags   | v0.1   |
| `ModelVariables`, all types, causality, variability, initial, start | v0.1   |
| `<Dimension>`, fixed sizes                                          | v0.1   |
| `TypeDefinitions`, including `EnumerationType`                      | v0.1   |
| `UnitDefinitions`                                                   | v0.1   |
| `LogCategories`                                                     | v0.1   |
| `ModelStructure` with explicit `dependencies`                       | v0.1   |
| `DefaultExperiment`                                                 | v0.1   |
| `Annotations`                                                       | later  |
| `terminalsAndIcons/`                                                | later  |
| `buildDescription.xml`, source FMUs                                 | —      |

### Packaging and beyond

| Item                                                                                     | Status |
| ---------------------------------------------------------------------------------------- | ------ |
| FMU archive: `binaries/<arch>-<os>/`, `documentation/`, `resources/` (`package` feature) | v0.1   |
| Import: load an FMU and drive it                                                         | later  |
| FMI 2.0                                                                                  | —      |
| Layered standards (bus communication, references)                                        | —      |

## Why another FMI crate

The existing Rust FMI implementation is the rust-fmi workspace: `fmi`, `fmi-export`,
`fmi-sys` and `fmi-schema`. It covers more ground than fmite plans to, including
FMI 2.0, importing and most of Model Exchange. We built a Co-Simulation export on
`fmi-export` and shipped it. These are the problems we found, pinned to the versions
we read: `fmi` 0.8.0, `fmi-export` 0.3.0 and `fmi-sys` 0.6.0, from a single workspace
commit.

**Unimplemented exports abort the importer.** Eighteen exported functions have
`todo!()` as their body:

- FMU state, all six functions (`fmi3/traits/wrappers.rs:555–600`).
- Directional and adjoint derivatives (`:617`, `:633`).
- Clock intervals and shifts, all eight functions (`:671–753`).
- `fmi3EvaluateDiscreteStates` (`:761`).
- `fmi3InstantiateScheduledExecution` (`:412`).

Nothing in the workspace calls `catch_unwind`, so a panic inside an `extern "C"`
function aborts the importer's whole process. Other panics are reachable from normal
input too: `CString::new(..).unwrap()` (`fmi3/traits/model_get_set.rs:187`) and
unchecked `values[0]` indexing (`:49`, `:62`, `:200`, `:252`, `:267`).

**Some exports use the wrong calling convention.** Four of them are declared without
`extern "C"`, so a C importer calling them is undefined behaviour:

- `fmi3SerializedFMUStateSize` (`fmi3/export.rs:456`)
- `fmi3SerializeFMUState` (`:467`)
- `fmi3DeserializeFMUState` (`:482`)
- `fmi3GetIntervalDecimal` (`:567`)

The instantiation token is exported as a Rust `&'static str` (`:84`), which is not a
C string.

**Unsafe code is spread beyond the boundary.** `fmi-export` has 240 lines containing
`unsafe`. 118 of them are in the export macro and 119 are in default trait methods
that every model inherits (`fmi3/traits/wrappers.rs`). Those methods make 29
`slice::from_raw_parts` calls and none checks for null. The standard allows an
importer to pass `NULL` with a count of zero, and `from_raw_parts` on a null pointer
is undefined behaviour (`:125`, `:151`).

**Every build needs libclang.** `fmi-sys` runs bindgen in its build script, with no
pre-generated bindings to fall back on. `fmi` turns on all of `fmi-sys`'s default
features even for an FMI 3-only user. That builds FMI 2.0 bindings, a C logger
compiled with `cc`, and the bus-communication bindings.

**There are no enumerations.** The exporter's variable builder has no Enumeration
type (`fmi3/variable_builder.rs:375–536`). An exporter on top of it has to refuse any
model with a discrete mode.

**Output dependencies are wrong by default.** An output that is not a derivative gets
`dependencies` omitted, which FMI reads as "depends on every known"
(`fmi-export-derive …/model_impl/metadata.rs:239–256`). At runtime
`fmi3GetVariableDependencies` always answers with an empty list
(`fmi3/instance/common.rs:218–231`).

**The dependency weight is high.** For us, turning the feature on took the dependency
tree from 20 crates to 205 (246 with build dependencies). `fmi` depends on `zip` with
its default features on. Cargo then enables bzip2, deflate64, zopfli and AES in every
crate that uses `zip`, which brings in a licence exception we had no other reason to
grant.

**Packaging cannot be called from code.** `cargo-fmi` is a binary, not a library, so a
tool that generates FMUs cannot call its packaging.

Here is what fmite does about each:

- It has no build script.
- Unsafe code lives only in `abi` and `export!`, and every pointer is checked.
- `catch_unwind` wraps every export.
- Unsupported functions answer `fmi3Error` and log why.
- Enumerations and explicit dependencies are in v0.1.
- The default build has zero dependencies.
- Packaging is a library function.

## Roadmap

| Milestone | Contents                                                                                      |
| --------- | --------------------------------------------------------------------------------------------- |
| M0        | Licences, CI: `cargo fmt`, `clippy -D warnings`, tests on Linux, macOS and Windows            |
| M1        | `abi`: types and all 75 signatures, plus a test that checks them against the official headers |
| M2        | `Instance` (mode state machine, logging), `CoSimulation`, `export!`, an example FMU           |
| M3        | `description`: the `modelDescription.xml` writer, validated in CI against the official XSD    |
| M4        | `package` feature; CI runs the example FMU through the standard's reference simulator         |
| M5        | `State` (FMU state and serialization) and enumerations                                        |
| M6        | Magnetite switches its FMU export from `fmi-export` to fmite                                  |
| Later     | Clocks and Scheduled Execution, event mode, early return, import, a crates.io release         |

## Conformance

fmite checks itself against the standard's own artifacts rather than against its own
reading of the standard:

- The C signatures are checked against the official headers. A dev-only C test, using
  the headers vendored under BSD-2-Clause, assigns each exported symbol to the
  header's function-pointer type.
- Generated `modelDescription.xml` files are validated against the official XSD
  schema.
- The example FMUs are run in the standard's open-source reference simulator on every
  CI platform.

## Licence

Licensed under either the [Apache License, Version 2.0](LICENSE-APACHE) or the
[MIT licence](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution you intentionally submit for
inclusion in fmite, as defined in the Apache-2.0 licence, is dual-licensed as above,
with no additional terms or conditions.
