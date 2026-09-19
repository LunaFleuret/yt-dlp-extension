#!/usr/bin/env python
# -*- coding: utf-8 -*-

import sys
import os
import json
import time
import struct
import uuid
import pathlib
import subprocess

def send_message(message_dict):
    """Send a message back to Chrome/Edge via standard output."""
    encoded_content = json.dumps(message_dict, ensure_ascii=False).encode("utf-8")
    length = len(encoded_content)
    sys.stdout.buffer.write(struct.pack("I", length))
    sys.stdout.buffer.write(encoded_content)
    sys.stdout.buffer.flush()

def read_message():
    """Read a message from Chrome/Edge via standard input."""
    raw_length = sys.stdin.buffer.read(4)
    if not raw_length:
        return None
    length = struct.unpack("I", raw_length)[0]
    raw_content = sys.stdin.buffer.read(length)
    return json.loads(raw_content.decode("utf-8"))

def get_base_dir():
    """Get the directory where host.py is located."""
    return pathlib.Path(__file__).resolve().parent

def get_tasks_dir():
    """Get or create tasks storage directory."""
    tasks_dir = get_base_dir() / "tasks"
    tasks_dir.mkdir(parents=True, exist_ok=True)
    return tasks_dir

def load_config():
    """Load configuration from config.json."""
    base_dir = get_base_dir()
    config_file = base_dir / "config.json"
    default_config = {
        "bat_path": "../yt-dlp/yt-dlp.bat",
        "use_bat": True,
        "download_dir": "../output",
        "show_console": False,
        "keep_console_open": False,
        "cookies_from_browser": "",
        "cookies_file": ""
    }
    if config_file.exists():
        try:
            with open(config_file, "r", encoding="utf-8") as f:
                loaded = json.load(f)
                default_config.update(loaded)
        except Exception:
            pass
    return default_config

def resolve_bat_path(config):
    """Resolve bat file path relative to host directory."""
    base_dir = get_base_dir()
    raw_path = config.get("bat_path", "../yt-dlp/yt-dlp.bat")
    path_obj = pathlib.Path(raw_path)
    if path_obj.is_absolute():
        return str(path_obj)
    return str((base_dir / path_obj).resolve())

