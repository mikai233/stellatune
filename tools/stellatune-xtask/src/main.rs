use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAX_RUST_LOC: usize = 1_200;
const SCAN_ROOTS: [&str; 3] = ["apps", "crates", "tools"];
const GENERATED_ALLOWLIST: [&str; 1] = ["crates/stellatune-ffi/src/frb_generated.rs"];

fn main() {
    let result = match env::args().nth(1).as_deref() {
        Some("check-loc") => find_workspace_root().and_then(|root| check_loc(&root)),
        Some(command) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown xtask `{command}`; expected `check-loc`"),
        )),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing xtask; expected `check-loc`",
        )),
    };

    if let Err(error) = result {
        eprintln!("stellatune-xtask: {error}");
        std::process::exit(1);
    }
}

fn find_workspace_root() -> io::Result<PathBuf> {
    let mut current = env::current_dir()?;
    loop {
        let manifest = current.join("Cargo.toml");
        if manifest.is_file() && fs::read_to_string(&manifest)?.contains("[workspace]") {
            return Ok(current);
        }
        if !current.pop() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "could not find workspace Cargo.toml",
            ));
        }
    }
}

fn check_loc(workspace_root: &Path) -> io::Result<()> {
    let mut violations = Vec::new();
    for scan_root in SCAN_ROOTS {
        collect_violations(
            workspace_root,
            &workspace_root.join(scan_root),
            &mut violations,
        )?;
    }

    violations.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    if violations.is_empty() {
        println!("Rust LOC check passed (limit: {MAX_RUST_LOC} lines)");
        return Ok(());
    }

    eprintln!("Rust files exceeding {MAX_RUST_LOC} physical lines:");
    for (path, lines) in violations {
        eprintln!("{lines:>5}  {}", path.display());
    }
    Err(io::Error::other("Rust LOC limit exceeded"))
}

fn collect_violations(
    workspace_root: &Path,
    directory: &Path,
    violations: &mut Vec<(PathBuf, usize)>,
) -> io::Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            if entry.file_name() != "target" {
                collect_violations(workspace_root, &path, violations)?;
            }
            continue;
        }
        if !file_type.is_file() || path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }

        let relative = normalized_relative_path(workspace_root, &path)?;
        if GENERATED_ALLOWLIST.contains(&relative.as_str()) {
            continue;
        }
        let lines = physical_line_count(&path)?;
        if lines > MAX_RUST_LOC {
            violations.push((PathBuf::from(relative), lines));
        }
    }
    Ok(())
}

fn normalized_relative_path(workspace_root: &Path, path: &Path) -> io::Result<String> {
    path.strip_prefix(workspace_root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}

fn physical_line_count(path: &Path) -> io::Result<usize> {
    let bytes = fs::read(path)?;
    if bytes.is_empty() {
        return Ok(0);
    }
    Ok(bytes.iter().filter(|byte| **byte == b'\n').count()
        + usize::from(bytes.last() != Some(&b'\n')))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn fixture_root() -> PathBuf {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("stellatune-xtask-{}-{id}", std::process::id()));
        fs::create_dir_all(root.join("crates/stellatune-ffi/src")).unwrap();
        root
    }

    fn write_lines(path: &Path, count: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "line\n".repeat(count)).unwrap();
    }

    #[test]
    fn physical_line_count_includes_final_unterminated_line() {
        let root = fixture_root();
        let path = root.join("sample.rs");
        fs::write(&path, "one\ntwo").unwrap();
        assert_eq!(physical_line_count(&path).unwrap(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_only_files_over_the_limit() {
        let root = fixture_root();
        write_lines(&root.join("crates/within.rs"), MAX_RUST_LOC);
        write_lines(&root.join("crates/over.rs"), MAX_RUST_LOC + 1);
        let mut violations = Vec::new();
        collect_violations(&root, &root.join("crates"), &mut violations).unwrap();
        assert_eq!(violations, vec![(PathBuf::from("crates/over.rs"), 1_201)]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn skips_only_the_explicit_generated_allowlist() {
        let root = fixture_root();
        write_lines(
            &root.join("crates/stellatune-ffi/src/frb_generated.rs"),
            MAX_RUST_LOC + 1,
        );
        write_lines(
            &root.join("crates/stellatune-ffi/src/other_generated.rs"),
            MAX_RUST_LOC + 1,
        );
        let mut violations = Vec::new();
        collect_violations(&root, &root.join("crates"), &mut violations).unwrap();
        assert_eq!(
            violations,
            vec![(
                PathBuf::from("crates/stellatune-ffi/src/other_generated.rs"),
                1_201
            )]
        );
        fs::remove_dir_all(root).unwrap();
    }
}
