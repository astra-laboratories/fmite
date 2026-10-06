//! Packages the battery FMU from the `cdylib` Cargo built next to this program.
//!
//! ```text
//! cargo build -p battery && cargo run -p battery --bin battery-package -- battery.fmu
//! ```

use std::path::PathBuf;

use battery::Battery;
use fmite::package::{Binary, package};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "battery.fmu".to_owned());
    let library = std::env::current_exe()?.with_file_name(format!(
        "{}battery{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    package::<Battery>(&[Binary::host(&library)?], &PathBuf::from(&out))?;
    println!("{out}");
    Ok(())
}
