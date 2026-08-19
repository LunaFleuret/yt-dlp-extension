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
        "keep_console_open": False
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

    tasks_dir = get_tasks_dir()
    task_file = tasks_dir / f"task_{task_id}.json"

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
        "custom_args": custom_args,
        "download_dir": download_dir,
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
                tf.unlink(missing_ok=True)
                cleared += 1
        except Exception:
            continue

    return {"status": "success", "cleared_count": cleared}

def open_output_dir(msg, config):
    """Open the output download directory in OS file manager."""
    base_dir = get_base_dir()
    download_dir = msg.get("download_dir", config.get("download_dir", "../output")).strip()
    if not download_dir:
        download_dir = "../output"

    target_path = pathlib.Path(download_dir)
    if not target_path.is_absolute():
        target_path = (base_dir / target_path).resolve()

    target_path.mkdir(parents=True, exist_ok=True)

    try:
        if sys.platform == "win32":
            os.startfile(str(target_path))
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
