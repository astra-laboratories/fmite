//! `modelDescription.xml`, written from the model's types. Nothing in it is set by hand:
//! the variables and their attributes come from `Variables`, the units and enumeration
//! types from the variables' types, the interface element from `CoSimulation` and the
//! export list, and the start values from `T::default()`.

use core::fmt::Write as _;

use crate::export::Exported;
use crate::unit::Unit;
use crate::{CoSimulation, Error, Fmu, Kind, LogCategory, ValueReference, ValuesMut, Variable};

/// The `modelIdentifier`: the model name with `-` as `_`, which is also the file name
/// Cargo gives the `cdylib`.
#[must_use]
pub fn model_identifier<T: Fmu>() -> String {
    T::MODEL_NAME.replace('-', "_")
}

/// The model description of `T`.
///
/// # Errors
///
/// When `T::default()` cannot give a start value its `VARIABLES` promise.
pub fn model_description<T: CoSimulation + Exported>() -> Result<String, Error> {
    let mut xml = Xml::default();
    xml.out
        .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let mut root = vec![
        ("fmiVersion", "3.0".to_owned()),
        ("modelName", T::MODEL_NAME.to_owned()),
        ("instantiationToken", T::INSTANTIATION_TOKEN.to_owned()),
    ];
    if let Some(description) = T::DESCRIPTION {
        root.push(("description", description.to_owned()));
    }
    root.push((
        "generationTool",
        concat!("fmite ", env!("CARGO_PKG_VERSION")).to_owned(),
    ));
    root.push(("variableNamingConvention", "structured".to_owned()));
    xml.open("fmiModelDescription", &root);

    let mut co_simulation = vec![("modelIdentifier", model_identifier::<T>())];
    if T::CAPABILITIES.state {
        co_simulation.push(("canGetAndSetFMUState", "true".to_owned()));
    }
    if let Some(step) = T::FIXED_INTERNAL_STEP_SIZE {
        co_simulation.push(("fixedInternalStepSize", number(step)));
    }
    co_simulation.push(("canHandleVariableCommunicationStepSize", "true".to_owned()));
    xml.empty("CoSimulation", &co_simulation);

    unit_definitions(&mut xml, T::VARIABLES);
    type_definitions(&mut xml, T::VARIABLES);

    let categories = <T::Log as LogCategory>::CATEGORIES;
    if !categories.is_empty() {
        xml.open("LogCategories", &[]);
        for (name, description) in categories {
            xml.empty(
                "Category",
                &[
                    ("name", (*name).to_owned()),
                    ("description", (*description).to_owned()),
                ],
            );
        }
        xml.close("LogCategories");
    }

    let experiment = T::DEFAULT_EXPERIMENT;
    let experiment: Vec<_> = [
        ("startTime", experiment.start),
        ("stopTime", experiment.stop),
        ("tolerance", experiment.tolerance),
        ("stepSize", experiment.step),
    ]
    .into_iter()
    .filter_map(|(name, value)| Some((name, number(value?))))
    .collect();
    if !experiment.is_empty() {
        xml.empty("DefaultExperiment", &experiment);
    }

    xml.open("ModelVariables", &[]);
    xml.empty(
        "Float64",
        &[
            ("name", "time".to_owned()),
            ("valueReference", "0".to_owned()),
            ("causality", "independent".to_owned()),
            ("variability", "continuous".to_owned()),
        ],
    );
    let model = T::default();
    for variable in T::VARIABLES {
        model_variable(&mut xml, &model, variable)?;
    }
    xml.close("ModelVariables");

    model_structure(&mut xml, T::VARIABLES);
    xml.close("fmiModelDescription");
    Ok(xml.out)
}

/// One `<Unit>` per distinct unit name, in order of first use. `check` has refused two
/// definitions of one name.
fn unit_definitions(xml: &mut Xml, variables: &[Variable]) {
    let mut units: Vec<&(&str, Unit)> = Vec::new();
    for unit in variables.iter().filter_map(|variable| variable.unit) {
        if !units.iter().any(|seen| seen.0 == unit.0) {
            units.push(unit);
        }
    }
    if units.is_empty() {
        return;
    }
    xml.open("UnitDefinitions", &[]);
    for (name, unit) in units {
        xml.open("Unit", &[("name", (*name).to_owned())]);
        xml.empty("BaseUnit", &base_unit(unit));
        xml.close("Unit");
    }
    xml.close("UnitDefinitions");
}

/// `<BaseUnit>`'s attributes, each left out at its default: 0 for an exponent, 1 for
/// the factor, 0 for the offset.
fn base_unit(unit: &Unit) -> Vec<(&'static str, String)> {
    let exponents = [
        ("kg", unit.kilogram),
        ("m", unit.meter),
        ("s", unit.second),
        ("A", unit.ampere),
        ("K", unit.kelvin),
        ("mol", unit.mole),
        ("cd", unit.candela),
        ("rad", unit.radian),
    ];
    let mut attributes: Vec<_> = (exponents.into_iter())
        .filter(|(_, exponent)| *exponent != 0)
        .map(|(name, exponent)| (name, exponent.to_string()))
        .collect();
    if unit.factor.to_bits() != 1.0_f64.to_bits() {
        attributes.push(("factor", number(unit.factor)));
    }
    if unit.offset.to_bits() != 0.0_f64.to_bits() {
        attributes.push(("offset", number(unit.offset)));
    }
    attributes
}

