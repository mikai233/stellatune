use std::path::Path;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use tracing::debug;

use super::paths::{normalize_path_str, parent_dir_norm};

pub(crate) async fn init_db(db_path: &Path) -> Result<SqlitePool> {
    let pool = connect_pool(db_path).await?;
    check_schema(&pool).await?;

    ensure_fts5(&pool).await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    backfill_norm_paths(&pool).await?;
    backfill_artist_names(&pool).await?;

    debug!("sqlite ready: {}", db_path.display());
    Ok(pool)
}

pub(crate) async fn open_state_db_pool(db_path: &Path) -> Result<SqlitePool> {
    let pool = connect_pool(db_path).await?;
    check_schema(&pool).await?;

    ensure_fts5(&pool).await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    backfill_artist_names(&pool).await?;
    Ok(pool)
}

async fn backfill_artist_names(pool: &SqlitePool) -> Result<()> {
    loop {
        let rows = sqlx::query(
            "SELECT id,artist,album_artist,artists_json FROM tracks WHERE artist_names_version=0 ORDER BY id LIMIT 500",
        ).fetch_all(pool).await?;
        if rows.is_empty() {
            return Ok(());
        }
        let mut transaction = pool.begin().await?;
        for row in rows {
            let artists = crate::artist_names::normalize_artists_json(
                row.try_get::<&str, _>("artists_json")?,
                row.try_get::<Option<&str>, _>("artist")?,
            )?;
            let album_artists = crate::artist_names::normalize_artists_json(
                "[]",
                row.try_get::<Option<&str>, _>("album_artist")?,
            )?;
            sqlx::query("UPDATE tracks SET artists_json=?,album_artists_json=?,artist_names_version=1 WHERE id=? AND artist_names_version=0")
                .bind(artists).bind(album_artists).bind(row.try_get::<i64, _>("id")?)
                .execute(&mut *transaction).await?;
        }
        transaction.commit().await?;
    }
}

async fn check_schema(pool: &SqlitePool) -> Result<()> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='tracks'",
    )
    .fetch_one(pool)
    .await?;
    if exists == 0 {
        return Ok(());
    }
    let marker: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='library_schema'",
    )
    .fetch_one(pool)
    .await?;
    let version = if marker == 0 {
        None
    } else {
        sqlx::query_scalar::<_, i64>("SELECT version FROM library_schema")
            .fetch_optional(pool)
            .await?
    };
    if version != Some(2) {
        anyhow::bail!(
            "LIBRARY_REBUILD_REQUIRED: music library schema changed; explicitly rebuild the library and playback state"
        );
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upgrades_artist_index_without_rescan_or_repeated_updates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        let old = connect_pool(&path).await.unwrap();
        let mut migrations = sqlx::migrate!("./migrations");
        migrations.migrations.to_mut().retain(|m| m.version < 11);
        migrations.run(&old).await.unwrap();
        // More than one backfill batch, including repeated tags and album credits.
        sqlx::query("WITH RECURSIVE n(id) AS (SELECT 1 UNION ALL SELECT id+1 FROM n WHERE id<501) INSERT INTO tracks(id,path,path_norm,dir_norm,mtime_ms,size_bytes,meta_scanned_ms,artist,album,album_artist,artists_json) SELECT id,'/music/'||id||'.mp3','/music/'||id||'.mp3','/music',123,456,789,'A / B','Album','A、C','[\"A / B\",\"B\"]' FROM n")
            .execute(&old).await.unwrap();
        let snapshot_sql = "SELECT json_array(id,path,mtime_ms,size_bytes,meta_scanned_ms,artist,album,album_artist,album_key) FROM catalog_tracks ORDER BY id";
        let before: Vec<String> = sqlx::query_scalar(snapshot_sql)
            .fetch_all(&old)
            .await
            .unwrap();
        old.close().await;

        let pool = init_db(&path).await.unwrap();
        let after: Vec<String> = sqlx::query_scalar(snapshot_sql)
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(after, before);
        let normalized: i64 = sqlx::query_scalar("SELECT count(*) FROM tracks WHERE artists_json='[\"A\",\"B\"]' AND album_artists_json='[\"A\",\"C\"]' AND artist_names_version=1")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(normalized, 501);
        let names: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT name FROM catalog_artists ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(names, ["A", "B", "C"]);
        let revision: i64 = sqlx::query_scalar("SELECT revision FROM catalog_revision")
            .fetch_one(&pool)
            .await
            .unwrap();
        pool.close().await;
        let reopened = open_state_db_pool(&path).await.unwrap();
        let unchanged: i64 = sqlx::query_scalar("SELECT revision FROM catalog_revision")
            .fetch_one(&reopened)
            .await
            .unwrap();
        assert_eq!(unchanged, revision);
        reopened.close().await;
    }
}
