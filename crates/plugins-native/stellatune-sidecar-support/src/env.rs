use std::io;
use std::path::{Path, PathBuf};

pub const SIDECAR_LOG_DIR_ENV: &str = "STELLATUNE_SIDECAR_LOG_DIR";
pub const SIDECAR_LOG_LEVEL_ENV: &str = "STELLATUNE_SIDECAR_LOG_LEVEL";
pub const SIDECAR_LOG_FILE_PREFIX_ENV: &str = "STELLATUNE_SIDECAR_LOG_FILE_PREFIX";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarLogConfig {
    pub dir: PathBuf,
    pub level: String,
    pub file_prefix: String,
}

pub fn build_sidecar_log_config(
    executable: &str,
    plugins_dir: &Path,
    plugin_id: &str,
    level: &str,
) -> io::Result<SidecarLogConfig> {
    let file_prefix = derive_sidecar_log_prefix(executable)?;
    let level = validate_log_level(level)?;
    let dir = default_sidecar_log_dir(plugins_dir, plugin_id, file_prefix.as_str())?;
    Ok(SidecarLogConfig {
        dir,
        level,
        file_prefix,
    })
}

pub fn sidecar_log_config_from_env() -> io::Result<SidecarLogConfig> {
    let file_prefix = std::env::var(SIDECAR_LOG_FILE_PREFIX_ENV)
        .ok()
        .map(|value| sanitize_log_file_prefix(value.as_str()))
        .filter(|value| !value.is_empty())
        .ok_or_else(missing_sidecar_log_file_prefix_error)?;
    let dir = std::env::var_os(SIDECAR_LOG_DIR_ENV)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .map(create_sidecar_log_dir)
        .transpose()?
        .ok_or_else(missing_sidecar_log_dir_error)?;
    let level = std::env::var(SIDECAR_LOG_LEVEL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(missing_sidecar_log_level_error)?;

    Ok(SidecarLogConfig {
        dir,
        level,
        file_prefix,
    })
}

fn derive_sidecar_log_prefix(executable: &str) -> io::Result<String> {
    let path = Path::new(executable.trim());
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|value| value.to_str())
        .map(sanitize_log_file_prefix)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unable to derive sidecar log prefix from executable `{executable}`"),
            )
        })
}

fn validate_log_level(level: &str) -> io::Result<String> {
    let trimmed = level.trim();
    if trimmed.is_empty() {
        return Err(missing_sidecar_log_level_error());
    }
    Ok(trimmed.to_string())
}

fn default_sidecar_log_dir(
    plugins_dir: &Path,
    plugin_id: &str,
    log_prefix: &str,
) -> io::Result<PathBuf> {
    create_sidecar_log_dir(plugin_sidecar_log_dir(plugins_dir, plugin_id, log_prefix))
}

fn plugin_sidecar_log_dir(plugins_dir: &Path, plugin_id: &str, log_prefix: &str) -> PathBuf {
    plugins_dir
        .join(".stellatune")
        .join("logs")
        .join(plugin_id)
        .join(log_prefix)
}

fn missing_sidecar_log_dir_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("missing required env var `{SIDECAR_LOG_DIR_ENV}`"),
    )
}

fn missing_sidecar_log_file_prefix_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("missing required env var `{SIDECAR_LOG_FILE_PREFIX_ENV}`"),
    )
}

fn missing_sidecar_log_level_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("missing required env var `{SIDECAR_LOG_LEVEL_ENV}`"),
    )
}

fn create_sidecar_log_dir(path: PathBuf) -> io::Result<PathBuf> {
    std::fs::create_dir_all(&path).map(|_| path)
}

fn sanitize_log_file_prefix(value: &str) -> String {
    let mut sanitized = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            sanitized.push(ch.to_ascii_lowercase());
        } else {
            sanitized.push('-');
        }
    }
    sanitized.trim_matches('-').to_string()
}
