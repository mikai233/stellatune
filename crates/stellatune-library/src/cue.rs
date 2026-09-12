//! External CUE documents. Times remain CD frames until an audio file is probed.
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CueTrack {
    pub number: u32,
    pub file: String,
    pub title: Option<String>,
    pub performer: Option<String>,
    pub index: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CueSheet {
    pub title: Option<String>,
    pub performer: Option<String>,
    pub disc_number: Option<i64>,
    pub date: Option<String>,
    pub tracks: Vec<CueTrack>,
}

fn quoted(value: &str) -> Result<String> {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix('"') {
        Ok(rest
            .strip_suffix('"')
            .context("unterminated CUE string")?
            .to_owned())
    } else {
        Ok(value.to_owned())
    }
}

fn time(value: &str) -> Result<u64> {
    let parts = value
        .split(':')
        .map(str::parse::<u64>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if parts.len() != 3 || parts[1] >= 60 || parts[2] >= 75 {
        bail!("invalid CUE index: {value}");
    }
    parts[0]
        .checked_mul(60)
        .and_then(|v| v.checked_add(parts[1]))
        .and_then(|v| v.checked_mul(75))
        .and_then(|v| v.checked_add(parts[2]))
        .context("CUE time overflow")
}

pub fn sample_frame(cd_frame: u64, sample_rate: u32) -> Result<u64> {
    if sample_rate == 0 {
        bail!("unknown sample rate");
    }
    u64::try_from(u128::from(cd_frame) * u128::from(sample_rate) / 75)
        .context("CUE sample offset overflow")
}

pub fn parse(text: &str) -> Result<CueSheet> {
    if text.len() > 4 * 1024 * 1024 {
        bail!("CUE document exceeds 4 MiB");
    }
    let mut sheet = CueSheet::default();
    let mut file = None;
    let mut numbers = HashSet::new();
    let mut indexed = false;
    let mut index_zero = None;
    for (line_number, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (command, value) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match command.to_ascii_uppercase().as_str() {
            "FILE" => {
                let (name, _) = value
                    .trim()
                    .rsplit_once(char::is_whitespace)
                    .context("FILE requires a type")?;
                let name = quoted(name)?;
                if name.is_empty() {
                    bail!("empty FILE");
                }
                file = Some(name);
            },
            "TRACK" => {
                if !sheet.tracks.is_empty() && !indexed {
                    bail!("missing INDEX 01");
                }
                let mut parts = value.split_whitespace();
                let number: u32 = parts.next().context("missing TRACK number")?.parse()?;
                if number == 0 || !numbers.insert(number) || sheet.tracks.len() >= 999 {
                    bail!("invalid or duplicate TRACK number");
                }
                if !parts
                    .next()
                    .is_some_and(|v| v.eq_ignore_ascii_case("AUDIO"))
                {
                    bail!("only AUDIO tracks are supported");
                }
                sheet.tracks.push(CueTrack {
                    number,
                    file: file.clone().context("TRACK precedes FILE")?,
                    title: None,
                    performer: None,
                    index: 0,
                });
                indexed = false;
                index_zero = None;
            },
            "TITLE" | "PERFORMER" => {
                let value = quoted(value)?;
                let target = match (
                    sheet.tracks.last_mut(),
                    command.eq_ignore_ascii_case("TITLE"),
                ) {
                    (Some(track), true) => &mut track.title,
                    (Some(track), false) => &mut track.performer,
                    (None, true) => &mut sheet.title,
                    (None, false) => &mut sheet.performer,
                };
                *target = (!value.trim().is_empty()).then_some(value);
            },
            "INDEX" => {
                let (number, value) = value
                    .trim()
                    .split_once(char::is_whitespace)
                    .context("invalid INDEX")?;
                let index = time(value.trim())?;
                let track = sheet.tracks.last_mut().context("INDEX precedes TRACK")?;
                match number.parse::<u32>()? {
                    0 => index_zero = Some(index),
                    1 => {
                        if indexed || index_zero.is_some_and(|v| v > index) {
                            bail!("invalid INDEX order");
                        }
                        track.index = index;
                        indexed = true;
                    },
                    _ => {},
                }
            },
            "REM" => {
                let (key, value) = value
                    .trim()
                    .split_once(char::is_whitespace)
                    .unwrap_or((value, ""));
                if key.eq_ignore_ascii_case("DISCNUMBER") {
                    sheet.disc_number = quoted(value)?.parse::<i64>().ok().filter(|v| *v > 0);
                }
                if key.eq_ignore_ascii_case("DATE") {
                    sheet.date = Some(quoted(value)?);
                }
            },
            // These do not change the existing PCM timeline. Do not synthesize gaps.
            "PREGAP" | "POSTGAP" => {
                time(value.trim())?;
            },
            "CATALOG" | "ISRC" | "FLAGS" | "CDTEXTFILE" => {},
            _ => bail!(
                "unsupported CUE command on line {}: {command}",
                line_number + 1
            ),
        }
    }
    if sheet.tracks.is_empty() || !indexed {
        bail!("CUE has no complete audio tracks");
    }
    let mut previous = std::collections::HashMap::new();
    for track in &sheet.tracks {
        if previous
            .insert(&track.file, track.index)
            .is_some_and(|p| p >= track.index)
        {
            bail!("CUE indices must increase within each audio file");
        }
    }
    Ok(sheet)
}

/// Resolve a FILE exactly, or an unambiguous same-stem audio file after conversion.
pub fn resolve_file(directory: &Path, name: &str) -> Result<PathBuf> {
    let normalized = name.replace('\\', "/");
    let exact = directory.join(&normalized);
    if exact.is_file() {
        return Ok(exact);
    }
    let parent = exact.parent().context("FILE has no parent")?;
    let stem = exact.file_stem().context("FILE has no name")?;
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(parent)? {
        let path = entry?.path();
        if path.is_file()
            && path.file_stem() == Some(stem)
            && path
                .extension()
                .and_then(|v| v.to_str())
                .is_some_and(stellatune_media_probe::formats::auto_scan_extension)
        {
            candidates.push(path);
        }
    }
    if candidates.len() != 1 {
        bail!("missing or ambiguous CUE FILE: {name}");
    }
    tracing::warn!(file = name, replacement = %candidates[0].display(), "CUE FILE extension differs from audio file");
    Ok(candidates.remove(0))
}

pub fn read(path: &Path) -> Result<CueSheet> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4 * 1024 * 1024 {
        bail!("CUE document exceeds 4 MiB");
    }
    if let Some((encoding, offset)) = encoding_rs::Encoding::for_bom(&bytes) {
        let (text, errors) = encoding.decode_without_bom_handling(&bytes[offset..]);
        if errors {
            bail!("invalid CUE text encoding");
        }
        return parse(&text);
    }
    if let Ok(text) = std::str::from_utf8(&bytes) {
        return parse(text);
    }
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(&bytes, true);
    let (guessed, assessed) = detector.guess_assess(None, true);
    let mut candidates = Vec::new();
    for encoding in [
        guessed,
        encoding_rs::SHIFT_JIS,
        encoding_rs::GBK,
        encoding_rs::BIG5,
    ] {
        let (text, errors) = encoding.decode_without_bom_handling(&bytes);
        if errors {
            continue;
        }
        if let Ok(sheet) = parse(&text) {
            let directory = path.parent().context("CUE has no parent")?;
            let files: HashSet<_> = sheet.tracks.iter().map(|t| &t.file).collect();
            let matches = files
                .iter()
                .filter(|file| resolve_file(directory, file).is_ok())
                .count();
            if !candidates.iter().any(|(_, _, previous)| previous == &sheet) {
                candidates.push((
                    matches,
                    assessed
                        && encoding == guessed
                        && bytes.iter().filter(|b| **b >= 0x80).count() >= 8,
                    sheet,
                ));
            }
        }
    }
    candidates.sort_by_key(|(matches, detected, _)| std::cmp::Reverse((*matches, *detected)));
    if candidates.is_empty()
        || candidates[0].0 == 0
        || (candidates.len() > 1
            && candidates[0].0 == candidates[1].0
            && candidates[0].1 == candidates[1].1)
    {
        bail!("cannot reliably determine CUE encoding and referenced files");
    }
    Ok(candidates.remove(0).2)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chinese_encoding_with_ascii_filename_uses_text_detection() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("album.wav"), []).unwrap();
        let text = "TITLE \"音乐专辑\"\nPERFORMER \"周杰伦\"\nFILE \"album.wav\" WAVE\nTRACK 01 AUDIO\nTITLE \"青花瓷\"\nINDEX 01 00:00:00";
        let (bytes, _, errors) = encoding_rs::GBK.encode(text);
        assert!(!errors);
        let path = dir.path().join("album.cue");
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            read(&path).unwrap().tracks[0].title.as_deref(),
            Some("青花瓷")
        );
    }
    #[test]
    fn multiple_files_and_exact_cd_frames() {
        let sheet = parse("TITLE \"Album\"\nPERFORMER \"A / B\"\nFILE \"one.wav\" WAVE\nTRACK 01 AUDIO\nTITLE \"First\"\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nINDEX 00 00:01:00\nINDEX 01 00:01:01\nFILE \"two.flac\" WAVE\nTRACK 03 AUDIO\nINDEX 01 00:00:00").unwrap();
        assert_eq!(sheet.tracks.len(), 3);
        assert_eq!(sheet.tracks[1].index, 76);
        assert_eq!(sample_frame(76, 44100).unwrap(), 44688);
        assert_eq!(sample_frame(1, 48000).unwrap(), 640);
        assert_eq!(sheet.performer.as_deref(), Some("A / B"));
    }
    #[test]
    fn rejects_incomplete_and_invalid_documents() {
        for body in [
            "TRACK 01 AUDIO",
            "FILE \"x.wav\" WAVE\nTRACK 01 AUDIO",
            "FILE \"x.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:75",
            "FILE \"x.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:01:00\nTRACK 02 AUDIO\nINDEX 01 00:00:00",
        ] {
            assert!(parse(body).is_err(), "{body}");
        }
    }
    #[test]
    fn japanese_encoding_and_converted_filename() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("初音ミク.flac"), []).unwrap();
        let text =
            "FILE \"初音ミク.wav\" WAVE\nTRACK 01 AUDIO\nTITLE \"初音ミク\"\nINDEX 01 00:00:00";
        let (bytes, _, errors) = encoding_rs::SHIFT_JIS.encode(text);
        assert!(!errors);
        let path = dir.path().join("album.cue");
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            read(&path).unwrap().tracks[0].title.as_deref(),
            Some("初音ミク")
        );
    }
}
