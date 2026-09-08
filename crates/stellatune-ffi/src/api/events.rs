use super::error::AppError;
use stellatune_backend_api::LyricsDoc;

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Clone, Debug)]
pub enum LibraryEvent {
    Changed,
    ScanProgress {
        scanned: i64,
        updated: i64,
        skipped: i64,
        errors: i64,
    },
    ScanFinished {
        duration_ms: i64,
        scanned: i64,
        updated: i64,
        skipped: i64,
        errors: i64,
    },
    Error {
        error: AppError,
    },
    Log {
        message: String,
    },
}
impl From<stellatune_library::LibraryEvent> for LibraryEvent {
    fn from(event: stellatune_library::LibraryEvent) -> Self {
        use stellatune_library::LibraryEvent as Event;
        match event {
            Event::Changed => Self::Changed,
            Event::ScanProgress {
                scanned,
                updated,
                skipped,
                errors,
            } => Self::ScanProgress {
                scanned,
                updated,
                skipped,
                errors,
            },
            Event::ScanFinished {
                duration_ms,
                scanned,
                updated,
                skipped,
                errors,
            } => Self::ScanFinished {
                duration_ms,
                scanned,
                updated,
                skipped,
                errors,
            },
            Event::Error { message } => Self::Error {
                error: AppError::message("library_event", message),
            },
            Event::Log { message } => Self::Log { message },
        }
    }
}
#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Clone, Debug)]
pub enum LyricsEvent {
    Loading { track_key: String },
    Ready { track_key: String, doc: LyricsDoc },
    Cursor { track_key: String, line_index: i64 },
    Empty { track_key: String },
    Error { track_key: String, error: AppError },
}
impl From<stellatune_backend_api::LyricsEvent> for LyricsEvent {
    fn from(event: stellatune_backend_api::LyricsEvent) -> Self {
        use stellatune_backend_api::LyricsEvent as Event;
        match event {
            Event::Loading { track_key } => Self::Loading { track_key },
            Event::Ready { track_key, doc } => Self::Ready { track_key, doc },
            Event::Cursor {
                track_key,
                line_index,
            } => Self::Cursor {
                track_key,
                line_index,
            },
            Event::Empty { track_key } => Self::Empty { track_key },
            Event::Error { track_key, message } => Self::Error {
                track_key,
                error: AppError::message("lyrics_event", message),
            },
        }
    }
}
