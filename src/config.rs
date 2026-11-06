use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_bat_path")]
    pub bat_path: String,
    #[serde(default = "default_true")]
    pub use_bat: bool,
    #[serde(default = "default_download_dir")]
    pub download_dir: String,
    #[serde(default)]
    pub show_console: bool,
    #[serde(default)]
    pub keep_console_open: bool,
    #[serde(default = "default_auto")]
    pub cookies_from_browser: String,
    #[serde(default = "default_cookies_file")]
    pub cookies_file: String,
    #[serde(default = "default_video")]
    pub default_type: String,
    #[serde(default = "default_best")]
    pub default_video_quality: String,
    #[serde(default = "default_mp3")]
    pub default_audio_format: String,
    #[serde(default = "default_true")]
    pub normalize_audio: bool,
    #[serde(default = "default_true")]
    pub embed_thumbnail: bool,
    #[serde(default = "default_true")]
    pub embed_metadata: bool,
    #[serde(default)]
    pub embed_subtitles: bool,
    #[serde(default)]
    pub whisper_enabled: bool,
    #[serde(default = "default_whisper_bin")]
    pub whisper_bin_path: String,
    #[serde(default = "default_whisper_models")]
    pub whisper_models_dir: String,
    #[serde(default = "default_whisper_model")]
    pub whisper_model: String,
    #[serde(default)]
    pub whisper_burn_subtitles: bool,
    #[serde(default)]
    pub whisper_save_md: bool,
    #[serde(default = "default_threads")]
    pub whisper_threads: u32,
    #[serde(default = "default_device")]
    pub whisper_device: String,
}

fn default_bat_path() -> String { "../yt-dlp/yt-dlp.bat".to_string() }
fn default_true() -> bool { true }
fn default_download_dir() -> String { "../../_Output".to_string() }
fn default_auto() -> String { "auto".to_string() }
fn default_cookies_file() -> String { "../www.youtube.com_cookies.txt".to_string() }
fn default_video() -> String { "video".to_string() }
fn default_best() -> String { "best".to_string() }
fn default_mp3() -> String { "mp3".to_string() }
fn default_whisper_bin() -> String { "E:/Coding/Whisper-AMD/bin/whisper-cli.exe".to_string() }
fn default_whisper_models() -> String { "E:/Coding/Whisper-AMD/bin/Whisper-models".to_string() }
fn default_whisper_model() -> String { "large-v3-turbo".to_string() }
fn default_threads() -> u32 { 8 }
fn default_device() -> String { "0".to_string() }

impl Default for Config {
    fn default() -> Self {
        Self {
            bat_path: default_bat_path(),
            use_bat: true,
            download_dir: default_download_dir(),
            show_console: false,
            keep_console_open: false,
            cookies_from_browser: default_auto(),
            cookies_file: default_cookies_file(),
            default_type: default_video(),
            default_video_quality: default_best(),
            default_audio_format: default_mp3(),
            normalize_audio: true,
            embed_thumbnail: true,
            embed_metadata: true,
            embed_subtitles: false,
            whisper_enabled: false,
            whisper_bin_path: default_whisper_bin(),
            whisper_models_dir: default_whisper_models(),
            whisper_model: default_whisper_model(),
            whisper_burn_subtitles: false,
            whisper_save_md: false,
            whisper_threads: default_threads(),
            whisper_device: default_device(),
        }
    }
}

pub fn get_config_path(base_dir: &Path) -> PathBuf {
    base_dir.join("config.json")
}

pub fn load_config(base_dir: &Path) -> Config {
    let p = get_config_path(base_dir);
    if p.is_file() {
        if let Ok(content) = std::fs::read_to_string(&p) {
            if let Ok(cfg) = serde_json::from_str::<Config>(&content) {
                return cfg;
            }
        }
    }
    Config::default()
}

pub fn save_config(base_dir: &Path, config: &Config) -> Result<()> {
    let p = get_config_path(base_dir);
    let s = serde_json::to_string_pretty(config)?;
    std::fs::write(p, s)?;
    Ok(())
}
