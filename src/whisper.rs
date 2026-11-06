use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct WhisperConfig<'a> {
    pub whisper_bin: &'a Path,
    pub models_dir: &'a Path,
    pub model_name: &'a str,
    pub burn_subtitles: bool,
    pub save_md: bool,
    pub threads: u32,
    pub device: &'a str,
}

fn format_time(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn format_duration_ja(seconds: f64) -> String {
    let total_secs = seconds.round() as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{}時間{}分{}秒", hours, minutes, secs)
    } else {
        format!("{}分{}秒", minutes, secs)
    }
}

fn parse_time_to_ms(time_str: &str) -> u64 {
    let s = time_str.trim();
    let parts: Vec<&str> = s.split(',').collect();
    let hms_part = parts[0];
    let ms_part = if parts.len() > 1 {
        parts[1].parse::<u64>().unwrap_or(0)
    } else {
        0
    };

    let hms: Vec<&str> = hms_part.split(':').collect();
    if hms.len() >= 3 {
        let h = hms[0].parse::<u64>().unwrap_or(0);
        let m = hms[1].parse::<u64>().unwrap_or(0);
        let sec = hms[2].parse::<u64>().unwrap_or(0);
        h * 3600_000 + m * 60_000 + sec * 1_000 + ms_part
    } else {
        0
    }
}

#[derive(Debug)]
struct SrtSegment {
    start_ms: u64,
    end_ms: u64,
    text: String,
}

pub fn generate_markdown(
    srt_path: &Path,
    md_path: &Path,
    media_name: &str,
    model_key: &str,
    elapsed_sec: f64,
) -> Result<()> {
    let content = std::fs::read_to_string(srt_path)?;
    let content = content.replace("\r\n", "\n").replace('\r', "\n");
    let blocks: Vec<&str> = content.split("\n\n").collect();

    let mut transcription: Vec<SrtSegment> = Vec::new();

    for block in blocks {
        let lines: Vec<&str> = block.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.len() >= 3 {
            let time_line = lines[1];
            let text = lines[2..].join(" ");
            let times: Vec<&str> = time_line.split(" --> ").collect();
            if times.len() == 2 {
                let start_ms = parse_time_to_ms(times[0]);
                let end_ms = parse_time_to_ms(times[1]);
                transcription.push(SrtSegment {
                    start_ms,
                    end_ms,
                    text,
                });
            }
        }
    }

    if transcription.is_empty() {
        return Ok(());
    }

    let total_ms = transcription.last().map(|s| s.end_ms).unwrap_or(0);
    let boundaries = [
        (total_ms as f64 * 0.25) as u64,
        (total_ms as f64 * 0.50) as u64,
        (total_ms as f64 * 0.75) as u64,
    ];
    let mut current_part = 1;

    let mut md = String::new();
    md.push_str(&format!("# 文字起こし: {}\n", media_name));
    md.push_str(&format!("- **モデル**: {}\n", model_key));
    md.push_str(&format!(
        "- **処理時間**: {} ({:.2}秒)\n",
        format_duration_ja(elapsed_sec),
        elapsed_sec
    ));
    md.push_str(&format!(
        "- **音声長**: {}\n\n",
        format_duration_ja(total_ms as f64 / 1000.0)
    ));
    md.push_str("## Part 1/4 (00:00:00 ~)\n\n");

    for segment in &transcription {
        let ts_start = format_time(segment.start_ms);
        let ts_end = format_time(segment.end_ms);
        if current_part <= boundaries.len() && segment.start_ms >= boundaries[current_part - 1] {
            current_part += 1;
            let time_str = format_time(segment.start_ms);
            md.push_str(&format!("\n\n## Part {}/4 ({} ~)\n\n", current_part, time_str));
        }
        md.push_str(&format!("`[{} --> {}]` {}  \n", ts_start, ts_end, segment.text));
    }

    std::fs::write(md_path, md)?;
    Ok(())
}

fn resolve_model_file(models_dir: &Path, model_name: &str) -> Option<PathBuf> {
    let direct = models_dir.join(format!("ggml-{}.bin", model_name));
    if direct.is_file() {
        return Some(direct);
    }
    if let Ok(entries) = std::fs::read_dir(models_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Some(filename) = p.file_name().and_then(|s| s.to_str()) {
                if filename.contains(model_name) && filename.ends_with(".bin") {
                    return Some(p);
                }
            }
        }
    }
    None
}

