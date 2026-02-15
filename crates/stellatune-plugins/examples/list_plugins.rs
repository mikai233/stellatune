use std::collections::HashSet;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let plugin_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "plugins".to_string());

    let service = stellatune_plugins::runtime::handle::shared_runtime_service();

    let report = service
        .reload_dir_filtered(&plugin_dir, &HashSet::new())
        .await?;
    if !report.errors.is_empty() {
        eprintln!("load errors:");
        for err in &report.errors {
            eprintln!("  - {err:#}");
        }
    }

    println!("Active plugins:");
    for item in service.list_active_plugins().await {
        println!("  - {} ({})", item.id, item.name);
    }

    Ok(())
}
