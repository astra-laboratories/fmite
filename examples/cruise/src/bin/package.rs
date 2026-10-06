//! Packages the cruise FMU from the `cdylib` Cargo built next to this program.
//!
//! ```text
//! cargo build -p cruise && cargo run -p cruise --bin package -- cruise.fmu
//! ```

use std::path::PathBuf;

use cruise::Cruise;
use fmite::package::{Binary, package};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "cruise.fmu".to_owned());
    let library = std::env::current_exe()?.with_file_name(format!(
        "{}cruise{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    package::<Cruise>(&[Binary::host(&library)?], &PathBuf::from(&out))?;
    println!("{out}");
    Ok(())
}
