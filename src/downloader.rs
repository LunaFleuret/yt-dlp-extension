use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(default = "default_video")]
    pub download_type: String,
    #[serde(default = "default_best")]
    pub video_quality: String,
    #[serde(default = "default_mp4")]
    pub video_format: String,
    #[serde(default = "default_mp3")]
    pub audio_format: String,
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
    #[serde(default = "default_whisper_model")]
    pub whisper_model: String,
    #[serde(default)]
    pub whisper_burn_subtitles: bool,
    #[serde(default)]
    pub whisper_save_md: bool,
    #[serde(default = "default_whisper_bin")]
    pub whisper_bin_path: String,
    #[serde(default = "default_whisper_models")]
    pub whisper_models_dir: String,
    #[serde(default)]
    pub custom_args: String,
    #[serde(default)]
    pub download_dir: String,
    #[serde(default)]
    pub cookies_from_browser: String,
    #[serde(default)]
    pub cookies_file: String,
    #[serde(default)]
    pub show_console: bool,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub percent: f64,
    #[serde(default = "default_speed")]
    pub speed: String,
    #[serde(default = "default_eta")]
    pub eta: String,
    #[serde(default = "default_unknown")]
    pub total_size: String,
    pub error: Option<String>,
    pub created_at: f64,
    pub started_at: Option<f64>,
    pub finished_at: Option<f64>,
    pub pid: Option<u32>,
}

fn default_video() -> String { "video".to_string() }
fn default_best() -> String { "best".to_string() }
fn default_mp4() -> String { "mp4".to_string() }
fn default_mp3() -> String { "mp3".to_string() }
fn default_true() -> bool { true }
fn default_whisper_model() -> String { "large-v3-turbo".to_string() }
fn default_whisper_bin() -> String { "E:/Coding/Whisper-AMD/bin/whisper-cli.exe".to_string() }
fn default_whisper_models() -> String { "E:/Coding/Whisper-AMD/bin/Whisper-models".to_string() }
fn default_status() -> String { "queued".to_string() }
fn default_speed() -> String { "Starting...".to_string() }
fn default_eta() -> String { "--:--".to_string() }
fn default_unknown() -> String { "Unknown".to_string() }

pub fn update_task_file(task_file: &Path, data: &TaskInfo) {
    let tmp = task_file.with_extension("tmp");
    if let Ok(s) = serde_json::to_string_pretty(data) {
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, task_file);
        }
    }
}

pub fn sanitize_url(raw_url: &str) -> String {
    let u = raw_url.trim();
    let re = Regex::new(r"(?:youtube\.com/watch\?v=|youtu\.be/)([\w-]+)").unwrap();
    if let Some(caps) = re.captures(u) {
        if u.contains("list=") {
            let id = &caps[1];
            if u.contains("music.youtube.com") {
                return format!("https://music.youtube.com/watch?v={}", id);
            }
            return format!("https://www.youtube.com/watch?v={}", id);
        }
    }
    u.to_string()
}

