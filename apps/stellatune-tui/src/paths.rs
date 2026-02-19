use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use directories::ProjectDirs;

use crate::cli::Cli;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub db_path: PathBuf,
    pub plugins_dir: PathBuf,
    pub log_path: PathBuf,
}

pub fn resolve_paths(cli: &Cli) -> Result<AppPaths> {
    let db_path = match cli.db_path.as_ref() {
        Some(path) => path.clone(),
        None => default_data_dir()?.join("stellatune.sqlite3"),
    };
    let data_dir = db_path
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("invalid db path: {}", db_path.display()))?;
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("create data dir {}", data_dir.display()))?;

    let plugins_dir = data_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir)
        .with_context(|| format!("create plugins dir {}", plugins_dir.display()))?;

    let log_path = data_dir.join("stellatune-tui.log");

    Ok(AppPaths {
        db_path,
        plugins_dir,
        log_path,
    })
}

fn default_data_dir() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "stellatune", "stellatune")
        .ok_or_else(|| anyhow!("failed to resolve default data directory"))?;
    let path = dirs.data_local_dir().to_path_buf();
    std::fs::create_dir_all(&path).with_context(|| format!("create {}", path.display()))?;
    Ok(path)
}
