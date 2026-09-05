use std::collections::HashMap;

use anyhow::Result;
use sqlx::{QueryBuilder, Sqlite};

use super::LibraryWorker;

impl LibraryWorker {
    pub(crate) async fn track_paths(&self, ids: &[i64]) -> Result<HashMap<i64, String>> {
        let mut paths = HashMap::new();
        for chunk in ids.chunks(400) {
            let mut query = QueryBuilder::<Sqlite>::new("SELECT id,path FROM tracks WHERE id IN (");
            let mut values = query.separated(",");
            for id in chunk {
                values.push_bind(*id);
            }
            query.push(")");
            paths.extend(
                query
                    .build_query_as::<(i64, String)>()
                    .fetch_all(&self.pool)
                    .await?,
            );
        }
        Ok(paths)
    }
}
