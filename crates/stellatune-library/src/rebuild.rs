//! Explicit, transactional reset of music identities. Configuration is retained.
use anyhow::Result;
use sqlx::{
    Connection, Row,
    sqlite::{SqliteConnectOptions, SqliteConnection},
};
use std::path::Path;

pub async fn required(path: &Path) -> Result<bool> {
    if !path.try_exists()? {
        return Ok(false);
    }
    let mut db =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(path).read_only(true))
            .await?;
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&mut db)
            .await?;
    if !tables.iter().any(|name| name == "tracks") {
        return Ok(false);
    }
    if !tables.iter().any(|name| name == "library_schema") {
        return Ok(true);
    }
    let version: Option<i64> = sqlx::query_scalar("SELECT version FROM library_schema")
        .fetch_optional(&mut db)
        .await?;
    if version != Some(2) {
        return Ok(true);
    }
    if tables.iter().any(|name| name == "player_schema_meta") {
        let version: Option<i64> =
            sqlx::query_scalar("SELECT schema_version FROM player_schema_meta WHERE singleton=1")
                .fetch_optional(&mut db)
                .await?;
        return Ok(version != Some(2));
    }
    Ok(false)
}

/// Call only before opening the library/player services, after explicit user action.
/// Any SQL failure rolls the entire reset back, including the original music data.
pub async fn rebuild(path: &Path) -> Result<()> {
    let mut db = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(false),
    )
    .await?;
    let mut tx = db.begin().await?;
    let tables = [
        "tracks",
        "tracks_fts",
        "playlist_tracks",
        "catalog_artists",
        "catalog_revision",
        "audio_files",
        "cue_documents",
        "library_schema",
        "track_presentation",
        "playback_state",
        "playback_queue",
        "track_catalog",
        "player_schema_meta",
        "_sqlx_migrations",
    ];
    let triggers = sqlx::query("SELECT name,tbl_name FROM sqlite_master WHERE type='trigger'")
        .fetch_all(&mut *tx)
        .await?;
    for trigger in triggers {
        let table: &str = trigger.get("tbl_name");
        if tables.contains(&table) || matches!(table, "scan_roots" | "playlists") {
            let name: &str = trigger.get("name");
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "DROP TRIGGER \"{}\"",
                name.replace('"', "\"\"")
            )))
            .execute(&mut *tx)
            .await?;
        }
    }
    sqlx::raw_sql("DROP VIEW IF EXISTS catalog_tracks")
        .execute(&mut *tx)
        .await?;
    for table in tables {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP TABLE IF EXISTS \"{table}\""
        )))
        .execute(&mut *tx)
        .await?;
    }
    sqlx::raw_sql("CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL)").execute(&mut *tx).await?;
    for migration in sqlx::migrate!("./migrations").iter() {
        sqlx::raw_sql(migration.sql.clone())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES(?,?,1,?,0)")
            .bind(migration.version).bind(migration.description.as_ref()).bind(migration.checksum.as_ref()).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE scan_roots SET last_scan_ms=0")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    db.close().await?;
    // Artwork is derived data. IDs restart after a rebuild, so stale numeric
    // cache entries must not be displayed for newly scanned, coverless files.
    if let Some(parent) = path.parent() {
        let covers = parent.join("covers");
        if covers.is_dir() && !covers.is_symlink() {
            for entry in std::fs::read_dir(covers)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.trim_end_matches(".tmp").parse::<i64>().is_ok()
                    && entry.file_type()?.is_file()
                {
                    std::fs::remove_file(entry.path())?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn sql_failure_does_not_partially_reset_music_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.db");
        let pool = crate::worker::db::init_db(&path).await.unwrap();
        sqlx::raw_sql("INSERT INTO tracks(path,ext,mtime_ms,size_bytes) VALUES('song.wav','wav',0,0); DROP TABLE library_schema; DROP INDEX idx_plugin_state_enabled; ALTER TABLE plugin_state DROP COLUMN enabled;").execute(&pool).await.unwrap();
        pool.close().await;
        assert!(rebuild(&path).await.is_err());
        let mut db = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracks")
                .fetch_one(&mut db)
                .await
                .unwrap(),
            1
        );
        assert!(required(&path).await.unwrap());
        db.close().await.unwrap();
    }
    #[tokio::test]
    async fn old_schema_requires_explicit_reset_and_retains_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.db");
        let pool = crate::worker::db::init_db(&path).await.unwrap();
        sqlx::raw_sql("INSERT INTO scan_roots(path) VALUES('D:/Music'); INSERT INTO plugin_state(plugin_id,enabled) VALUES('plugin',0); DROP TABLE library_schema;").execute(&pool).await.unwrap();
        pool.close().await;
        assert!(required(&path).await.unwrap());
        assert!(crate::worker::db::init_db(&path).await.is_err());
        rebuild(&path).await.unwrap();
        assert!(!required(&path).await.unwrap());
        let pool = crate::worker::db::init_db(&path).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT path FROM scan_roots")
                .fetch_one(&pool)
                .await
                .unwrap(),
            "D:/Music"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT enabled FROM plugin_state")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracks")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        pool.close().await;
    }
}