pub fn cleanup_standalone_artifacts(output_dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ["vtt", "ttml", "part", "ytdl", "temp", "wav"].contains(&ext_lower.as_str()) {
                        let _ = std::fs::remove_file(&p);
                    } else if ["jpg", "jpeg", "png", "webp"].contains(&ext_lower.as_str()) {
                        if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                            if fname.starts_with("temp_") || fname.starts_with("cdn_thumbnail") {
                                let _ = std::fs::remove_file(&p);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub struct DownloadResult {
    pub success: bool,
    pub media_path: Option<PathBuf>,
    pub error: Option<String>,
}

pub fn execute_ytdlp_download(
    task_file: &Path,
    task: &mut TaskInfo,
    output_dir: &Path,
    base_dir: &Path,
) -> DownloadResult {
    let url = sanitize_url(&task.url);

    let progress_re = Regex::new(r"\[download\]\s+([0-9\.]+)%\s+of\s+~?([^\s]+)\s+at\s+([^\s]+)\s+ETA\s+([^\s]+)").unwrap();
    let progress_100_re = Regex::new(r"\[download\]\s+100%(?:\.0%)?\s+of\s+~?([^\s]+)").unwrap();
    let dest_re = Regex::new(r"\[(?:download|Merger|ExtractAudio)\]\s+Destination:\s+(.+)").unwrap();

    let mut args: Vec<String> = vec![
        "--newline".to_string(),
        "--windows-filenames".to_string(),
        "--no-mtime".to_string(),
        "--remote-components".to_string(),
        "ejs:github".to_string(),
        "-P".to_string(),
        output_dir.to_string_lossy().to_string(),
    ];

    if !(url.contains("youtube.com/playlist") && url.contains("list=")) {
        args.push("--no-playlist".to_string());
    }

    if !task.cookies_from_browser.is_empty() && task.cookies_from_browser.to_lowercase() != "auto" {
        args.push("--cookies-from-browser".to_string());
        args.push(task.cookies_from_browser.clone());
    } else if !task.cookies_file.is_empty() {
        let cookie_p = PathBuf::from(&task.cookies_file);
        let abs_cookie = if cookie_p.is_absolute() {
            cookie_p
        } else {
            base_dir.join(cookie_p)
        };
        if abs_cookie.is_file() {
            args.push("--cookies".to_string());
            args.push(abs_cookie.to_string_lossy().to_string());
        }
    }

    if task.download_type == "audio" {
        args.push("-x".to_string());
        args.push("--audio-format".to_string());
        args.push(task.audio_format.clone());
        args.push("--no-keep-video".to_string());

        if task.audio_format == "mp3" {
            args.push("--audio-quality".to_string());
            args.push("0".to_string());
        }
        if task.embed_thumbnail {
            args.push("--embed-thumbnail".to_string());
            args.push("--convert-thumbnails".to_string());
            args.push("jpg".to_string());
        }
        if task.embed_metadata {
            args.push("--embed-metadata".to_string());
            args.push("--parse-metadata".to_string());
            args.push("%(artist,creator,uploader,channel)s:%(artist)s".to_string());
            args.push("--parse-metadata".to_string());
            args.push("%(description)s:%(comment)s".to_string());
        }
    } else if task.download_type == "video" {
        let quality_arg = match task.video_quality.as_str() {
            "1080" => "bv*[height<=1080]+ba/b[height<=1080]",
            "720" => "bv*[height<=720]+ba/b[height<=720]",
            "480" => "bv*[height<=480]+ba/b[height<=480]",
            _ => "bv*+ba/b",
        };
        args.push("-f".to_string());
        args.push(quality_arg.to_string());

        let target_fmt = if matches!(task.video_format.as_str(), "mp4" | "mkv") {
            &task.video_format
        } else {
            "mp4"
        };
        args.push("--merge-output-format".to_string());
        args.push(target_fmt.to_string());

        if task.embed_metadata {
            args.push("--embed-metadata".to_string());
            args.push("--parse-metadata".to_string());
            args.push("%(artist,creator,uploader,channel)s:%(artist)s".to_string());
            args.push("--parse-metadata".to_string());
            args.push("%(description)s:%(comment)s".to_string());
        }
        if task.embed_thumbnail {
            args.push("--embed-thumbnail".to_string());
            args.push("--convert-thumbnails".to_string());
            args.push("jpg".to_string());
        }
        if task.embed_subtitles && !task.whisper_enabled {
            args.extend([
                "--write-subs".to_string(),
                "--write-auto-subs".to_string(),
                "--sub-langs".to_string(),
                "ja.*,en.*".to_string(),
                "--embed-subs".to_string(),
                "--compat-options".to_string(),
                "no-keep-subs".to_string(),
                "--sleep-subtitles".to_string(),
                "2".to_string(),
                "-i".to_string(),
            ]);
        }
    } else if task.download_type == "custom" && !task.custom_args.is_empty() {
        for piece in task.custom_args.split_whitespace() {
            args.push(piece.to_string());
        }
    }

    args.push(url);

    task.status = "downloading".to_string();
    let now_sec = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
    task.started_at = Some(now_sec);
    update_task_file(task_file, task);

    let mut detected_filepath: Option<PathBuf> = None;
    let mut fatal_error: Option<String> = None;
    let mut last_save = Instant::now();

    let mut cmd = Command::new("yt-dlp");
    cmd.args(&args)
        .current_dir(output_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return DownloadResult {
                success: false,
                media_path: None,
                error: Some(format!("Failed to spawn yt-dlp: {}", e)),
            };
        }
    };

    task.pid = Some(child.id());
    update_task_file(task_file, task);

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().flatten() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(caps) = dest_re.captures(line) {
                let dest_str = caps[1].trim();
                let path = PathBuf::from(dest_str);
                if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                    let ignored = ["vtt", "srt", "ttml", "part", "ytdl", "temp", "jpg", "jpeg", "png", "webp"];
                    if !ignored.contains(&ext.as_str()) {
                        detected_filepath = Some(path.clone());
                        task.title = fname.to_string();
                    }
                }
            }

            let is_converting = ["[ExtractAudio]", "[Merger]", "[Fixup", "[EmbedThumbnail]", "[Metadata]", "[EmbedSubtitle]"]
                .iter()
                .any(|&k| line.contains(k));

            if is_converting {
                task.status = "converting".to_string();
                task.percent = 99.0;
                task.speed = "ffmpeg で結合・変換中...".to_string();
                task.eta = "--:--".to_string();
                update_task_file(task_file, task);
                continue;
            }

            if let Some(caps) = progress_re.captures(line) {
                if let Ok(pct) = caps[1].parse::<f64>() {
                    task.percent = pct;
                }
                task.total_size = caps[2].to_string();
                task.speed = caps[3].to_string();
                task.eta = caps[4].to_string();
                task.status = "downloading".to_string();

                if last_save.elapsed().as_millis() >= 400 {
                    update_task_file(task_file, task);
                    last_save = Instant::now();
                }
                continue;
            }

            if let Some(caps) = progress_100_re.captures(line) {
                task.percent = 100.0;
                task.total_size = caps[1].to_string();
                task.eta = "00:00".to_string();
                update_task_file(task_file, task);
                continue;
            }

            if line.contains("ERROR:") {
                fatal_error = Some(line.to_string());
            }
        }
    }

    let status = child.wait();
    let exit_code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);

    // Verify if media exists
    let mut media_exists = false;
    if let Some(ref p) = detected_filepath {
        if p.is_file() {
            media_exists = true;
        }
    }

    if !media_exists {
        if let Ok(entries) = std::fs::read_dir(output_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                    if ["mp4", "mkv", "webm", "mp3", "flac", "m4a", "opus", "wav"].contains(&ext.as_str()) {
                        if let Ok(meta) = p.metadata() {
                            if let Ok(mtime) = meta.modified() {
                                let mtime_sec = mtime.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs_f64();
                                if mtime_sec >= task.started_at.unwrap_or(0.0) {
                                    detected_filepath = Some(p);
                                    media_exists = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let is_subtitle_error = fatal_error
        .as_ref()
        .map(|e| e.to_lowercase().contains("subtitles") || e.contains("429"))
        .unwrap_or(false);

    let is_success = exit_code == 0 || (media_exists && is_subtitle_error);

    DownloadResult {
        success: is_success && media_exists,
        media_path: detected_filepath,
        error: if is_success { None } else { fatal_error.or(Some(format!("Process exited with code {}", exit_code))) },
    }
}
