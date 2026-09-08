use crate::frb_generated::StreamSink;
use std::path::PathBuf;
pub use stellatune_backend_api::diagnostics::model::{LogBatch, LogPage, LogRecord};
use stellatune_backend_api::diagnostics::shared;

pub fn diagnostics_initialize(log_dir: String) -> Result<String, super::error::AppError> {
    stellatune_backend_api::runtime::init_tracing();
    if let Err(error) = shared().configure(PathBuf::from(log_dir)) {
        shared().record(LogRecord::new(
            "rust",
            "ERROR",
            "diagnostics",
            "Diagnostic storage unavailable; using memory and console".into(),
            error,
        ));
    }
    Ok(shared().session_id().to_owned())
}
pub fn diagnostics_append(records: Vec<LogRecord>) {
    for mut record in records.into_iter().take(200) {
        record.source = "flutter".into();
        shared().record(record);
    }
}
pub fn diagnostics_events(sink: StreamSink<LogBatch>) {
    let mut receiver = shared().subscribe();
    let records = shared().snapshot();
    if sink
        .add(LogBatch {
            records,
            resync: true,
        })
        .is_err()
    {
        return;
    }
    crate::background_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            let mut records = Vec::new();
            let mut resync = false;
            while records.len() < 200 {
                match receiver.try_recv() {
                    Ok(record) => records.push(record),
                    Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                        records = shared().snapshot();
                        resync = true;
                        break;
                    },
                    Err(tokio::sync::broadcast::error::TryRecvError::Closed) => return,
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                }
            }
            // Empty batches also detect a closed Dart stream during idle periods.
            if sink.add(LogBatch { records, resync }).is_err() {
                break;
            }
        }
    });
}
pub fn diagnostics_sessions() -> Result<Vec<String>, super::error::AppError> {
    shared()
        .sessions()
        .map_err(|e| super::error::AppError::message("diagnostics_sessions", e))
}
pub fn diagnostics_query(
    session: String,
    offset: u32,
    limit: u32,
    level: String,
    source: String,
    search: String,
) -> Result<LogPage, super::error::AppError> {
    shared()
        .query(&session, offset, limit, &level, &source, &search)
        .map_err(|e| super::error::AppError::message("diagnostics_query", e))
}
pub fn diagnostics_detail(id: String) -> Result<LogRecord, super::error::AppError> {
    shared()
        .detail(&id)
        .map_err(|e| super::error::AppError::message("diagnostics_detail", e))
}
pub fn diagnostics_export(
    session: String,
    destination: String,
) -> Result<(), super::error::AppError> {
    shared()
        .export(&session, PathBuf::from(destination))
        .map_err(|e| super::error::AppError::message("diagnostics_export", e))
}
pub fn diagnostics_flush() {
    shared().flush();
}
