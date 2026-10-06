//! Drives the battery through `fmite::Instance` as an importer would, and checks its
//! model description against the standard's schema.

use battery::{Battery, Mode};
use fmite::abi::Status;
use fmite::description::model_description;
use fmite::log::Logger;
use fmite::{Instance, Instantiation, Interface, Values, ValuesMut, Variables};

fn initialized(current: f64) -> Instance<Battery> {
    let context = Instantiation {
        instance_name: "test".to_owned(),
        resource_path: None,
    };
    let token = Battery::INSTANTIATION_TOKEN;
    let mut battery =
        Instance::instantiate(token, context, Interface::CoSimulation, Logger::silent()).unwrap();
    assert_eq!(battery.set(&[1], Values::Float64(&[current])), Status::Ok);
    assert_eq!(battery.enter_initialization(0.0, None), Status::Ok);
    assert_eq!(battery.exit_initialization(), Status::Ok);
    battery
}

#[test]
fn a_discharge_drains_the_charge_and_heats_the_pack() {
    let mut battery = initialized(100.0);
    assert_eq!(battery.do_step(0.0, 360.0).0, Status::Ok);
    let model = battery.model();
    // 100 A for a tenth of an hour is 10 Ah of 50.
    assert!((*model.soc - 80.0).abs() < 1e-6, "{}", *model.soc);
    assert!(*model.temperature > 25.0);
    assert_eq!(*model.mode, Mode::Discharging);
    let mut cells = [0.0; 4];
    assert_eq!(
        battery.get(&[7], ValuesMut::Float64(&mut cells)),
        Status::Ok
    );
    assert!(cells.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn an_empty_pack_asks_the_importer_to_stop() {
    let mut battery = initialized(1000.0);
    let (status, terminate, now) = battery.do_step(0.0, 600.0);
    assert_eq!((status, terminate), (Status::Ok, true));
    assert!((now - 600.0).abs() < 1e-9);
}

#[test]
fn the_description_validates_against_the_official_schema() {
    let xml = model_description::<Battery>().unwrap();
    let fmite = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let file = std::env::temp_dir().join(format!("battery-{}.xml", std::process::id()));
    std::fs::write(&file, xml).unwrap();
    let Ok(output) = std::process::Command::new("xmllint")
        .args(["--noout", "--schema"])
        .arg(format!("{fmite}/tests/schema/fmi3ModelDescription.xsd"))
        .arg(&file)
        .output()
    else {
        eprintln!("xmllint not found; schema check skipped");
        return;
    };
    let _ = std::fs::remove_file(&file);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