/// One `<EnumerationType>` per distinct enumeration.
fn type_definitions(xml: &mut Xml, variables: &[Variable]) {
    let mut types = Vec::new();
    for enumeration in variables.iter().filter_map(|variable| variable.enumeration) {
        if !types
            .iter()
            .any(|seen: &&crate::EnumerationType| seen.name == enumeration.name)
        {
            types.push(enumeration);
        }
    }
    if types.is_empty() {
        return;
    }
    xml.open("TypeDefinitions", &[]);
    for enumeration in types {
        xml.open("EnumerationType", &[("name", enumeration.name.to_owned())]);
        for (name, value) in enumeration.items {
            xml.empty(
                "Item",
                &[("name", (*name).to_owned()), ("value", value.to_string())],
            );
        }
        xml.close("EnumerationType");
    }
    xml.close("TypeDefinitions");
}

fn model_variable<T: Fmu>(xml: &mut Xml, model: &T, variable: &Variable) -> Result<(), Error> {
    let mut attributes = vec![
        ("name", variable.name.to_owned()),
        ("valueReference", variable.value_reference.to_string()),
    ];
    if let Some(enumeration) = variable.enumeration {
        attributes.push(("declaredType", enumeration.name.to_owned()));
    }
    attributes.push(("causality", variable.causality.to_owned()));
    attributes.push(("variability", variable.variability.to_owned()));
    if variable.initial != variable.default_initial {
        attributes.push(("initial", variable.initial.to_owned()));
    }
    if let Some((unit, _)) = variable.unit {
        attributes.push(("unit", (*unit).to_owned()));
    }
    if variable.has_start {
        attributes.push(("start", start(model, variable)?));
    }
    let element = variable.kind.name();
    let dims = variable.dims.as_slice();
    if dims.is_empty() {
        xml.empty(element, &attributes);
    } else {
        xml.open(element, &attributes);
        for size in dims {
            xml.empty("Dimension", &[("start", size.to_string())]);
        }
        xml.close(element);
    }
    Ok(())
}

/// The variable's value in `model`, as the `start` attribute writes it: a list,
/// row-major, for an array.
fn start<T: Fmu>(model: &T, variable: &Variable) -> Result<String, Error> {
    let vr = ValueReference(variable.value_reference);
    let count = variable.dims.count();
    macro_rules! read {
        ($variant:ident, $zero:expr, $format:expr) => {{
            let mut buffer = vec![$zero; count];
            model.get(vr, ValuesMut::$variant(&mut buffer))?;
            buffer.into_iter().map($format).collect::<Vec<String>>()
        }};
    }
    let values = match variable.kind {
        Kind::Float32 => read!(Float32, 0.0, |x: f32| number(f64::from(x))),
        Kind::Float64 => read!(Float64, 0.0, number),
        Kind::Int8 => read!(Int8, 0, |x: i8| x.to_string()),
        Kind::UInt8 => read!(UInt8, 0, |x: u8| x.to_string()),
        Kind::Int16 => read!(Int16, 0, |x: i16| x.to_string()),
        Kind::UInt16 => read!(UInt16, 0, |x: u16| x.to_string()),
        Kind::Int32 => read!(Int32, 0, |x: i32| x.to_string()),
        Kind::UInt32 => read!(UInt32, 0, |x: u32| x.to_string()),
        Kind::Int64 | Kind::Enumeration => read!(Int64, 0, |x: i64| x.to_string()),
        Kind::UInt64 => read!(UInt64, 0, |x: u64| x.to_string()),
        Kind::Boolean => read!(Boolean, false, |x: bool| x.to_string()),
    };
    Ok(values.join(" "))
}

/// A float as `xs:double` writes it: the shortest text that reads back exactly.
fn number(x: f64) -> String {
    if x.is_nan() {
        "NaN".to_owned()
    } else if x.is_infinite() {
        if x > 0.0 { "INF" } else { "-INF" }.to_owned()
    } else {
        x.to_string()
    }
}

