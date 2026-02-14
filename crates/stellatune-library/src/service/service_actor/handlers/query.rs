use stellatune_core::{PlaylistLite, TrackLite};
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LibraryServiceActor;

pub(crate) struct ListRootsMessage;

impl Message for ListRootsMessage {
    type Response = Result<Vec<String>, String>;
}

#[async_trait::async_trait]
impl Handler<ListRootsMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _message: ListRootsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<String>, String> {
        self.worker.list_roots().await.map_err(|e| format!("{e:#}"))
    }
}

pub(crate) struct ListFoldersMessage;

impl Message for ListFoldersMessage {
    type Response = Result<Vec<String>, String>;
}

#[async_trait::async_trait]
impl Handler<ListFoldersMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _message: ListFoldersMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<String>, String> {
        self.worker
            .list_folders()
            .await
            .map_err(|e| format!("{e:#}"))
    }
}

pub(crate) struct ListExcludedFoldersMessage;

impl Message for ListExcludedFoldersMessage {
    type Response = Result<Vec<String>, String>;
}

#[async_trait::async_trait]
impl Handler<ListExcludedFoldersMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _message: ListExcludedFoldersMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<String>, String> {
        self.worker
            .list_excluded_folders()
            .await
            .map_err(|e| format!("{e:#}"))
    }
}

pub(crate) struct ListTracksMessage {
    pub(crate) folder: String,
    pub(crate) recursive: bool,
    pub(crate) query: String,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

impl Message for ListTracksMessage {
    type Response = Result<Vec<TrackLite>, String>;
}

#[async_trait::async_trait]
impl Handler<ListTracksMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: ListTracksMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<TrackLite>, String> {
        self.worker
            .list_tracks(
                message.folder,
                message.recursive,
                message.query,
                message.limit,
                message.offset,
            )
            .await
            .map_err(|e| format!("{e:#}"))
    }
}

pub(crate) struct SearchTracksMessage {
    pub(crate) query: String,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

impl Message for SearchTracksMessage {
    type Response = Result<Vec<TrackLite>, String>;
}

#[async_trait::async_trait]
impl Handler<SearchTracksMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: SearchTracksMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<TrackLite>, String> {
        self.worker
            .search(message.query, message.limit, message.offset)
            .await
            .map_err(|e| format!("{e:#}"))
    }
}

pub(crate) struct ListPlaylistsMessage;

impl Message for ListPlaylistsMessage {
    type Response = Result<Vec<PlaylistLite>, String>;
}

#[async_trait::async_trait]
impl Handler<ListPlaylistsMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _message: ListPlaylistsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<PlaylistLite>, String> {
        self.worker
            .list_playlists()
            .await
            .map_err(|e| format!("{e:#}"))
    }
}

pub(crate) struct ListPlaylistTracksMessage {
    pub(crate) playlist_id: i64,
    pub(crate) query: String,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

impl Message for ListPlaylistTracksMessage {
    type Response = Result<Vec<TrackLite>, String>;
}

#[async_trait::async_trait]
impl Handler<ListPlaylistTracksMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: ListPlaylistTracksMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<TrackLite>, String> {
        self.worker
            .list_playlist_tracks(
                message.playlist_id,
                message.query,
                message.limit,
                message.offset,
            )
            .await
            .map_err(|e| format!("{e:#}"))
    }
}

pub(crate) struct ListLikedTrackIdsMessage;

impl Message for ListLikedTrackIdsMessage {
    type Response = Result<Vec<i64>, String>;
}

#[async_trait::async_trait]
impl Handler<ListLikedTrackIdsMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _message: ListLikedTrackIdsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<i64>, String> {
        self.worker
            .list_liked_track_ids()
            .await
            .map_err(|e| format!("{e:#}"))
    }
}
