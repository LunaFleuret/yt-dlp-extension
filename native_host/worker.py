#!/usr/bin/env python
# -*- coding: utf-8 -*-

import sys
import os
import re
import json
import time
import datetime
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

def format_time(ms):
    td = datetime.timedelta(milliseconds=ms)
    hours, remainder = divmod(td.seconds, 3600)
    minutes, seconds = divmod(remainder, 60)
    return f"{hours:02d}:{minutes:02d}:{seconds:02d}"

def format_duration_ja(seconds):
    seconds = int(round(seconds))
    hours = seconds // 3600
    minutes = (seconds % 3600) // 60
    secs = seconds % 60
    if hours > 0:
        return f"{hours}時間{minutes}分{secs}秒"
    else:
        return f"{minutes}分{secs}秒"

def parse_time_to_ms(time_str):
    time_str = time_str.strip()
    if ',' not in time_str:
        time_str += ',000'
    parts = time_str.split(',')
    hms = parts[0].split(':')
    return int(hms[0])*3600000 + int(hms[1])*60000 + int(hms[2])*1000 + int(parts[1])

def generate_markdown(srt_path, md_path, media_name, model_key, elapsed_sec):
    try:
        with open(srt_path, "r", encoding="utf-8", errors="replace") as f:
            content = f.read().strip()
        if not content:
            return
        content = content.replace("\r\n", "\n").replace("\r", "\n")
        blocks = re.split(r'\n\n+', content)
        transcription = []
        for block in blocks:
            lines = [l.strip() for l in block.split('\n') if l.strip()]
            if len(lines) >= 3:
                time_line = lines[1]
                text = " ".join(lines[2:]).strip()
                times = time_line.split(" --> ")
                if len(times) == 2:
                    start_ms = parse_time_to_ms(times[0])
                    end_ms = parse_time_to_ms(times[1])
                    transcription.append({"start": start_ms, "end": end_ms, "text": text})
        if not transcription:
            return
        total_ms = transcription[-1]["end"]
        boundaries = [total_ms * 0.25, total_ms * 0.50, total_ms * 0.75]
        current_part = 1

        with open(md_path, "w", encoding="utf-8") as md:
            md.write(f"# 文字起こし: {media_name}\n")
            md.write(f"- **モデル**: {model_key}\n")
            md.write(f"- **処理時間**: {format_duration_ja(elapsed_sec)} ({elapsed_sec:.2f}秒)\n")
            md.write(f"- **音声長**: {format_duration_ja(total_ms / 1000.0)}\n\n")
            md.write("## Part 1/4 (00:00:00 ~)\n\n")
            for segment in transcription:
                start_ms = segment["start"]
                end_ms = segment["end"]
                text = segment["text"]
                ts_start = format_time(start_ms)
                ts_end = format_time(end_ms)
                if current_part <= len(boundaries) and start_ms >= boundaries[current_part - 1]:
                    current_part += 1
                    time_str = format_time(start_ms)
                    md.write(f"\n\n## Part {current_part}/4 ({time_str} ~)\n\n")
                md.write(f"`[{ts_start} --> {ts_end}]` {text}  \n")
    except Exception as err:
        sys.stderr.write(f"Markdown generation warning: {err}\n")