def start_download(msg, config):
    """Start a detached download worker."""
    url = msg.get("url", "").strip()
    if not url:
        return {"status": "error", "error": "No URL provided"}

    task_id = str(uuid.uuid4())[:8]
    title = msg.get("title") or url
    download_type = msg.get("download_type", "video")
    video_quality = msg.get("video_quality", "best")
    video_format = msg.get("video_format", "mp4")
    audio_format = msg.get("audio_format", "mp3")
    embed_thumbnail = msg.get("embed_thumbnail", config.get("embed_thumbnail", True))
    embed_metadata = msg.get("embed_metadata", config.get("embed_metadata", True))
    embed_subtitles = msg.get("embed_subtitles", config.get("embed_subtitles", False))
    custom_args = msg.get("custom_args", "").strip()
    show_console = msg.get("show_console", config.get("show_console", False))
    download_dir = msg.get("download_dir", config.get("download_dir", "")).strip()
    cookies_from_browser = msg.get("cookies_from_browser", config.get("cookies_from_browser", "")).strip()
    cookies_file = msg.get("cookies_file", config.get("cookies_file", "")).strip()
    cookies_data = msg.get("cookies_data", "").strip()

    whisper_enabled = msg.get("whisper_enabled", config.get("whisper_enabled", False))
    whisper_model = msg.get("whisper_model", config.get("whisper_model", "large-v3-turbo"))
    whisper_burn_subtitles = msg.get("whisper_burn_subtitles", config.get("whisper_burn_subtitles", False))
    whisper_save_md = msg.get("whisper_save_md", config.get("whisper_save_md", False))
    whisper_bin_path = config.get("whisper_bin_path", "C:/AI-Models/Whisper-Models/bin/whisper-cli.exe")
    whisper_models_dir = config.get("whisper_models_dir", "C:/AI-Models/Whisper-Models")

    tasks_dir = get_tasks_dir()
    task_file = tasks_dir / f"task_{task_id}.json"

    # Save direct cookies data if provided
    if cookies_data:
        task_cookies_file = tasks_dir / f"task_{task_id}_cookies.txt"
        try:
            with open(task_cookies_file, "w", encoding="utf-8") as cf:
                cf.write(cookies_data)
            cookies_file = str(task_cookies_file)
            cookies_from_browser = ""
        except Exception:
            pass

    task_info = {
        "id": task_id,
        "url": url,
        "title": title,
        "download_type": download_type,
        "video_quality": video_quality,
        "video_format": video_format,
        "audio_format": audio_format,
        "embed_thumbnail": embed_thumbnail,
        "embed_metadata": embed_metadata,
        "embed_subtitles": embed_subtitles,
        "whisper_enabled": whisper_enabled,
        "whisper_model": whisper_model,
        "whisper_burn_subtitles": whisper_burn_subtitles,
        "whisper_save_md": whisper_save_md,
        "whisper_bin_path": whisper_bin_path,
        "whisper_models_dir": whisper_models_dir,
        "custom_args": custom_args,
        "download_dir": download_dir,
        "cookies_from_browser": cookies_from_browser,
        "cookies_file": cookies_file,
        "show_console": show_console,
        "status": "queued",
        "percent": 0.0,
        "speed": "Starting...",
        "eta": "--:--",
        "total_size": "Unknown",
        "error": None,
        "created_at": time.time(),
        "started_at": None,
        "finished_at": None,
        "pid": None
    }

    # Save initial task file
    with open(task_file, "w", encoding="utf-8") as f:
        json.dump(task_info, f, ensure_ascii=False, indent=2)

    # Launch worker.py as a completely detached independent process
    base_dir = get_base_dir()
    worker_script = str(base_dir / "worker.py")
    
    CREATE_NO_WINDOW = 0x08000000
    DETACHED_PROCESS = 0x00000008

    try:
        subprocess.Popen(
            [sys.executable, worker_script, str(task_file)],
            cwd=str(base_dir),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            creationflags=CREATE_NO_WINDOW | DETACHED_PROCESS
        )
        return {
            "status": "success",
            "task_id": task_id,
            "task": task_info
        }
    except Exception as e:
        task_info["status"] = "error"
        task_info["error"] = str(e)
        with open(task_file, "w", encoding="utf-8") as f:
            json.dump(task_info, f, ensure_ascii=False, indent=2)
        return {"status": "error", "error": str(e)}

def get_all_tasks():
    """Read all task JSON files from tasks directory."""
    tasks_dir = get_tasks_dir()
    task_files = list(tasks_dir.glob("task_*.json"))
    tasks = []

    for tf in task_files:
        try:
            with open(tf, "r", encoding="utf-8") as f:
                task_data = json.load(f)
                tasks.append(task_data)
        except Exception:
            continue

    # Sort descending by creation time
    tasks.sort(key=lambda x: x.get("created_at", 0), reverse=True)
    return tasks

