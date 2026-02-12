use anyhow::Result;

use crate::library::LibraryService;
use crate::lyrics_service::LyricsService;
use crate::runtime::shared_runtime_engine;
use std::sync::Arc;
use stellatune_audio::EngineHandle;

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
    player: Arc<EngineHandle>,
    lyrics: Arc<LyricsService>,
    library: Option<LibraryService>,
}

impl BackendSession {
    pub fn new() -> Self {
        Self {
            player: shared_runtime_engine(),
            lyrics: LyricsService::new(),
            library: None,
        }
    }

    pub fn from_options(options: BackendSessionOptions) -> Result<Self> {
        let player = shared_runtime_engine();
        let lyrics = LyricsService::new();
        let library = match options.library {
            Some(opts) => Some(LibraryService::new(opts.db_path)?),
            None => None,
        };
        Ok(Self {
            player,
            lyrics,
            library,
        })
    }

    pub fn player(&self) -> &Arc<EngineHandle> {
        &self.player
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

    pub fn attach_library(&mut self, options: LibrarySessionOptions) -> Result<&LibraryService> {
        let service = LibraryService::new(options.db_path)?;
        self.library = Some(service);
        Ok(self.library.as_ref().expect("library just initialized"))
    }

    pub fn detach_library(&mut self) {
        self.library = None;
    }
}

impl Default for BackendSession {
    fn default() -> Self {
        Self::new()
    }
}
