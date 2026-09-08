//! Bounded, process-wide diagnostics. Producers never wait for filesystem I/O.
mod files;
pub mod model;
#[cfg(test)]
mod tests;
mod tracing_layer;
pub use model::{LogBatch, LogPage, LogRecord};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::broadcast;
pub use tracing_layer::DiagnosticsLayer;

const CACHE_BYTES: usize = 8 * 1024 * 1024;
enum Command {
    Record(LogRecord),
    Configure(PathBuf, mpsc::Sender<Result<(), String>>),
    Flush(mpsc::Sender<()>),
}
pub struct Diagnostics {
    session: String,
    sequence: AtomicU64,
    sender: mpsc::SyncSender<Command>,
    queued_bytes: AtomicUsize,
    dropped: AtomicU64,
    cache: Mutex<VecDeque<LogRecord>>,
    root: Mutex<Option<PathBuf>>,
    events: broadcast::Sender<LogRecord>,
}
pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}
pub fn shared() -> &'static Arc<Diagnostics> {
    static INSTANCE: OnceLock<Arc<Diagnostics>> = OnceLock::new();
    INSTANCE.get_or_init(start)
}
fn start() -> Arc<Diagnostics> {
    let (sender, receiver) = mpsc::sync_channel(1024);
    let service = Arc::new(Diagnostics {
        session: format!(
            "session-{}-{}-{}",
            now_ms() as u64,
            std::process::id(),
            rand::random::<u32>()
        ),
        sequence: AtomicU64::new(1),
        sender,
        queued_bytes: AtomicUsize::new(0),
        dropped: AtomicU64::new(0),
        cache: Mutex::new(VecDeque::new()),
        root: Mutex::new(None),
        events: broadcast::channel(1024).0,
    });
    let worker = Arc::clone(&service);
    std::thread::Builder::new()
        .name("stellatune-diagnostics".into())
        .spawn(move || worker.run(receiver))
        .expect("start diagnostics worker");
    service
}
impl Diagnostics {
    pub fn session_id(&self) -> &str {
        &self.session
    }
    pub fn record(&self, mut record: LogRecord) -> String {
        if record.source != "flutter" || record.id.is_empty() {
            record.id = format!(
                "{}:{}",
                self.session,
                self.sequence.fetch_add(1, Ordering::Relaxed)
            );
        }
        record.session = self.session.clone();
        let id = record.id.clone();
        // Redact before either persistence or broadcast.
        record.message = redact(&record.message);
        record.details = redact(&record.details);
        let bytes = record.bytes();
        if self
            .queued_bytes
            .fetch_add(bytes, Ordering::Relaxed)
            .saturating_add(bytes)
            > CACHE_BYTES
            || self.sender.try_send(Command::Record(record)).is_err()
        {
            self.queued_bytes.fetch_sub(bytes, Ordering::Relaxed);
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        id
    }
    pub fn subscribe(&self) -> broadcast::Receiver<LogRecord> {
        self.events.subscribe()
    }
    pub fn snapshot(&self) -> Vec<LogRecord> {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .map(LogRecord::summary)
            .collect()
    }
    pub fn configure(&self, root: PathBuf) -> Result<String, String> {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(Command::Configure(root, tx))
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())??;
        Ok(self.session.clone())
    }
    pub fn flush(&self) {
        let (tx, rx) = mpsc::channel();
        if self.sender.send(Command::Flush(tx)).is_ok() {
            let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
        }
    }
    fn root(&self) -> Result<PathBuf, String> {
        self.root
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| "diagnostic storage is not initialized".into())
    }
    pub fn sessions(&self) -> Result<Vec<String>, String> {
        if self.root().is_err() {
            return Ok(vec![self.session.clone()]);
        }
        let mut sessions: Vec<_> = files::paths(&self.root()?, None)
            .map_err(|e| e.to_string())?
            .iter()
            .filter_map(|p| {
                p.file_stem()?
                    .to_str()?
                    .rsplit_once('-')
                    .map(|(s, _)| s.to_owned())
            })
            .collect();
        sessions.push(self.session.clone());
        sessions.sort();
        sessions.dedup();
        sessions.reverse();
        Ok(sessions)
    }
    pub fn query(
        &self,
        session: &str,
        offset: u32,
        limit: u32,
        level: &str,
        source: &str,
        search: &str,
    ) -> Result<LogPage, String> {
        self.flush();
        let mut matched = 0u32;
        let mut records = Vec::new();
        let mut more = false;
        let limit = limit.clamp(1, 200) as usize;
        let search = search.to_lowercase();
        self.read_records(Some(session), |record| {
            if (!level.is_empty() && record.level != level)
                || (!source.is_empty() && record.source != source)
                || (!search.is_empty()
                    && !format!("{} {} {}", record.message, record.details, record.target)
                        .to_lowercase()
                        .contains(&search))
            {
                return true;
            }
            matched += 1;
            if matched <= offset {
                return true;
            }
            if records.len() == limit {
                more = true;
                return false;
            }
            records.push(record.summary());
            true
        })
        .map_err(|e| e.to_string())?;
        Ok(LogPage {
            next_offset: more.then_some(offset + records.len() as u32),
            records,
        })
    }
    pub fn detail(&self, id: &str) -> Result<LogRecord, String> {
        self.flush();
        if let Some(record) = self
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|r| r.id == id)
        {
            return Ok(record.clone());
        }
        let mut found = None;
        self.read_records(
            id.split_once(':')
                .map(|(s, _)| s)
                .filter(|s| *s != "flutter"),
            |r| {
                if r.id == id {
                    found = Some(r);
                    false
                } else {
                    true
                }
            },
        )
        .map_err(|e| e.to_string())?;
        found.ok_or_else(|| "log record expired or was not persisted".into())
    }
    pub fn export(&self, session: &str, destination: PathBuf) -> Result<(), String> {
        use std::io::Write;
        self.flush();
        let mut file = std::fs::File::create(destination).map_err(|e| e.to_string())?;
        let mut error = None;
        self.read_records(Some(session), |r| {
            if let Err(e) = writeln!(
                file,
                "{} {} [{}] {}\nID: {}\nPlugin: {:?}, generation: {:?}\n{}\n{}\n",
                r.timestamp_ms,
                r.level,
                r.source,
                r.target,
                r.id,
                r.plugin_id,
                r.generation,
                r.message,
                r.details
            ) {
                error = Some(e);
                return false;
            }
            true
        })
        .map_err(|e| e.to_string())?;
        error.map_or(Ok(()), |e| Err(e.to_string()))
    }
    fn read_records(
        &self,
        session: Option<&str>,
        mut visit: impl FnMut(LogRecord) -> bool,
    ) -> std::io::Result<()> {
        if let Ok(root) = self.root() {
            return files::read(&root, session, visit);
        }
        let records: Vec<_> = self
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect();
        for record in records {
            if session.is_none_or(|s| s == record.session) && !visit(record) {
                break;
            }
        }
        Ok(())
    }
    fn accept(&self, record: LogRecord, files: &mut Option<files::LogFiles>) {
        if let Some(writer) = files
            && let Err(error) = writer.append(&record)
        {
            eprintln!("diagnostic file unavailable: {error}");
            *files = None;
            self.accept(
                LogRecord {
                    id: format!("{}:storage", self.session),
                    session: self.session.clone(),
                    ..LogRecord::new(
                        "rust",
                        "ERROR",
                        "diagnostics",
                        "Diagnostic file unavailable; retaining memory and console logs".into(),
                        error.to_string(),
                    )
                },
                files,
            );
        }
        let summary = record.clone().summary();
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.push_back(record);
        let mut bytes: usize = cache.iter().map(LogRecord::bytes).sum();
        while cache.len() > 5000 || bytes > CACHE_BYTES {
            if let Some(old) = cache.pop_front() {
                bytes = bytes.saturating_sub(old.bytes());
            } else {
                break;
            }
        }
        drop(cache);
        let _ = self.events.send(summary);
    }
    fn run(&self, receiver: mpsc::Receiver<Command>) {
        let mut files = None;
        while let Ok(command) = receiver.recv() {
            match command {
                Command::Record(record) => {
                    self.queued_bytes
                        .fetch_sub(record.bytes(), Ordering::Relaxed);
                    self.accept(record, &mut files);
                },
                Command::Configure(root, reply) => {
                    let result = if self.root.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
                        == Some(&root)
                    {
                        Ok(())
                    } else {
                        files::LogFiles::open(root.clone(), &self.session)
                            .and_then(|mut writer| {
                                for r in self.cache.lock().unwrap_or_else(|e| e.into_inner()).iter()
                                {
                                    writer.append(r)?;
                                }
                                files = Some(writer);
                                *self.root.lock().unwrap_or_else(|e| e.into_inner()) = Some(root);
                                Ok(())
                            })
                            .map_err(|e| e.to_string())
                    };
                    let _ = reply.send(result);
                },
                Command::Flush(reply) => {
                    self.report_dropped(&mut files);
                    if let Some(writer) = &mut files {
                        let _ = writer.flush();
                    }
                    let _ = reply.send(());
                },
            }
            self.report_dropped(&mut files);
        }
    }
    fn report_dropped(&self, files: &mut Option<files::LogFiles>) {
        let count = self.dropped.swap(0, Ordering::Relaxed);
        if count > 0 {
            self.accept(
                LogRecord {
                    id: format!(
                        "{}:dropped:{}",
                        self.session,
                        self.sequence.fetch_add(1, Ordering::Relaxed)
                    ),
                    session: self.session.clone(),
                    ..LogRecord::new(
                        "rust",
                        "WARN",
                        "diagnostics",
                        format!("Dropped {count} log records: diagnostic queue capacity exceeded"),
                        String::new(),
                    )
                },
                files,
            );
        }
    }
}

pub fn redact(text: &str) -> String {
    // Header/config values may contain spaces, so redact to end of line.
    text.lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            let position = [
                "authorization",
                "cookie",
                "access_token",
                "refresh_token",
                "password",
                "passwd",
                "token=",
                "token:",
                "\"token\"",
                "api_key",
            ]
            .iter()
            .filter_map(|key| lower.find(key))
            .min();
            position.map_or_else(|| line.to_owned(), |p| format!("{}[redacted]", &line[..p]))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