/// `<ModelStructure>`, every `dependencies` written out. An output depends on no input:
/// model code writes an output only in a hook or a step, never in a `set`, so an input
/// set at a communication point reaches no output before the next `fmi3DoStep`. There
/// is no direct feedthrough, and an importer can close a loop through the FMU without
/// an algebraic one. An initial unknown depends on every variable the importer may set
/// during initialization: the hooks read whichever they like, so that is the coarsest
/// truthful answer, and a finer one needs the model to say which feed it.
fn model_structure(xml: &mut Xml, variables: &[Variable]) {
    let list = |keep: fn(&Variable) -> bool| {
        (variables.iter())
            .filter(|variable| keep(variable))
            .map(|variable| variable.value_reference.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let knowns = list(|v| v.settable_in_initialization);
    xml.open("ModelStructure", &[]);
    for output in variables.iter().filter(|v| v.causality == "output") {
        xml.empty(
            "Output",
            &[
                ("valueReference", output.value_reference.to_string()),
                ("dependencies", String::new()),
            ],
        );
    }
    let initial_unknowns = variables.iter().filter(|v| {
        (v.causality == "output" && !v.has_start) || v.causality == "calculatedParameter"
    });
    for unknown in initial_unknowns {
        xml.empty(
            "InitialUnknown",
            &[
                ("valueReference", unknown.value_reference.to_string()),
                ("dependencies", knowns.clone()),
            ],
        );
    }
    xml.close("ModelStructure");
}

/// An indenting XML writer, enough for a model description.
#[derive(Default)]
struct Xml {
    out: String,
    depth: usize,
}

impl Xml {
    fn start(&mut self, name: &str, attributes: &[(&str, String)]) {
        let indent = "  ".repeat(self.depth);
        let _ = write!(self.out, "{indent}<{name}");
        for (key, value) in attributes {
            let _ = write!(self.out, " {key}=\"{}\"", escape(value));
        }
    }

    fn open(&mut self, name: &str, attributes: &[(&str, String)]) {
        self.start(name, attributes);
        self.out.push_str(">\n");
        self.depth += 1;
    }

    fn empty(&mut self, name: &str, attributes: &[(&str, String)]) {
        self.start(name, attributes);
        self.out.push_str("/>\n");
    }

    fn close(&mut self, name: &str) {
        self.depth -= 1;
        let indent = "  ".repeat(self.depth);
        let _ = writeln!(self.out, "{indent}</{name}>");
    }
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            c => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_model::Gain;

    const EXPECTED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<fmiModelDescription fmiVersion="3.0" modelName="gain-test" instantiationToken="{gain}" description="y = 2k·u" generationTool="fmite 0.1.0" variableNamingConvention="structured">
  <CoSimulation modelIdentifier="gain_test" canGetAndSetFMUState="true" fixedInternalStepSize="0.1" canHandleVariableCommunicationStepSize="true"/>
  <UnitDefinitions>
    <Unit name="V">
      <BaseUnit kg="1" m="2" s="-3" A="-1"/>
    </Unit>
  </UnitDefinitions>
  <TypeDefinitions>
    <EnumerationType name="Sign">
      <Item name="Zero" value="1"/>
      <Item name="Positive" value="2"/>
    </EnumerationType>
  </TypeDefinitions>
  <LogCategories>
    <Category name="logStatusError" description="Refused calls &amp; why"/>
  </LogCategories>
  <DefaultExperiment stopTime="1"/>
  <ModelVariables>
    <Float64 name="time" valueReference="0" causality="independent" variability="continuous"/>
    <Float64 name="u" valueReference="1" causality="input" variability="continuous" unit="V" start="0"/>
    <Float64 name="k" valueReference="2" causality="parameter" variability="tunable" start="0"/>
    <Float64 name="twice_k" valueReference="3" causality="calculatedParameter" variability="tunable"/>
    <Float64 name="y" valueReference="4" causality="output" variability="continuous" unit="V">
      <Dimension start="2"/>
    </Float64>
    <UInt32 name="steps" valueReference="5" causality="output" variability="discrete"/>
    <Enumeration name="sign" valueReference="6" declaredType="Sign" causality="output" variability="discrete"/>
  </ModelVariables>
  <ModelStructure>
    <Output valueReference="4" dependencies=""/>
    <Output valueReference="5" dependencies=""/>
    <Output valueReference="6" dependencies=""/>
    <InitialUnknown valueReference="3" dependencies="1 2"/>
    <InitialUnknown valueReference="4" dependencies="1 2"/>
    <InitialUnknown valueReference="5" dependencies="1 2"/>
    <InitialUnknown valueReference="6" dependencies="1 2"/>
  </ModelStructure>
</fmiModelDescription>
"#;

    #[test]
    fn the_description_is_written_from_the_types() {
        assert_eq!(model_description::<Gain>().unwrap(), EXPECTED);
    }

    #[test]
    fn the_description_is_valid_against_the_official_schema() {
        let schema = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/schema/fmi3ModelDescription.xsd"
        );
        let dir = std::env::temp_dir().join(format!("fmite-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("modelDescription.xml");
        std::fs::write(&file, EXPECTED).unwrap();
        let Ok(output) = std::process::Command::new("xmllint")
            .args(["--noout", "--schema", schema])
            .arg(&file)
            .output()
        else {
            eprintln!("xmllint not found; schema check skipped");
            return;
        };
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn floats_are_written_as_xs_double() {
        assert_eq!(number(0.1), "0.1");
        assert_eq!(number(273.15), "273.15");
        assert_eq!(number(f64::INFINITY), "INF");
        assert_eq!(number(f64::NEG_INFINITY), "-INF");
        assert_eq!(number(f64::NAN), "NaN");
    }
}
