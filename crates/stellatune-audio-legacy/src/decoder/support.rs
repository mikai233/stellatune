use std::path::Path;

/// Built-in decoder extension support allowlist.
///
/// Keep this explicit so higher layers can decide whether fallback to built-in decoding is valid
/// after a plugin decoder fails to open.
pub fn supports_extension(ext: &str) -> bool {
    matches!(ext, "mp3" | "flac" | "wav")
}

/// Returns whether built-in decoding is allowed for a given path by extension allowlist.
pub fn supports_path(path: impl AsRef<Path>) -> bool {
    let ext = path
        .as_ref()
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    supports_extension(&ext)
}
