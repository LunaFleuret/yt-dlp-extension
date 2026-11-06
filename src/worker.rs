use crate::cover;
use crate::downloader::{self, TaskInfo};
use crate::loudness;
use crate::whisper::{self, WhisperConfig};
use anyhow::Result;
use std::path::PathBuf;

pub async fn run_worker(task_file: PathBuf) -> Result<()> {
    if !task_file.is_file() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&task_file)?;
    let mut task: TaskInfo = serde_json::from_str(&content)?;

    let base_dir = task_file
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let target_dir = if !task.download_dir.trim().is_empty() {
        PathBuf::from(task.download_dir.trim())
    } else {
        PathBuf::from("../output")
    };

    let output_dir = if target_dir.is_absolute() {
        target_dir
    } else {
        base_dir.join(target_dir)
    };

    let _ = std::fs::create_dir_all(&output_dir);

    let ffmpeg_path = PathBuf::from("ffmpeg");

    let dl_result = downloader::execute_ytdlp_download(&task_file, &mut task, &output_dir, &base_dir);

    if dl_result.success && dl_result.media_path.is_some() {
        let media_path = dl_result.media_path.unwrap();

        // 1. Thumbnail Rescue if needed
        if task.embed_thumbnail {
            let cdn_thumb_path = output_dir.join(format!("cdn_thumbnail_{}.jpg", task.id));
            if let Some(thumb_path) = cover::download_youtube_cdn_thumbnail(&task.url, &cdn_thumb_path).await {
                let is_audio = task.download_type == "audio";
                let square_thumb = output_dir.join(format!("square_thumb_{}.jpg", task.id));
                let final_thumb = if is_audio {
                    cover::format_cover_art_square(&ffmpeg_path, &thumb_path, &square_thumb)
                        .unwrap_or(thumb_path.clone())
                } else {
                    thumb_path.clone()
                };

                let _ = cover::embed_cover_art(&ffmpeg_path, &media_path, &final_thumb);
                let _ = std::fs::remove_file(&thumb_path);
                let _ = std::fs::remove_file(&square_thumb);
            }
        }

        // 2. Audio Loudness Normalization (EBU R128 / -14 LUFS)
        if task.normalize_audio {
            task.status = "normalizing".to_string();
            task.percent = 99.0;
            task.speed = "音量を自動均一化中 (EBU R128)...".to_string();
            task.eta = "--:--".to_string();
            downloader::update_task_file(&task_file, &task);

            let _ = loudness::normalize_audio_loudness(&ffmpeg_path, &media_path, -14.0);
        }

        // 3. Whisper AI Transcription (AMD Radeon RX 9060 XT / Vulkan)
        let mut whisper_err: Option<String> = None;
        if task.whisper_enabled {
            task.status = "transcribing".to_string();
            task.percent = 99.0;
            task.speed = "Whisper文字起こし中 (AMD RX 9060 XT / Vulkan)...".to_string();
            task.eta = "--:--".to_string();
            downloader::update_task_file(&task_file, &task);

            let whisper_bin_path = PathBuf::from(&task.whisper_bin_path);
            let whisper_models_dir = PathBuf::from(&task.whisper_models_dir);

            let wcfg = WhisperConfig {
                whisper_bin: &whisper_bin_path,
                models_dir: &whisper_models_dir,
                model_name: &task.whisper_model,
                burn_subtitles: task.whisper_burn_subtitles,
                save_md: task.whisper_save_md,
                threads: 8,
                device: "0",
            };

            if let Err(e) = whisper::apply_whisper(&ffmpeg_path, &media_path, &output_dir, &wcfg) {
                whisper_err = Some(e.to_string());
            }
        }

        // Clean up intermediate temp files
        downloader::cleanup_standalone_artifacts(&output_dir);

        let now_sec = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
        task.finished_at = Some(now_sec);
        task.status = "completed".to_string();
        task.percent = 100.0;
        task.speed = if task.whisper_enabled && whisper_err.is_none() {
            "完了 (Whisper字幕付き)".to_string()
        } else {
            "完了".to_string()
        };
        task.eta = "00:00".to_string();
        task.error = whisper_err;
    } else {
        let now_sec = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
        task.finished_at = Some(now_sec);
        if task.status != "cancelled" {
            task.status = "error".to_string();
            task.error = dl_result.error.or(Some("Download failed".to_string()));
        }
    }

    downloader::update_task_file(&task_file, &task);
    Ok(())
}
