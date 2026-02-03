pub(super) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub(super) fn normalize_path_str(path: &str) -> String {
    let mut s = path.replace('\\', "/");
    while s.ends_with('/') {
        s.pop();
    }
    s
}

pub(super) fn parent_dir_norm(path_norm: &str) -> Option<String> {
    let s = path_norm.trim_end_matches('/');
    let (parent, _) = s.rsplit_once('/')?;
    if parent.is_empty() {
        None
    } else {
        Some(parent.to_string())
    }
}

pub(super) fn is_drive_root(s: &str) -> bool {
    s.len() == 2 && s.ends_with(':')
}

pub(super) fn is_under_excluded(dir_norm: &str, excluded: &[String]) -> bool {
    excluded.iter().any(|ex| {
        dir_norm == ex
            || (!ex.is_empty()
                && dir_norm.starts_with(ex)
                && dir_norm.as_bytes().get(ex.len()) == Some(&b'/'))
    })
}
