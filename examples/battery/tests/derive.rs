//! `#[derive(Variables)]` writes the list a hand-written impl would. This is the list
//! `Battery` declared by hand before it derived it.

use battery::{Battery, KelvinPerSecond, MilliOhm, Mode, Percent};
use fmite::unit::{Ampere, AmpereHour, Celsius, Volt, WattHour};
use fmite::{
    CalculatedParameter, Discrete, Enumeration, Exact, Input, Output, Parameter, Tunable, Variable,
    Variables,
};

const HAND_WRITTEN: &[Variable] = &[
    Variable::new::<Input<f64, Ampere>>("current", 1),
    Variable::new::<Input<f64, Celsius>>("ambient", 2),
    Variable::new::<Parameter<f64, AmpereHour>>("capacity", 3),
    Variable::new::<Parameter<f64, MilliOhm, Tunable>>("resistance", 4),
    Variable::new::<CalculatedParameter<f64, WattHour>>("energy", 5),
    Variable::new::<Output<f64, Percent, Discrete, Exact>>("soc", 6),
    Variable::new::<Output<[f64; 4], Volt>>("cells", 7),
    Variable::new::<Output<f64, Celsius, Discrete>>("temperature", 8),
    Variable::new::<Output<f64, KelvinPerSecond, Discrete>>("heat_rate", 9),
    Variable::new::<Output<Mode>>("mode", 10),
    Variable::new::<Output<u32>>("charges", 11),
];

#[test]
fn the_derived_list_is_the_hand_written_one() {
    assert_eq!(Battery::VARIABLES, HAND_WRITTEN);
    assert_eq!(Battery::MODEL_NAME, "battery");
}

#[test]
fn the_token_is_guid_shaped() {
    let token = Battery::INSTANTIATION_TOKEN;
    let groups: Vec<usize> = token
        .trim_matches(|c| c == '{' || c == '}')
        .split('-')
        .map(str::len)
        .collect();
    assert_eq!(groups, [8, 4, 4, 4, 12], "{token}");
}

#[test]
fn the_derived_enumeration_takes_its_discriminants() {
    assert_eq!(
        Mode::ITEMS,
        &[("Idle", 1), ("Charging", 2), ("Discharging", 3)]
    );
    assert_eq!(Mode::from_i64(3), Some(Mode::Discharging));
    assert_eq!(Mode::from_i64(0), None);
    assert_eq!(Mode::Charging.to_i64(), 2);
}
