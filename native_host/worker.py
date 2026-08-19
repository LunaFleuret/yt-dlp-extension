#!/usr/bin/env python
# -*- coding: utf-8 -*-

import sys
import os
import re
import json
import time
import pathlib
import subprocess

PROGRESS_REGEX = re.compile(
    r'\[download\]\s+([0-9\.]+)%\s+of\s+~?([^\s]+)\s+at\s+([^\s]+)\s+ETA\s+([^\s]+)'
)
PROGRESS_100_REGEX = re.compile(
    r'\[download\]\s+100%(?:\.0%)?\s+of\s+~?([^\s]+)'
)
DESTINATION_REGEX = re.compile(
    r'\[(?:download|Merger|ExtractAudio)\]\s+Destination:\s+(.+)'
)

def update_task_file(task_file, data):
    """Atomically write task status to JSON file."""
    try:
        temp_file = task_file.with_suffix(".tmp")
        with open(temp_file, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
        temp_file.replace(task_file)
    except Exception:
        pass

def decode_line(raw_bytes):
    """Decode raw bytes gracefully across UTF-8 and Japanese Windows CP932."""
    if not raw_bytes:
        return ""
    try:
        return raw_bytes.decode("utf-8")
    except UnicodeDecodeError:
        try:
            return raw_bytes.decode("cp932")
        except Exception:
            return raw_bytes.decode("utf-8", errors="replace")

def cleanup_standalone_thumbnails(output_dir, final_filepath):
    """Ensure only the single target media file remains in the output directory."""
    if not output_dir.exists():
        return

    # Look for any lingering .jpg, .webp, .part, .ytdl, or intermediate files
    for f in output_dir.iterdir():
        if f.is_file():
            # Delete image files, subtitle files, and temp extensions
            if f.suffix.lower() in [".jpg", ".jpeg", ".png", ".webp", ".vtt", ".srt", ".ttml", ".part", ".ytdl", ".temp"]:
                try:
                    f.unlink()
                except Exception:
                    pass


def main():
    if len(sys.argv) < 2:
        sys.exit(1)

    task_file_path = pathlib.Path(sys.argv[1]).resolve()
    if not task_file_path.exists():
        sys.exit(1)

    try:
        with open(task_file_path, "r", encoding="utf-8") as f:
            task_data = json.load(f)
    except Exception:
        sys.exit(1)

    url = task_data.get("url", "").strip()
    if not url:
        sys.exit(1)

    download_type = task_data.get("download_type", "video") # video, audio, custom
    video_quality = task_data.get("video_quality", "best")   # best, 1080, 720, 480
    video_format = task_data.get("video_format", "mp4")     # mp4, mkv
    audio_format = task_data.get("audio_format", "mp3")     # mp3, flac, m4a, opus, wav
    embed_thumbnail = task_data.get("embed_thumbnail", True)
    embed_metadata = task_data.get("embed_metadata", True)
    embed_subtitles = task_data.get("embed_subtitles", False)
    custom_args = task_data.get("custom_args", "").strip()
    download_dir = task_data.get("download_dir", "").strip()
    show_console = task_data.get("show_console", False)

    base_dir = pathlib.Path(__file__).resolve().parent
    output_dir = (base_dir / (download_dir if download_dir else "../output")).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    working_dir = str(output_dir)

    # Base yt-dlp command configured to merge everything into a single file
    cmd = [
        "yt-dlp",
        "--newline",
        "--windows-filenames",
        "--no-mtime",
        "-P", working_dir
    ]

    if download_type == "audio":
        # Extract audio and merge tags/cover into a single audio file
        cmd.extend([
            "-x",
            "--audio-format", audio_format,
            "--no-keep-video"
        ])
        if audio_format == "mp3":
            cmd.extend(["--audio-quality", "0"])

        if embed_thumbnail:
            cmd.extend(["--embed-thumbnail", "--convert-thumbnails", "jpg"])

        if embed_metadata:
            cmd.extend([
                "--embed-metadata",
                "--parse-metadata", "%(artist,creator,uploader,channel)s:%(artist)s"
            ])

    elif download_type == "video":
        # Video stream selection
        if video_quality == "1080":
            cmd.extend(["-f", "bv*[height<=1080]+ba/b[height<=1080]"])
        elif video_quality == "720":
            cmd.extend(["-f", "bv*[height<=720]+ba/b[height<=720]"])
        elif video_quality == "480":
            cmd.extend(["-f", "bv*[height<=480]+ba/b[height<=480]"])
        else:
            cmd.extend(["-f", "bv*+ba/b"])

        # Force merge into a single container via ffmpeg
        target_format = video_format if video_format in ["mp4", "mkv"] else "mp4"
        cmd.extend(["--merge-output-format", target_format])

        if embed_metadata:
            cmd.extend([
                "--embed-metadata",
                "--parse-metadata", "%(artist,creator,uploader,channel)s:%(artist)s"
            ])

        if embed_thumbnail:
            cmd.extend(["--embed-thumbnail", "--convert-thumbnails", "jpg"])

        if embed_subtitles:
            cmd.extend([
                "--write-subs",
                "--write-auto-subs",
                "--sub-langs", "ja,en,ja-orig,en-orig",
                "--embed-subs",
                "--no-abort-on-error"
            ])


    elif download_type == "custom" and custom_args:
        cmd.extend(custom_args.split())

    cmd.append(url)

    CREATE_NO_WINDOW = 0x08000000
    creation_flags = subprocess.CREATE_NEW_CONSOLE if show_console else CREATE_NO_WINDOW

    task_data["status"] = "downloading"
    task_data["started_at"] = time.time()
    update_task_file(task_file_path, task_data)

    last_save_time = 0
    fatal_error = None
    final_detected_filepath = None

    # Force UTF-8 environment
    env = os.environ.copy()
    env["PYTHONUTF8"] = "1"
    env["PYTHONIOENCODING"] = "utf-8"

    try:
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
            cwd=working_dir,
            env=env,
            creationflags=creation_flags,
            bufsize=1
        )

        task_data["pid"] = proc.pid
        update_task_file(task_file_path, task_data)

        for raw_line in proc.stdout:
            clean_line = decode_line(raw_line).strip()
            if not clean_line:
                continue

            # Detect destination filename
            dest_match = DESTINATION_REGEX.search(clean_line)
            if dest_match:
                detected_dest = dest_match.group(1).strip()
                final_detected_filepath = detected_dest
                filename = os.path.basename(detected_dest)
                if not filename.endswith(".part"):
                    task_data["title"] = filename

            # Converting / Merging stages
            if any(k in clean_line for k in ["[ExtractAudio]", "[Merger]", "[Fixup", "[EmbedThumbnail]", "[Metadata]", "[EmbedSubtitle]"]):
                task_data["status"] = "converting"
                task_data["percent"] = 99.0
                task_data["speed"] = "ffmpeg で結合・変換中..."
                task_data["eta"] = "--:--"
                update_task_file(task_file_path, task_data)
                continue

            # Parse downloading progress
            match = PROGRESS_REGEX.search(clean_line)
            if match:
                task_data["percent"] = float(match.group(1))
                task_data["total_size"] = match.group(2)
                task_data["speed"] = match.group(3)
                task_data["eta"] = match.group(4)
                task_data["status"] = "downloading"

                now = time.time()
                if now - last_save_time >= 0.4:
                    update_task_file(task_file_path, task_data)
                    last_save_time = now
                continue

            match_100 = PROGRESS_100_REGEX.search(clean_line)
            if match_100:
                task_data["percent"] = 100.0
                task_data["total_size"] = match_100.group(1)
                task_data["eta"] = "00:00"
                update_task_file(task_file_path, task_data)
                continue

            if "ERROR:" in clean_line:
                fatal_error = clean_line

        proc.wait()

        task_data["finished_at"] = time.time()
        if proc.returncode == 0:
            task_data["status"] = "completed"
            task_data["percent"] = 100.0
            task_data["speed"] = "完了"
            task_data["eta"] = "00:00"
            task_data["error"] = None

            # Clean up standalone temp/thumbnail files so only the single file remains
            if final_detected_filepath:
                cleanup_standalone_thumbnails(output_dir, final_detected_filepath)
        else:
            if task_data.get("status") != "cancelled":
                task_data["status"] = "error"
                task_data["error"] = fatal_error or f"Process exited with code {proc.returncode}"

    except Exception as e:
        task_data["status"] = "error"
        task_data["error"] = str(e)
        task_data["finished_at"] = time.time()

    update_task_file(task_file_path, task_data)

if __name__ == "__main__":
    main()
