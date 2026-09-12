//! Built-in candidate formats. Actual codec availability is checked after demuxing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinDecoderScoreRule {
    pub ext: &'static str,
    pub score: u16,
}
macro_rules! formats {
    ($($ext:literal),* $(,)?) => { &[$(BuiltinDecoderScoreRule { ext: $ext, score: 90 }),*] };
}
pub const BUILTIN_DECODER_SCORE_RULES: &[BuiltinDecoderScoreRule] = formats![
    "mp1", "mp2", "mp3", "mpa", "aac", "alac", "m4a", "m4b", "m4r", "m4p", "mp4", "mov", "3gp",
    "3g2", "caf", "flac", "wav", "wave", "aif", "aiff", "aifc", "ogg", "oga",
];
pub fn auto_scan_extension(ext: &str) -> bool {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    !matches!(ext.as_str(), "mp4" | "mov" | "3gp" | "3g2" | "m4p")
        && BUILTIN_DECODER_SCORE_RULES.iter().any(|r| r.ext == ext)
}