def apply_whisper_transcription(final_media_path, output_dir, task_data, task_file_path):
    """
    Run Whisper CLI with AMD Vulkan acceleration (RX 9060 XT),
    generate subtitles, and embed/burn them into the video.
    """
    media_path = pathlib.Path(final_media_path)
    if not media_path.exists():
        return False, "対象の動画ファイルが見つかりません。"

    whisper_bin = pathlib.Path(task_data.get("whisper_bin_path", "C:/AI-Models/Whisper-Models/bin/whisper-cli.exe"))
    models_dir = pathlib.Path(task_data.get("whisper_models_dir", "C:/AI-Models/Whisper-Models"))
    model_name = task_data.get("whisper_model", "large-v3-turbo")
    burn_subtitles = task_data.get("whisper_burn_subtitles", False)
    save_md = task_data.get("whisper_save_md", False)

    model_file = models_dir / f"ggml-{model_name}.bin"
    if not model_file.exists():
        candidates = list(models_dir.glob(f"*{model_name}*.bin"))
        if candidates:
            model_file = candidates[0]
        else:
            return False, f"Whisperモデルが見つかりません: {model_file}"

    if not whisper_bin.exists():
        return False, f"Whisper CLIが見つかりません: {whisper_bin}"

    pid = os.getpid()
    temp_wav = output_dir / f"temp_whisper_audio_{pid}.wav"
    srt_base = output_dir / f"temp_whisper_srt_{pid}"
    srt_output = output_dir / f"temp_whisper_srt_{pid}.srt"

    start_whisper_time = time.time()
    try:
        # 1. 音声抽出 (16kHz mono pcm_s16le)
        task_data["status"] = "transcribing"
        task_data["percent"] = 99.0
        task_data["speed"] = "音声を抽出中..."
        task_data["eta"] = "--:--"
        update_task_file(task_file_path, task_data)

        CREATE_NO_WINDOW = 0x08000000
        extract_cmd = [
            "ffmpeg", "-y",
            "-i", str(media_path),
            "-vn", "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le",
            str(temp_wav)
        ]
        res = subprocess.run(extract_cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=CREATE_NO_WINDOW)
        if res.returncode != 0 or not temp_wav.exists():
            return False, "ffmpegによる音声抽出に失敗しました。"

        # 2. Whisper文字起こし実行 (AMD RX 9060 XT / Vulkan)
        task_data["speed"] = "Whisper文字起こし中 (AMD RX 9060 XT / Vulkan)..."
        update_task_file(task_file_path, task_data)

        whisper_cmd = [
            str(whisper_bin),
            "-m", str(model_file),
            "-f", str(temp_wav),
            "-osrt",
            "-of", str(srt_base),
            "-l", "ja",
            "-t", "8",
            "-bs", "2",
            "-nth", "0.60",
            "-et", "2.40",
            "-nf",
            "-mc", "0"
        ]
        whisper_env = os.environ.copy()
        whisper_env["GGML_VULKAN_DEVICE"] = "0"

        subprocess.run(
            whisper_cmd,
            env=whisper_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            creationflags=CREATE_NO_WINDOW
        )

        if not srt_output.exists() or srt_output.stat().st_size == 0:
            return False, "Whisperによる字幕(SRT)の生成に失敗しました。"

        elapsed_whisper = time.time() - start_whisper_time

        # 3. Markdownファイルの書き出し
        if save_md:
            base_stem = media_path.stem
            md_path = output_dir / f"{base_stem}.md"
            generate_markdown(srt_output, md_path, media_path.name, model_name, elapsed_whisper)

        # 4. 字幕の動画への統合
        task_data["status"] = "converting"
        task_data["speed"] = "字幕を動画に埋め込み中..." if not burn_subtitles else "字幕を映像に焼き付け中 (エンコード)..."
        update_task_file(task_file_path, task_data)

        ext = media_path.suffix.lower()
        temp_merged = output_dir / f"temp_subbed_{pid}{ext}"

        if not burn_subtitles:
            # ソフトサブ（埋め込み・無劣化・サムネイル・チャプター等完全保持）
            subtitle_codec = "mov_text" if ext == ".mp4" else "subrip"
            merge_cmd = [
                "ffmpeg", "-y",
                "-i", str(media_path),
                "-i", str(srt_output),
                "-map", "0",
                "-map", "1:s",
                "-c", "copy",
                "-c:s", subtitle_codec,
                "-metadata:s:s:0", "language=jpn",
                "-metadata:s:s:0", "title=Whisper AI",
                str(temp_merged)
            ]
            m_res = subprocess.run(merge_cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=CREATE_NO_WINDOW)
            if m_res.returncode == 0 and temp_merged.exists() and temp_merged.stat().st_size > 0:
                media_path.unlink()
                temp_merged.rename(media_path)
            else:
                return False, "字幕の埋め込み処理(ffmpeg)に失敗しました。"
        else:
            # ハードサブ（映像への直接焼き付け）
            # Windowsパスのコロンやバックスラッシュ問題を回避するため、カレントディレクトリでファイル名指定
            srt_filename = srt_output.name
            merge_cmd = [
                "ffmpeg", "-y",
                "-i", str(media_path),
                "-vf", f"subtitles='{srt_filename}'",
                "-c:a", "copy",
                "-c:v", "libx264",
                "-crf", "18",
                "-preset", "fast",
                str(temp_merged)
            ]
            m_res = subprocess.run(merge_cmd, cwd=str(output_dir), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=CREATE_NO_WINDOW)
            if m_res.returncode == 0 and temp_merged.exists() and temp_merged.stat().st_size > 0:
                media_path.unlink()
                temp_merged.rename(media_path)
            else:
                return False, "字幕の焼き付け処理(ffmpeg)に失敗しました。"

        return True, None

    except Exception as e:
        return False, str(e)
    finally:
        for temp_p in [temp_wav, srt_output]:
            if temp_p.exists():
                try:
                    temp_p.unlink()
                except Exception:
                    pass

