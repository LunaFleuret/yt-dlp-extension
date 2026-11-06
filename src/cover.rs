use anyhow::Result;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn is_valid_image(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() < 1024 {
            return false;
        }
    } else {
        return false;
    }

    if let Ok(reader) = image::ImageReader::open(path) {
        if let Ok(dimensions) = reader.into_dimensions() {
            return dimensions.0 > 0 && dimensions.1 > 0;
        }
    }

    false
}

pub fn get_image_dimensions(path: &Path) -> Option<(u32, u32)> {
    if let Ok(reader) = image::ImageReader::open(path) {
        if let Ok(dims) = reader.into_dimensions() {
            return Some(dims);
        }
    }
    None
}

pub async fn download_youtube_cdn_thumbnail(
    url: &str,
    destination: &Path,
) -> Option<PathBuf> {
    let re = Regex::new(r"(?:youtube\.com/(?:watch\?.*v=|shorts/)|youtu\.be/)([a-zA-Z0-9_-]{11})").ok()?;
    let video_id = re.captures(url)?.get(1)?.as_str();

    let qualities = ["maxresdefault", "sddefault", "hqdefault"];
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .ok()?;

    for q in &qualities {
        let cdn_url = format!("https://i.ytimg.com/vi/{}/{}.jpg", video_id, q);

        if let Ok(resp) = client.get(&cdn_url).send().await {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    if let Ok(_) = std::fs::write(destination, &bytes) {
                        if is_valid_image(destination) {
                            return Some(destination.to_path_buf());
                        }
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_file(destination);
    None
}

pub fn format_cover_art_square(
    ffmpeg_path: &Path,
    input_image: &Path,
    output_image: &Path,
) -> Result<PathBuf> {
    if let Some((w, h)) = get_image_dimensions(input_image) {
        if w == h {
            return Ok(input_image.to_path_buf());
        }
    }

    let filter = "crop='min(iw,ih)':'min(iw,ih)'";
    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-y")
        .arg("-i")
        .arg(input_image)
        .arg("-vf")
        .arg(filter)
        .args(["-q:v", "2"])
        .arg(output_image);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let status = cmd.status()?;
    if status.success() && output_image.is_file() {
        Ok(output_image.to_path_buf())
    } else {
        Ok(input_image.to_path_buf())
    }
}

pub fn embed_cover_art(
    ffmpeg_path: &Path,
    media_file: &Path,
    cover_image: &Path,
) -> Result<bool> {
    if !media_file.is_file() || !cover_image.is_file() {
        return Ok(false);
    }

    let ext = media_file
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let temp_embedded = media_file.with_extension(format!("embed_temp.{}", ext));

    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-y")
        .arg("-i")
        .arg(media_file)
        .arg("-i")
        .arg(cover_image);

    match ext.as_str() {
        "mp3" => {
            cmd.args([
                "-map", "0:a",
                "-map", "1:v",
                "-c", "copy",
                "-id3v2_version", "3",
                "-metadata:s:v", "title=Album cover",
                "-metadata:s:v", "comment=Cover (front)",
            ]);
        }
        "mp4" | "m4a" => {
            cmd.args([
                "-map", "0",
                "-map", "1",
                "-c", "copy",
                "-disposition:v:1", "attached_pic",
            ]);
        }
        "mkv" => {
            cmd.args([
                "-c", "copy",
                "-attach", &cover_image.to_string_lossy(),
                "-metadata:s:t", "mimetype=image/jpeg",
            ]);
        }
        _ => {
            return Ok(false);
        }
    }

    cmd.arg(&temp_embedded);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let status = cmd.status()?;
    if status.success() && temp_embedded.is_file() {
        std::fs::rename(&temp_embedded, media_file)?;
        Ok(true)
    } else {
        let _ = std::fs::remove_file(&temp_embedded);
        Ok(false)
    }
}
