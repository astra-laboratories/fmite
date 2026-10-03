//! Packaging: the FMU archive, `modelDescription.xml` and one shared library per
//! platform, written from the host. A value in memory is not a compiled library, so
//! this is a call on the model type, made by a program that links the model crate as
//! an rlib and is handed the `cdylib`s it built.

use std::io::{Cursor, Write as _};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::CoSimulation;
use crate::Error;
use crate::description::{model_description, model_identifier};
use crate::export::Exported;

/// A compiled `cdylib` and the FMI platform tuple it runs on, `"x86_64-linux"`.
#[derive(Clone, Copy, Debug)]
pub struct Binary<'a> {
    pub platform: &'a str,
    pub path: &'a Path,
}

impl<'a> Binary<'a> {
    /// A library built for the host this runs on.
    ///
    /// # Errors
    ///
    /// On a host FMI names no platform tuple for.
    pub fn host(path: &'a Path) -> Result<Self, Error> {
        let platform = platform(std::env::consts::ARCH, std::env::consts::OS).ok_or_else(|| {
            Error::new(format!(
                "FMI names no platform for {}-{}",
                std::env::consts::ARCH,
                std::env::consts::OS
            ))
        })?;
        Ok(Self { platform, path })
    }
}

/// The FMI platform tuple for an architecture and an operating system, as
/// `std::env::consts` spells them.
#[must_use]
pub fn platform(arch: &str, os: &str) -> Option<&'static str> {
    Some(match (arch, os) {
        ("x86_64", "linux") => "x86_64-linux",
        ("aarch64", "linux") => "aarch64-linux",
        ("x86_64", "macos") => "x86_64-darwin",
        ("aarch64", "macos") => "aarch64-darwin",
        ("x86_64", "windows") => "x86_64-windows",
        ("aarch64", "windows") => "aarch64-windows",
        _ => return None,
    })
}

/// Writes the FMU of `T` to `out`: its model description, and each binary under
/// `binaries/<platform>/<modelIdentifier>.<extension>`. Entries are stored
/// uncompressed, with a fixed timestamp, so the same inputs give the same bytes.
///
/// # Errors
///
/// When a binary cannot be read, the archive cannot be written, or the description
/// cannot be built.
pub fn package<T: CoSimulation + Exported>(
    binaries: &[Binary<'_>],
    out: &Path,
) -> Result<(), Error> {
    let identifier = model_identifier::<T>();
    let description = model_description::<T>()?;
    let mut entries = vec![("modelDescription.xml".to_owned(), description.into_bytes())];
    for binary in binaries {
        let extension = (binary.path.extension())
            .and_then(|extension| extension.to_str())
            .ok_or_else(|| Error::new(format!("{} has no extension", binary.path.display())))?;
        let contents = std::fs::read(binary.path)
            .map_err(|e| Error::new(format!("{}: {e}", binary.path.display())))?;
        entries.push((
            format!("binaries/{}/{identifier}.{extension}", binary.platform),
            contents,
        ));
    }

    let archive = |e: zip::result::ZipError| Error::new(format!("the FMU archive: {e}"));
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default());
    for (path, contents) in entries {
        zip.start_file(path, stored).map_err(archive)?;
        zip.write_all(&contents)
            .map_err(|e| Error::new(format!("the FMU archive: {e}")))?;
    }
    let bytes = zip.finish().map_err(archive)?.into_inner();
    std::fs::write(out, bytes).map_err(|e| Error::new(format!("{}: {e}", out.display())))
}
