use super::model::LogRecord;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

const FILE_BYTES: u64 = 10 * 1024 * 1024;
const TOTAL_BYTES: u64 = 100 * 1024 * 1024;

pub(super) struct LogFiles {
    root: PathBuf,
    session: String,
    part: u32,
    file: File,
    bytes: u64,
}
impl LogFiles {
    pub fn open(root: PathBuf, session: &str) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        cleanup(&root, None)?;
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(root.join(format!("{session}-00000.jsonl")))?;
        Ok(Self {
            root,
            session: session.into(),
            part: 0,
            file,
            bytes: 0,
        })
    }
    pub fn append(&mut self, record: &LogRecord) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(record)?;
        bytes.push(b'\n');
        if bytes.len() as u64 > FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "one diagnostic record exceeds the 10 MiB segment limit",
            ));
        }
        if self.bytes > 0 && self.bytes + bytes.len() as u64 > FILE_BYTES {
            self.file.flush()?;
            self.part += 1;
            let path = self
                .root
                .join(format!("{}-{:05}.jsonl", self.session, self.part));
            self.file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)?;
            self.bytes = 0;
            cleanup(&self.root, Some(&path))?;
        }
        self.file.write_all(&bytes)?;
        self.bytes += bytes.len() as u64;
        Ok(())
    }
    pub fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

pub(super) fn paths(root: &Path, session: Option<&str>) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("session-")
            && name.ends_with(".jsonl")
            && session.is_none_or(|s| name.starts_with(&format!("{s}-")))
        {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

pub(super) fn read(
    root: &Path,
    session: Option<&str>,
    mut visit: impl FnMut(LogRecord) -> bool,
) -> io::Result<()> {
    for path in paths(root, session)? {
        // A concurrent retention pass may remove an old segment.
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        for line in BufReader::new(file).lines() {
            if let Ok(record) = serde_json::from_str::<LogRecord>(&line?)
                && !visit(record)
            {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn cleanup(root: &Path, active: Option<&Path>) -> io::Result<()> {
    let mut retained = Vec::new();
    for path in paths(root, None)? {
        let metadata = fs::metadata(&path)?;
        if active != Some(path.as_path())
            && SystemTime::now()
                .duration_since(metadata.modified()?)
                .unwrap_or_default()
                > Duration::from_secs(7 * 86400)
        {
            fs::remove_file(path)?;
        } else {
            retained.push((path, metadata.len()));
        }
    }
    let mut total: u64 = retained.iter().map(|(_, n)| n).sum();
    // Reserve one segment for the active writer.
    for (path, bytes) in retained {
        if total <= TOTAL_BYTES - FILE_BYTES {
            break;
        }
        if active != Some(path.as_path()) {
            fs::remove_file(path)?;
            total -= bytes;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retention_removes_expired_segments_and_caps_total_size() {
        let root = tempfile::tempdir().unwrap();
        let expired = root.path().join("session-000-expired-00000.jsonl");
        let file = File::create(&expired).unwrap();
        file.set_times(
            fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(8 * 86400)),
        )
        .unwrap();
        drop(file);
        for i in 1..=11 {
            let path = root.path().join(format!("session-{i:03}-00000.jsonl"));
            File::create(path).unwrap().set_len(FILE_BYTES).unwrap();
        }
        cleanup(root.path(), None).unwrap();
        assert!(!expired.exists());
        assert!(
            paths(root.path(), None)
                .unwrap()
                .iter()
                .map(|p| fs::metadata(p).unwrap().len())
                .sum::<u64>()
                <= TOTAL_BYTES - FILE_BYTES
        );
    }
}
