//! Header conformance: `tests/abi.c` names all 75 functions of the FMI 3.0.1 headers
//! with the headers' own types, and links against this FMU's `cdylib`. A missing or
//! misspelled symbol fails the link. The headers cannot check Rust's parameter types,
//! so an importer checks those at runtime instead.
//!
//! Skipped, with a message, if there is no C compiler.

use std::path::{Path, PathBuf};
use std::process::Command;

fn run(command: &mut Command) -> String {
    let output = command.output().expect("the command runs");
    assert!(
        output.status.success(),
        "{command:?}\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn every_header_function_is_exported() {
    if Command::new("cc").arg("--version").output().is_err() {
        eprintln!("no C compiler; header conformance skipped");
        return;
    }
    let fmite = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // `cargo test` builds only the rlib, so build the cdylib separately.
    run(Command::new(env!("CARGO"))
        .args(["build", "--quiet", "--package", "battery", "--lib"])
        .current_dir(&fmite));
    // The test binary is target/<profile>/deps/abi-…; the cdylib is two levels up.
    let exe = std::env::current_exe().unwrap();
    let libraries: PathBuf = exe.ancestors().nth(2).unwrap().to_path_buf();
    let program = libraries.join("abi-conformance");
    run(Command::new("cc")
        .arg(fmite.join("tests/abi.c"))
        .arg("-I")
        .arg(fmite.join("tests/headers"))
        .arg("-L")
        .arg(&libraries)
        .arg(format!("-Wl,-rpath,{}", libraries.display()))
        .args(["-lbattery", "-o"])
        .arg(&program));
    // On Windows the C runtime writes stdout in text mode, so lines end in `\r\n`.
    let output = run(&mut Command::new(&program));
    assert_eq!(output.lines().collect::<Vec<_>>(), ["75 3.0"]);
}
