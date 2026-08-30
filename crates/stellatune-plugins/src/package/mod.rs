use std::io::Write;
use std::path::Path;

use crate::typescript::package::TypeScriptPackageError;

pub(crate) fn copy_dir_recursive(
    source: &Path,
    destination: &Path,
) -> Result<(), TypeScriptPackageError> {
    std::fs::create_dir_all(destination).map_err(|error| io("create directory", error))?;
    for entry in walkdir::WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|error| invalid(error.to_string()))?;
        let path = entry.path();
        if path == source {
            continue;
        }
        let relative = path
            .strip_prefix(source)
            .map_err(|error| invalid(error.to_string()))?;
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).map_err(|error| io("create directory", error))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|error| io("create directory", error))?;
            }
            std::fs::copy(path, target).map_err(|error| io("copy package file", error))?;
        }
    }
    Ok(())
}

pub(crate) fn extract_zip_to_dir(
    zip_path: &Path,
    destination: &Path,
) -> Result<(), TypeScriptPackageError> {
    let bytes = std::fs::read(zip_path).map_err(|error| io("read archive", error))?;
    let archive = rawzip::ZipArchive::from_slice(&bytes)
        .map_err(|error| invalid(format!("invalid zip archive: {error:?}")))?;
    for entry in archive.entries() {
        let entry = entry.map_err(|error| invalid(format!("invalid zip entry: {error:?}")))?;
        let normalized = entry
            .file_path()
            .try_normalize()
            .map_err(|error| invalid(format!("invalid zip path: {error:?}")))?
            .as_ref()
            .to_string();
        let relative = Path::new(&normalized);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(invalid(format!("unsafe zip path: {normalized}")));
        }
        let target = destination.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|error| io("create directory", error))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io("create directory", error))?;
        }
        let wayfinder = entry.wayfinder();
        let data = archive
            .get_entry(wayfinder)
            .map_err(|error| invalid(format!("read zip entry: {error:?}")))?
            .data();
        let mut output =
            std::fs::File::create(&target).map_err(|error| io("create file", error))?;
        match entry.compression_method() {
            rawzip::CompressionMethod::STORE => {
                std::io::copy(&mut &*data, &mut output)
                    .map_err(|error| io("extract file", error))?;
            },
            rawzip::CompressionMethod::DEFLATE => {
                let mut decoder = flate2::read::DeflateDecoder::new(data);
                std::io::copy(&mut decoder, &mut output)
                    .map_err(|error| io("extract file", error))?;
            },
            method => {
                return Err(invalid(format!(
                    "unsupported compression method: {method:?}"
                )));
            },
        }
        output.flush().map_err(|error| io("flush file", error))?;
    }
    Ok(())
}

fn io(operation: &'static str, error: impl std::fmt::Display) -> TypeScriptPackageError {
    TypeScriptPackageError::Io {
        operation,
        message: error.to_string(),
    }
}

fn invalid(message: impl Into<String>) -> TypeScriptPackageError {
    TypeScriptPackageError::Invalid(message.into())
}
