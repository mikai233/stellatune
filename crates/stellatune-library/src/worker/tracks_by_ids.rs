use std::collections::HashMap;

use anyhow::Result;
use sqlx::{QueryBuilder, Sqlite};

use super::LibraryWorker;

impl LibraryWorker {
    pub(crate) async fn tracks_by_ids(
        &self,
        ids: &[i64],
    ) -> Result<HashMap<i64, crate::TrackLite>> {
        let mut tracks = HashMap::new();
        for chunk in ids.chunks(400) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "SELECT id,path,title,artist,album,duration_ms FROM tracks WHERE id IN (",
            );
            let mut values = query.separated(",");
            for id in chunk {
                values.push_bind(*id);
            }
            query.push(")");
            tracks.extend(
                query
                    .build_query_as::<super::tracks::TrackLiteRow>()
                    .fetch_all(&self.pool)
                    .await?
                    .into_iter()
                    .map(|row| {
                        (
                            row.id,
                            crate::TrackLite {
                                id: row.id,
                                path: row.path,
                                title: row.title,
                                artist: row.artist,
                                album: row.album,
                                duration_ms: row.duration_ms,
                                cover_id: None,
                                is_segment: false,
                            },
                        )
                    }),
            );
        }
        let mut values = tracks.into_values().collect::<Vec<_>>();
        super::tracks::decorate(&self.pool, &mut values).await?;
        Ok(values.into_iter().map(|track| (track.id, track)).collect())
    }
}
