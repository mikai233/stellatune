use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser, Clone)]
#[command(name = "stellatune-tui")]
#[command(about = "Terminal UI frontend for Stellatune")]
pub struct Cli {
    /// Override library database file path.
    #[arg(long)]
    pub db_path: Option<PathBuf>,

    /// Track page size for list/search queries.
    #[arg(long, default_value_t = 300)]
    pub page_size: i64,
}