pub fn apply_whisper(
    ffmpeg_path: &Path,
    media_file: &Path,
    output_dir: &Path,
    cfg: &WhisperConfig,
) -> Result<()> {
    if !media_file.is_file() {
        return Err(anyhow!("Media file does not exist"));
    }

    let model_file = resolve_model_file(cfg.models_dir, cfg.model_name)
        .ok_or_else(|| anyhow!("Whisper model file not found in {:?}", cfg.models_dir))?;

    if !cfg.whisper_bin.is_file() {
        return Err(anyhow!("Whisper CLI not found at {:?}", cfg.whisper_bin));
    }

    let pid = std::process::id();
    let temp_wav = output_dir.join(format!("temp_whisper_audio_{}.wav", pid));
    let srt_base = output_dir.join(format!("temp_whisper_srt_{}", pid));
    let srt_file = output_dir.join(format!("temp_whisper_srt_{}.srt", pid));

    let start_time = std::time::Instant::now();

    // 1. Extract 16kHz mono audio for Whisper
    let mut extract_cmd = Command::new(ffmpeg_path);
    extract_cmd.args([
        "-y",
        "-i",
        &media_file.to_string_lossy(),
        "-vn",
        "-ar",
        "16000",
        "-ac",
        "1",
        "-c:a",
        "pcm_s16le",
        &temp_wav.to_string_lossy(),
    ]);
    #[cfg(windows)]
    extract_cmd.creation_flags(CREATE_NO_WINDOW);
    let extract_status = extract_cmd.status()?;

    if !extract_status.success() || !temp_wav.is_file() {
        let _ = std::fs::remove_file(&temp_wav);
        return Err(anyhow!("ffmpeg audio extraction failed"));
    }

    // 2. Run Whisper CLI with AMD Vulkan acceleration
    let thread_str = cfg.threads.to_string();
    let mut whisper_cmd = Command::new(cfg.whisper_bin);
    whisper_cmd
        .env("GGML_VULKAN_DEVICE", cfg.device)
        .args([
            "-m",
            &model_file.to_string_lossy(),
            "-f",
            &temp_wav.to_string_lossy(),
            "-osrt",
            "-of",
            &srt_base.to_string_lossy(),
            "-l",
            "ja",
            "-t",
            &thread_str,
            "-bs",
            "2",
            "-nth",
            "0.60",
            "-et",
            "2.40",
            "-nf",
            "-mc",
            "0",
        ]);

    #[cfg(windows)]
    whisper_cmd.creation_flags(CREATE_NO_WINDOW);

    let output = whisper_cmd.output()?;
    let _ = std::fs::remove_file(&temp_wav);

    if !output.status.success() || !srt_file.is_file() {
        let _ = std::fs::remove_file(&srt_file);
        return Err(anyhow!("Whisper transcription failed to produce SRT"));
    }

    let elapsed = start_time.elapsed().as_secs_f64();

    // 3. Save Markdown if configured
    if cfg.save_md {
        if let Some(stem) = media_file.file_stem().and_then(|s| s.to_str()) {
            let md_path = output_dir.join(format!("{}.md", stem));
            let media_name = media_file.file_name().and_then(|s| s.to_str()).unwrap_or(stem);
            let _ = generate_markdown(&srt_file, &md_path, media_name, cfg.model_name, elapsed);
        }
    }

    // 4. Merge subtitles into video
    let ext = media_file
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mp4")
        .to_lowercase();

    let temp_merged = output_dir.join(format!("temp_subbed_{}.{}", pid, ext));

    if !cfg.burn_subtitles {
        // Soft subtitles (no re-encoding)
        let sub_codec = if ext == "mp4" { "mov_text" } else { "subrip" };
        let mut merge_cmd = Command::new(ffmpeg_path);
        merge_cmd.args([
            "-y",
            "-i",
            &media_file.to_string_lossy(),
            "-i",
            &srt_file.to_string_lossy(),
            "-map",
            "0",
            "-map",
            "1:s",
            "-c",
            "copy",
            "-c:s",
            sub_codec,
            "-metadata:s:s:0",
            "language=jpn",
            "-metadata:s:s:0",
            "title=Whisper AI",
            &temp_merged.to_string_lossy(),
        ]);

        #[cfg(windows)]
        merge_cmd.creation_flags(CREATE_NO_WINDOW);

        let status = merge_cmd.status()?;
        if status.success() && temp_merged.is_file() {
            let _ = std::fs::remove_file(media_file);
            std::fs::rename(&temp_merged, media_file)?;
        } else {
            let _ = std::fs::remove_file(&temp_merged);
            let _ = std::fs::remove_file(&srt_file);
            return Err(anyhow!("ffmpeg subtitle soft-merge failed"));
        }
    } else {
        // Hard subtitles (burn into video)
        let srt_name = srt_file.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let mut merge_cmd = Command::new(ffmpeg_path);
        merge_cmd
            .current_dir(output_dir)
            .args([
                "-y",
                "-i",
                &media_file.to_string_lossy(),
                "-vf",
                &format!("subtitles='{}'", srt_name),
                "-c:a",
                "copy",
                "-c:v",
                "libx264",
                "-crf",
                "18",
                "-preset",
                "fast",
                &temp_merged.to_string_lossy(),
            ]);

        #[cfg(windows)]
        merge_cmd.creation_flags(CREATE_NO_WINDOW);

        let status = merge_cmd.status()?;
        if status.success() && temp_merged.is_file() {
            let _ = std::fs::remove_file(media_file);
            std::fs::rename(&temp_merged, media_file)?;
        } else {
            let _ = std::fs::remove_file(&temp_merged);
            let _ = std::fs::remove_file(&srt_file);
            return Err(anyhow!("ffmpeg subtitle burn failed"));
        }
    }

    let _ = std::fs::remove_file(&srt_file);
    Ok(())
}
