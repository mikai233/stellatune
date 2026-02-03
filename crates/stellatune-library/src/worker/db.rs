use std::path::Path;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use tracing::debug;

use super::paths::{normalize_path_str, parent_dir_norm};

pub(super) async fn init_db(db_path: &Path) -> Result<SqlitePool> {
    let mut opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(3));

    // Helpful runtime pragmas.
    opts = opts.pragma("temp_store", "MEMORY");
    opts = opts.pragma("foreign_keys", "ON");

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .context("failed to connect sqlite")?;

    ensure_fts5(&pool).await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    backfill_norm_paths(&pool).await?;

    debug!("sqlite ready: {}", db_path.display());
    Ok(pool)
}

async fn backfill_norm_paths(pool: &SqlitePool) -> Result<()> {
    // Populate path_norm/dir_norm for tracks created before this feature existed.
    // Done at startup so folder browsing works without requiring a full re-scan.
    let rows = sqlx::query!(
        r#"
        SELECT id, path
        FROM tracks
        WHERE path_norm = '' OR dir_norm = ''
        LIMIT 20000
        "#,
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    for r in rows {
        let id = r.id;
        let path_norm = normalize_path_str(&r.path);
        let dir_norm = parent_dir_norm(&path_norm).unwrap_or_default();
        sqlx::query!(
            "UPDATE tracks SET path_norm=?1, dir_norm=?2 WHERE id=?3",
            path_norm,
            dir_norm,
            id
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn ensure_fts5(pool: &SqlitePool) -> Result<()> {
    // Best-effort: SQLite might not be compiled with compileoption reporting, so we also rely on
    // the migration creation to fail if FTS5 is missing. This check exists for a clearer error.
    let enabled: Option<i64> =
        sqlx::query_scalar("SELECT sqlite_compileoption_used('ENABLE_FTS5')")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    if let Some(0) = enabled {
        anyhow::bail!("SQLite is missing FTS5 support (ENABLE_FTS5=0)");
    }
    Ok(())
}
