use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

use crate::env::{SidecarLogConfig, sidecar_log_config_from_env};

const MAX_LOG_FILES: usize = 20;
const LOG_RETENTION_DAYS: u64 = 7;

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

pub fn init_daily_file_tracing_from_env() -> Result<(), Box<dyn Error>> {
    let config = sidecar_log_config_from_env()?;
    init_daily_file_tracing(&config)
}

pub fn init_daily_file_tracing(config: &SidecarLogConfig) -> Result<(), Box<dyn Error>> {
    cleanup_old_logs(&config.dir)?;
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(config.file_prefix.as_str())
        .filename_suffix("log")
        .build(config.dir.as_path())?;
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);
    let filter = EnvFilter::try_new(config.level.as_str())?;

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_target(false)
        .with_ansi(false)
        .compact()
        .try_init();
    Ok(())
}

fn cleanup_old_logs(log_dir: &Path) -> io::Result<()> {
    let now = SystemTime::now();
    let retention = Duration::from_secs(LOG_RETENTION_DAYS * 24 * 60 * 60);
    let mut candidates = Vec::<LogEntry>::new();

    for entry in std::fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("log") {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(_) => continue,
        };
        if now.duration_since(modified).unwrap_or_default() > retention {
            let _ = std::fs::remove_file(&path);
            continue;
        }

        candidates.push(LogEntry { path, modified });
    }

    candidates.sort_by(|left, right| right.modified.cmp(&left.modified));
    for entry in candidates.into_iter().skip(MAX_LOG_FILES) {
        let _ = std::fs::remove_file(entry.path);
    }
    Ok(())
}

struct LogEntry {
    path: PathBuf,
    modified: SystemTime,
}
