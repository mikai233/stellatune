use std::path::Path;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use tracing::debug;

use super::paths::{normalize_path_str, parent_dir_norm};

pub(super) async fn init_db(db_path: &Path) -> Result<SqlitePool> {
    let pool = connect_pool(db_path).await?;

    ensure_fts5(&pool).await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    backfill_norm_paths(&pool).await?;

    debug!("sqlite ready: {}", db_path.display());
    Ok(pool)
}

pub(crate) async fn open_state_db_pool(db_path: &Path) -> Result<SqlitePool> {
    let pool = connect_pool(db_path).await?;

    ensure_fts5(&pool).await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    Ok(pool)
}

async fn connect_pool(db_path: &Path) -> Result<SqlitePool> {
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
    Ok(pool)
}

pub(crate) async fn list_disabled_plugin_ids(
    pool: &SqlitePool,
) -> Result<std::collections::HashSet<String>> {
    let rows = sqlx::query_scalar::<_, String>(
        r#"
        SELECT plugin_id
        FROM plugin_state
        WHERE enabled = 0
        "#,
    )
    .fetch_all(pool)
    .await?;

    let disabled = rows
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();

    Ok(disabled)
}

pub(crate) async fn replace_disabled_plugin_ids(
    pool: &SqlitePool,
    disabled_ids: &std::collections::HashSet<String>,
) -> Result<()> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        UPDATE plugin_state
        SET enabled = 1,
            disable_in_progress = 0,
            updated_at_ms = ?1
        "#,
    )
    .bind(now_ms)
    .execute(&mut *tx)
    .await?;

    for plugin_id in disabled_ids {
        sqlx::query(
            r#"
            INSERT INTO plugin_state(
                plugin_id,
                enabled,
                install_state,
                disable_in_progress,
                last_error,
                updated_at_ms
            )
            VALUES (?1, 0, 'installed', 0, NULL, ?2)
            ON CONFLICT(plugin_id) DO UPDATE SET
                enabled = 0,
                disable_in_progress = 0,
                updated_at_ms = excluded.updated_at_ms
            "#,
        )
        .bind(plugin_id)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
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