def cleanup_standalone_thumbnails(output_dir, final_filepath):
    """Ensure temporary artifacts are cleaned up without removing markdown transcripts."""
    if not output_dir.exists():
        return

    for f in output_dir.iterdir():
        if f.is_file():
            # Clean up standalone temp files, intermediate subtitles, parts, etc.
            if f.suffix.lower() in [".vtt", ".ttml", ".part", ".ytdl", ".temp", ".wav"]:
                try:
                    f.unlink()
                except Exception:
                    pass
            # Standalone images (not the merged video)
            elif f.suffix.lower() in [".jpg", ".jpeg", ".png", ".webp"] and "temp_" in f.name:
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
    whisper_enabled = task_data.get("whisper_enabled", False)
    custom_args = task_data.get("custom_args", "").strip()
    download_dir = task_data.get("download_dir", "").strip()
    show_console = task_data.get("show_console", False)

    base_dir = pathlib.Path(__file__).resolve().parent
    output_dir = (base_dir / (download_dir if download_dir else "../output")).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    working_dir = str(output_dir)

    cookies_from_browser = task_data.get("cookies_from_browser", "").strip()
    cookies_file = task_data.get("cookies_file", "").strip()

    # Base yt-dlp command configured to merge everything into a single file
    cmd = [
        "yt-dlp",
        "--newline",
        "--windows-filenames",
        "--no-mtime",
        "--remote-components", "ejs:github",
        "-P", working_dir
    ]

    if cookies_from_browser and cookies_from_browser.lower() != "auto":
        cmd.extend(["--cookies-from-browser", cookies_from_browser])
    elif cookies_file:
        cookie_path = pathlib.Path(cookies_file)
        if not cookie_path.is_absolute():
            cookie_path = (base_dir / cookie_path).resolve()
        if cookie_path.exists():
            cmd.extend(["--cookies", str(cookie_path)])

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
                "--parse-metadata", "%(artist,creator,uploader,channel)s:%(artist)s",
                "--parse-metadata", "%(description)s:%(comment)s"
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
                "--parse-metadata", "%(artist,creator,uploader,channel)s:%(artist)s",
                "--parse-metadata", "%(description)s:%(comment)s"
            ])

        if embed_thumbnail:
            cmd.extend(["--embed-thumbnail", "--convert-thumbnails", "jpg"])

        if embed_subtitles and not whisper_enabled:
            cmd.extend([
                "--write-subs",
                "--write-auto-subs",
                "--sub-langs", "ja.*,en.*",
                "--embed-subs",
                "--compat-options", "no-keep-subs",
                "--sleep-subtitles", "2",
                "-i"
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

            # Detect destination filename (ignore subtitle and temp files)
            dest_match = DESTINATION_REGEX.search(clean_line)
            if dest_match:
                detected_dest = dest_match.group(1).strip()
                filename = os.path.basename(detected_dest)
                ignored_exts = [".vtt", ".srt", ".ttml", ".part", ".ytdl", ".temp", ".jpg", ".jpeg", ".png", ".webp"]
                if not any(filename.lower().endswith(ext) for ext in ignored_exts):
                    final_detected_filepath = detected_dest
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

        # Check if actual media file was created
        media_file_exists = False
        if final_detected_filepath and os.path.exists(final_detected_filepath):
            media_file_exists = True
        elif output_dir.exists():
            for f in output_dir.iterdir():
                if f.is_file() and f.suffix.lower() in [".mp4", ".mkv", ".webm", ".mp3", ".flac", ".m4a", ".opus", ".wav"]:
                    if f.stat().st_mtime >= task_data["started_at"]:
                        media_file_exists = True
                        final_detected_filepath = str(f)
                        break

        task_data["finished_at"] = time.time()
        # If exit code is 0 or media exists despite non-fatal subtitle error
        is_subtitle_only_error = fatal_error and ("subtitles" in fatal_error.lower() or "429" in fatal_error)
        if proc.returncode == 0 or (media_file_exists and is_subtitle_only_error):
            whisper_err = None
            if whisper_enabled and media_file_exists:
                w_success, w_msg = apply_whisper_transcription(final_detected_filepath, output_dir, task_data, task_file_path)
                if not w_success:
                    whisper_err = w_msg

            task_data["finished_at"] = time.time()
            task_data["status"] = "completed"
            task_data["percent"] = 100.0
            task_data["speed"] = "完了 (Whisper字幕付き)" if (whisper_enabled and not whisper_err) else "完了"
            task_data["eta"] = "00:00"
            task_data["error"] = whisper_err

            # Clean up standalone temp/thumbnail files so only the single file remains
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
