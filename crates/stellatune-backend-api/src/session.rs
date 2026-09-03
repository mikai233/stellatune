use anyhow::Result;

use crate::library::LibraryService;
use crate::lyrics_service::LyricsService;
use crate::player_service::catalog::PlayerCatalog;
use crate::player_service::service::PlayerService;
use crate::runtime::{
    TypeScriptSourceResolverFactory, shared_playback_controller, shared_typescript_runtime,
};
use std::sync::Arc;
use stellatune_audio::playback::control::PlaybackController;

#[derive(Debug, Clone, Default)]
pub struct BackendSessionOptions {
    pub library: Option<LibrarySessionOptions>,
}

impl BackendSessionOptions {
    pub fn with_library(db_path: impl Into<String>) -> Self {
        Self {
            library: Some(LibrarySessionOptions {
                db_path: db_path.into(),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LibrarySessionOptions {
    pub db_path: String,
}

pub struct BackendSession {
    player: PlaybackController,
    player_service: Option<Arc<PlayerService>>,
    lyrics: Arc<LyricsService>,
    library: Option<LibraryService>,
}

impl BackendSession {
    pub fn new() -> Self {
        Self {
            player: shared_playback_controller(),
            player_service: None,
            lyrics: LyricsService::new(),
            library: None,
        }
    }

    pub async fn from_options(options: BackendSessionOptions) -> Result<Self> {
        let player = shared_playback_controller();
        let lyrics = LyricsService::new();
        let (library, player_service) = match options.library {
            Some(opts) => {
                let library = LibraryService::new(opts.db_path.clone()).await?;
                let catalog = PlayerCatalog::open(&opts.db_path).await?;
                let local_source = catalog.ensure_local_source().await?;
                let _ = local_source;
                let service = Arc::new(PlayerService::new(
                    catalog,
                    player.clone(),
                    Arc::new(library.handle().clone()),
                    Arc::new(TypeScriptSourceResolverFactory::new(
                        shared_typescript_runtime(),
                    )),
                ));
                service.start_state_writer();
                if let Err(error) = service.restore().await {
                    tracing::warn!(%error, "player state restore skipped");
                }
                (Some(library), Some(service))
            },
            None => (None, None),
        };
        Ok(Self {
            player,
            player_service,
            lyrics,
            library,
        })
    }

    pub fn player(&self) -> &PlaybackController {
        &self.player
    }

    pub fn player_service(&self) -> Option<&Arc<PlayerService>> {
        self.player_service.as_ref()
    }

    pub fn lyrics(&self) -> &Arc<LyricsService> {
        &self.lyrics
    }

    pub fn library(&self) -> Option<&LibraryService> {
        self.library.as_ref()
    }

    pub fn library_mut(&mut self) -> Option<&mut LibraryService> {
        self.library.as_mut()
    }

    pub fn has_library(&self) -> bool {
        self.library.is_some()
    }

    pub async fn attach_library(
        &mut self,
        options: LibrarySessionOptions,
    ) -> Result<&LibraryService> {
        let catalog = PlayerCatalog::open(&options.db_path).await?;
        let service = LibraryService::new(options.db_path).await?;
        catalog.ensure_local_source().await?;
        let player_service = Arc::new(PlayerService::new(
            catalog,
            self.player.clone(),
            Arc::new(service.handle().clone()),
            Arc::new(TypeScriptSourceResolverFactory::new(
                shared_typescript_runtime(),
            )),
        ));
        player_service.start_state_writer();
        if let Err(error) = player_service.restore().await {
            tracing::warn!(%error, "player state restore skipped");
        }
        self.player_service = Some(player_service);
        self.library = Some(service);
        Ok(self.library.as_ref().expect("library just initialized"))
    }

    pub fn detach_library(&mut self) {
        self.player_service = None;
        self.library = None;
    }
}

impl Default for BackendSession {
    fn default() -> Self {
        Self::new()
    }
}
