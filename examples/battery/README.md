# battery: fmite's showcase FMU

A battery pack of four cells in series, as an FMI 3.0 Co-Simulation FMU. The model is
one struct, and every fact in its `modelDescription.xml` comes from a field's type:

| Field                                               | What the type says                                                    |
| --------------------------------------------------- | --------------------------------------------------------------------- |
| `current: Input<f64, Ampere>`                       | a shipped unit; an input, so model code cannot write it               |
| `ambient: Input<f64, Celsius>`                      | a unit with an offset                                                 |
| `capacity: Parameter<f64, AmpereHour>`              | a shipped scaled unit; fixed, so set before Step Mode only            |
| `resistance: Parameter<f64, MilliOhm, Tunable>`     | a **custom** unit, `Unit::ohm().scaled(1e-3)`; tunable in Step Mode   |
| `energy: CalculatedParameter<f64, WattHour>`        | computed from `capacity` in `exit_initialization`                     |
| `soc: Output<f64, Percent, Discrete, Exact>`        | a **custom** dimensionless unit; `Exact`, so the importer may set it  |
| `cells: Output<[f64; 4], Volt>`                     | an array, and its `<Dimension>`                                       |
| `heat_rate: Output<f64, KelvinPerSecond, Discrete>` | a **custom** composed unit, `Unit::kelvin().per(Unit::second())`      |
| `mode: Output<Mode>`                                | an `Enumeration`; an importer cannot set a value with no item         |
| `charges: Output<u32>`                              | an integer, discrete by default                                       |
| `charge: f64`                                       | a plain field: private state, not a variable                          |

The start values come from `Battery::default()`. `#[derive(Variables)]` numbers the
variables in field order. `tests/derive.rs` holds the hand-written list it replaces and
checks that the two are equal.

## Build and package

```sh
cargo build -p battery
cargo run -p battery --bin package -- battery.fmu
```

The first builds the `cdylib`, whose `fmi3…` symbols come from
`fmite::export!(Battery: CoSimulation + State)`. The second links the model as an rlib,
writes the model description from its type, and zips both:

```text
modelDescription.xml
binaries/aarch64-darwin/battery.dylib
```

## The model description

```xml
<?xml version="1.0" encoding="UTF-8"?>
<fmiModelDescription fmiVersion="3.0" modelName="battery" instantiationToken="{00621286-f35f-23e8-880a-9b7fb9b414db}" description="A four-cell battery pack" generationTool="fmite 0.1.0" variableNamingConvention="structured">
  <CoSimulation modelIdentifier="battery" canGetAndSetFMUState="true" fixedInternalStepSize="0.1" canHandleVariableCommunicationStepSize="true"/>
  <UnitDefinitions>
    <Unit name="A">
      <BaseUnit A="1"/>
    </Unit>
    <Unit name="degC">
      <BaseUnit K="1" offset="273.15"/>
    </Unit>
    <Unit name="Ah">
      <BaseUnit s="1" A="1" factor="3600"/>
    </Unit>
    <Unit name="mOhm">
      <BaseUnit kg="1" m="2" s="-3" A="-2" factor="0.001"/>
    </Unit>
    <Unit name="Wh">
      <BaseUnit kg="1" m="2" s="-2" factor="3600"/>
    </Unit>
    <Unit name="%">
      <BaseUnit factor="0.01"/>
    </Unit>
    <Unit name="V">
      <BaseUnit kg="1" m="2" s="-3" A="-1"/>
    </Unit>
    <Unit name="K/s">
      <BaseUnit s="-1" K="1"/>
    </Unit>
  </UnitDefinitions>
  <TypeDefinitions>
    <EnumerationType name="Mode">
      <Item name="Idle" value="1"/>
      <Item name="Charging" value="2"/>
      <Item name="Discharging" value="3"/>
    </EnumerationType>
  </TypeDefinitions>
  <LogCategories>
    <Category name="logStatusError" description="A refused call, and why"/>
    <Category name="logStatusFatal" description="A panic inside the FMU; the instance takes no more calls"/>
  </LogCategories>
  <DefaultExperiment startTime="0" stopTime="600" stepSize="0.1"/>
  <ModelVariables>
    <Float64 name="time" valueReference="0" causality="independent" variability="continuous"/>
    <Float64 name="current" valueReference="1" causality="input" variability="continuous" unit="A" start="0"/>
    <Float64 name="ambient" valueReference="2" causality="input" variability="continuous" unit="degC" start="25"/>
    <Float64 name="capacity" valueReference="3" causality="parameter" variability="fixed" unit="Ah" start="50"/>
    <Float64 name="resistance" valueReference="4" causality="parameter" variability="tunable" unit="mOhm" start="2"/>
    <Float64 name="energy" valueReference="5" causality="calculatedParameter" variability="fixed" unit="Wh"/>
    <Float64 name="soc" valueReference="6" causality="output" variability="discrete" initial="exact" unit="%" start="100"/>
    <Float64 name="cells" valueReference="7" causality="output" variability="continuous" unit="V">
      <Dimension start="4"/>
    </Float64>
    <Float64 name="temperature" valueReference="8" causality="output" variability="discrete" unit="degC"/>
    <Float64 name="heat_rate" valueReference="9" causality="output" variability="discrete" unit="K/s"/>
    <Enumeration name="mode" valueReference="10" declaredType="Mode" causality="output" variability="discrete"/>
    <UInt32 name="charges" valueReference="11" causality="output" variability="discrete"/>
  </ModelVariables>
  <ModelStructure>
    <Output valueReference="6" dependencies=""/>
    <Output valueReference="7" dependencies=""/>
    <Output valueReference="8" dependencies=""/>
    <Output valueReference="9" dependencies=""/>
    <Output valueReference="10" dependencies=""/>
    <Output valueReference="11" dependencies=""/>
    <InitialUnknown valueReference="5" dependencies="1 2 3 4 6"/>
    <InitialUnknown valueReference="7" dependencies="1 2 3 4 6"/>
    <InitialUnknown valueReference="8" dependencies="1 2 3 4 6"/>
    <InitialUnknown valueReference="9" dependencies="1 2 3 4 6"/>
    <InitialUnknown valueReference="10" dependencies="1 2 3 4 6"/>
    <InitialUnknown valueReference="11" dependencies="1 2 3 4 6"/>
  </ModelStructure>
</fmiModelDescription>
```

## Tests

- `tests/battery.rs` drives the model through `fmite::Instance` and validates the
  description against the standard's XSD, vendored in `../../tests/schema`.
- `tests/abi.rs` links `../../tests/abi.c`, which names all 75 functions of the
  standard's headers with their header types, against the `cdylib`.
- `tests/derive.rs` checks the derives against the hand-written impls.
