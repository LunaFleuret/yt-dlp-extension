use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Deserialize)]
struct LoudnormStats {
    input_i: String,
    input_lra: String,
    input_tp: String,
    input_thresh: String,
    target_offset: String,
}

pub fn normalize_audio_loudness(
    ffmpeg_path: &Path,
    media_file: &Path,
    target_lufs: f64,
) -> Result<bool> {
    if !media_file.is_file() {
        return Ok(false);
    }

    let ext = media_file
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_video = matches!(ext.as_str(), "mp4" | "mkv" | "webm" | "mov" | "avi");

    // Pass 1: Measure loudness metrics
    let mut pass1_cmd = Command::new(ffmpeg_path);
    pass1_cmd.args([
        "-hide_banner",
        "-nostats",
        "-i",
        &media_file.to_string_lossy(),
        "-af",
        &format!("loudnorm=I={:.1}:LRA=11:TP=-1.0:print_format=json", target_lufs),
        "-f",
        "null",
        "-",
    ]);
    #[cfg(windows)]
    pass1_cmd.creation_flags(CREATE_NO_WINDOW);
    let pass1_output = pass1_cmd.output()?;

    let stderr = String::from_utf8_lossy(&pass1_output.stderr);
    let re = Regex::new(r#"(?s)\{\s*"input_i"\s*:.*?\}"#)?;
    let json_str = match re.find(&stderr) {
        Some(m) => m.as_str(),
        None => {
            return Ok(false);
        }
    };

    let stats: LoudnormStats = match serde_json::from_str(json_str) {
        Ok(s) => s,
        Err(_) => {
            return Ok(false);
        }
    };

    // Pass 2: Apply linear gain normalization
    let temp_norm = media_file.with_extension(format!("norm_temp.{}", ext));

    let filter_str = format!(
        "loudnorm=I={:.1}:LRA=11:TP=-1.0:measured_I={}:measured_LRA={}:measured_TP={}:measured_thresh={}:offset={}:linear=true",
        target_lufs, stats.input_i, stats.input_lra, stats.input_tp, stats.input_thresh, stats.target_offset
    );

    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-y")
        .arg("-i")
        .arg(media_file)
        .arg("-af")
        .arg(&filter_str);

    if is_video {
        // Video file: copy video stream without re-encoding, re-encode audio only
        cmd.args([
            "-map", "0",
            "-c:v", "copy",
            "-c:a", "aac",
            "-b:a", "256k",
            "-c:s", "copy",
        ]);
    } else {
        // Audio-only file
        match ext.as_str() {
            "m4a" => { cmd.args(["-c:a", "aac", "-b:a", "256k"]); }
            "mp3" => { cmd.args(["-c:a", "libmp3lame", "-q:a", "0"]); }
            "flac" => { cmd.args(["-c:a", "flac"]); }
            "opus" => { cmd.args(["-c:a", "libopus", "-b:a", "160k"]); }
            "wav" => { cmd.args(["-c:a", "pcm_s16le"]); }
            _ => { cmd.args(["-c:a", "aac", "-b:a", "256k"]); }
        }
    }

    cmd.arg(&temp_norm);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let status = cmd.status()?;
    if status.success() && temp_norm.is_file() {
        std::fs::rename(&temp_norm, media_file)?;
        Ok(true)
    } else {
        let _ = std::fs::remove_file(&temp_norm);
        Ok(false)
    }
}