def cancel_task(task_id):
    """Kill worker process and mark task as cancelled."""
    tasks_dir = get_tasks_dir()
    task_file = tasks_dir / f"task_{task_id}.json"
    if not task_file.exists():
        return {"status": "error", "error": "Task not found"}

    try:
        with open(task_file, "r", encoding="utf-8") as f:
            task_data = json.load(f)

        pid = task_data.get("pid")
        if pid:
            # Kill process tree in Windows
            subprocess.run(
                ["taskkill", "/F", "/T", "/PID", str(pid)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL
            )

        task_data["status"] = "cancelled"
        task_data["finished_at"] = time.time()

        with open(task_file, "w", encoding="utf-8") as f:
            json.dump(task_data, f, ensure_ascii=False, indent=2)

        return {"status": "success", "message": "Task cancelled"}
    except Exception as e:
        return {"status": "error", "error": str(e)}

def clear_finished_tasks():
    """Remove finished, error, or cancelled task files."""
    tasks_dir = get_tasks_dir()
    task_files = list(tasks_dir.glob("task_*.json"))
    cleared = 0

    for tf in task_files:
        try:
            with open(tf, "r", encoding="utf-8") as f:
                task_data = json.load(f)
            if task_data.get("status") in ["completed", "error", "cancelled"]:
                tid = task_data.get("id")
                if tid:
                    cookie_f = tasks_dir / f"task_{tid}_cookies.txt"
                    cookie_f.unlink(missing_ok=True)
                tf.unlink(missing_ok=True)
                cleared += 1
        except Exception:
            continue

    return {"status": "success", "cleared_count": cleared}

def open_output_dir(msg, config):
    """Open the output download directory in Files app (GitHub/MS Store) or OS file manager."""
    base_dir = get_base_dir()
    download_dir = msg.get("download_dir", config.get("download_dir", "../output")).strip()
    if not download_dir:
        download_dir = "../output"

    target_path = pathlib.Path(download_dir)
    if not target_path.is_absolute():
        target_path = (base_dir / target_path).resolve()

    target_path.mkdir(parents=True, exist_ok=True)

    CREATE_NO_WINDOW = 0x08000000

    try:
        if sys.platform == "win32":
            opened = False
            # 1. Priority: GitHub Release Preview (files-preview.exe)
            try:
                subprocess.Popen(["files-preview.exe", str(target_path)], creationflags=CREATE_NO_WINDOW)
                opened = True
            except Exception:
                pass

            # 2. Priority: GitHub Release Stable (files-stable.exe)
            if not opened:
                try:
                    subprocess.Popen(["files-stable.exe", str(target_path)], creationflags=CREATE_NO_WINDOW)
                    opened = True
                except Exception:
                    pass

            # 3. Priority: URI Protocol (files-preview-uwp: / files-uwp:)
            if not opened:
                for uri_scheme in ["files-preview-uwp:", "files-uwp:"]:
                    try:
                        subprocess.Popen(
                            ["cmd.exe", "/c", "start", "", f"{uri_scheme}{target_path}"],
                            creationflags=CREATE_NO_WINDOW
                        )
                        opened = True
                        break
                    except Exception:
                        pass

            # 4. Fallback: Windows Explorer
            if not opened:
                subprocess.Popen(["explorer.exe", str(target_path)], creationflags=CREATE_NO_WINDOW)

        elif sys.platform == "darwin":
            subprocess.Popen(["open", str(target_path)])
        else:
            subprocess.Popen(["xdg-open", str(target_path)])
        return {"status": "success", "message": f"Opened {target_path}"}
    except Exception as e:
        return {"status": "error", "error": str(e)}



def main():

    config = load_config()
    while True:
        try:
            msg = read_message()
            if msg is None:
                break

            action = msg.get("action", "download")

            if action == "ping":
                send_message({"status": "pong", "config": config})
            elif action == "download":
                result = start_download(msg, config)
                send_message(result)
            elif action == "get_tasks":
                tasks = get_all_tasks()
                send_message({"status": "success", "tasks": tasks})
            elif action == "cancel_task":
                task_id = msg.get("task_id", "")
                result = cancel_task(task_id)
                send_message(result)
            elif action == "clear_finished":
                result = clear_finished_tasks()
                send_message(result)
            elif action == "open_output_dir":
                result = open_output_dir(msg, config)
                send_message(result)
            elif action == "update_config":

                new_conf = msg.get("config", {})
                config.update(new_conf)
                base_dir = get_base_dir()
                with open(base_dir / "config.json", "w", encoding="utf-8") as f:
                    json.dump(config, f, indent=2)
                send_message({"status": "success", "message": "Config updated", "config": config})
            else:
                send_message({"status": "error", "error": f"Unknown action: {action}"})
        except Exception as e:
            try:
                send_message({"status": "error", "error": str(e)})
            except Exception:
                pass
            break

if __name__ == "__main__":
    main()
