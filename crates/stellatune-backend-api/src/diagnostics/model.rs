use serde::{Deserialize, Serialize};

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogRecord {
    pub id: String,
    pub session: String,
    pub timestamp_ms: f64,
    pub level: String,
    pub source: String,
    pub target: String,
    pub message: String,
    pub details: String,
    pub plugin_id: Option<String>,
    pub generation: Option<String>,
    pub fingerprint: Option<String>,
}

impl LogRecord {
    pub fn new(source: &str, level: &str, target: &str, message: String, details: String) -> Self {
        Self {
            id: String::new(),
            session: String::new(),
            timestamp_ms: super::now_ms(),
            level: level.into(),
            source: source.into(),
            target: target.into(),
            message,
            details,
            plugin_id: None,
            generation: None,
            fingerprint: None,
        }
    }
    pub fn bytes(&self) -> usize {
        self.message.len() + self.details.len() + self.target.len() + 512
    }
    pub fn summary(mut self) -> Self {
        self.details.clear();
        if self.message.len() > 2048 {
            let mut end = 2048;
            while !self.message.is_char_boundary(end) {
                end -= 1;
            }
            self.message.truncate(end);
            self.message.push('…');
        }
        self
    }
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Clone, Debug)]
pub struct LogBatch {
    pub records: Vec<LogRecord>,
    pub resync: bool,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Clone, Debug)]
pub struct LogPage {
    pub records: Vec<LogRecord>,
    pub next_offset: Option<u32>,
}
