use serde::{Deserialize, Serialize};

pub const DECODER_TYPE_ID: &str = "ffmpeg_decode";
pub const ENCODER_TYPE_ID: &str = "ffmpeg_encode";
pub const DECODER_DISPLAY_NAME: &str = "FFmpeg Decoder (Sidecar)";
pub const ENCODER_DISPLAY_NAME: &str = "FFmpeg Encoder (Sidecar)";

pub const CONFIG_SCHEMA_JSON: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "ffmpeg_path": { "type": ["string", "null"], "default": "bin/ffmpeg" },
    "ffprobe_path": { "type": ["string", "null"], "default": "bin/ffprobe" },
    "ffmpeg_args": {
      "type": "array",
      "items": { "type": "string" },
      "default": []
    },
    "ffprobe_args": {
      "type": "array",
      "items": { "type": "string" },
      "default": []
    },
    "probe_timeout_ms": { "type": "integer", "minimum": 200, "maximum": 30000, "default": 3000 }
  }
}"#;

pub const DEFAULT_CONFIG_JSON: &str = r#"{
  "ffmpeg_path": "bin/ffmpeg",
  "ffprobe_path": "bin/ffprobe",
  "ffmpeg_args": [],
  "ffprobe_args": [],
  "probe_timeout_ms": 3000
}"#;

const DEFAULT_FFMPEG_EXECUTABLE: &str = "bin/ffmpeg";
const DEFAULT_FFPROBE_EXECUTABLE: &str = "bin/ffprobe";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FfmpegPluginConfig {
    pub ffmpeg_path: Option<String>,
    pub ffprobe_path: Option<String>,
    pub ffmpeg_args: Vec<String>,
    pub ffprobe_args: Vec<String>,
    pub probe_timeout_ms: u32,
}

impl Default for FfmpegPluginConfig {
    fn default() -> Self {
        Self {
            ffmpeg_path: Some(DEFAULT_FFMPEG_EXECUTABLE.to_string()),
            ffprobe_path: Some(DEFAULT_FFPROBE_EXECUTABLE.to_string()),
            ffmpeg_args: Vec::new(),
            ffprobe_args: Vec::new(),
            probe_timeout_ms: 3000,
        }
    }
}

impl FfmpegPluginConfig {
    pub fn ffmpeg_executable(&self) -> String {
        normalized_executable(self.ffmpeg_path.as_deref(), DEFAULT_FFMPEG_EXECUTABLE)
    }

    pub fn ffprobe_executable(&self) -> String {
        normalized_executable(self.ffprobe_path.as_deref(), DEFAULT_FFPROBE_EXECUTABLE)
    }

    pub fn normalized_ffmpeg_args(&self) -> Vec<String> {
        normalized_arg_list(self.ffmpeg_args.as_slice())
    }

    pub fn normalized_ffprobe_args(&self) -> Vec<String> {
        normalized_arg_list(self.ffprobe_args.as_slice())
            .into_iter()
            .filter(|arg| !arg.eq_ignore_ascii_case("-nostdin"))
            .collect()
    }

    pub fn clamped_probe_timeout_ms(&self) -> u32 {
        self.probe_timeout_ms.clamp(200, 30_000)
    }
}

fn normalized_executable(raw: Option<&str>, fallback: &str) -> String {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn normalized_arg_list(raw: &[String]) -> Vec<String> {
    raw.iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::FfmpegPluginConfig;

    #[test]
    fn normalized_ffprobe_args_ignores_nostdin() {
        let config = FfmpegPluginConfig {
            ffprobe_args: vec![
                "-nostdin".to_string(),
                " -NoStdin ".to_string(),
                "-v".to_string(),
                "error".to_string(),
            ],
            ..FfmpegPluginConfig::default()
        };

        assert_eq!(
            config.normalized_ffprobe_args(),
            vec!["-v".to_string(), "error".to_string()]
        );
    }
}
