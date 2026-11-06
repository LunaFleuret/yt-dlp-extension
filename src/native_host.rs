use crate::config::{self, Config};
use crate::downloader::TaskInfo;
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;
const DETACHED_PROCESS: u32 = 0x00000008;

pub fn read_native_message() -> Result<Option<Value>> {
    let mut len_buf = [0u8; 4];
    let mut stdin = std::io::stdin().lock();
    if stdin.read_exact(&mut len_buf).is_err() {
        return Ok(None);
    }
    let length = u32::from_le_bytes(len_buf) as usize;
    if length == 0 {
        return Ok(None);
    }

    let mut content_buf = vec![0u8; length];
    stdin.read_exact(&mut content_buf)?;
    let msg: Value = serde_json::from_slice(&content_buf)?;
    Ok(Some(msg))
}

pub fn send_native_message(msg: &Value) -> Result<()> {
    let content = serde_json::to_vec(msg)?;
    let length = content.len() as u32;
    let len_buf = length.to_le_bytes();

    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&len_buf)?;
    stdout.write_all(&content)?;
    stdout.flush()?;
    Ok(())
}

pub fn get_base_dir() -> PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            return parent.to_path_buf();
        }
    }
    PathBuf::from(".")
}

pub fn get_tasks_dir(base_dir: &Path) -> PathBuf {
    let tasks_dir = base_dir.join("tasks");
    let _ = std::fs::create_dir_all(&tasks_dir);
    tasks_dir
}

pub fn handle_ping(config: &Config) -> Value {
    json!({
        "status": "pong",
        "config": config
    })
}

pub fn handle_get_tasks(base_dir: &Path) -> Value {
    let tasks_dir = get_tasks_dir(base_dir);
    let mut tasks: Vec<TaskInfo> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&tasks_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    if let Ok(task) = serde_json::from_str::<TaskInfo>(&content) {
                        tasks.push(task);
                    }
                }
            }
        }
    }

    tasks.sort_by(|a, b| b.created_at.partial_cmp(&a.created_at).unwrap_or(std::cmp::Ordering::Equal));
    json!({
        "status": "success",
        "tasks": tasks
    })
}

pub fn handle_cancel_task(base_dir: &Path, task_id: &str) -> Value {
    let tasks_dir = get_tasks_dir(base_dir);
    let task_file = tasks_dir.join(format!("task_{}.json", task_id));

    if !task_file.is_file() {
        return json!({ "status": "error", "error": "Task not found" });
    }

    if let Ok(content) = std::fs::read_to_string(&task_file) {
        if let Ok(mut task) = serde_json::from_str::<TaskInfo>(&content) {
            if let Some(pid) = task.pid {
                #[cfg(windows)]
                {
                    let _ = Command::new("taskkill")
                        .args(["/F", "/T", "/PID", &pid.to_string()])
                        .creation_flags(CREATE_NO_WINDOW)
                        .output();
                }
            }

            task.status = "cancelled".to_string();
            let now_sec = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
            task.finished_at = Some(now_sec);
            let _ = std::fs::write(&task_file, serde_json::to_string_pretty(&task).unwrap_or_default());

            return json!({ "status": "success", "message": "Task cancelled" });
        }
    }

    json!({ "status": "error", "error": "Failed to update task" })
}

pub fn handle_clear_finished(base_dir: &Path) -> Value {
    let tasks_dir = get_tasks_dir(base_dir);
    let mut cleared = 0;

    if let Ok(entries) = std::fs::read_dir(&tasks_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    if let Ok(task) = serde_json::from_str::<TaskInfo>(&content) {
                        if ["completed", "error", "cancelled"].contains(&task.status.as_str()) {
                            let cookie_file = tasks_dir.join(format!("task_{}_cookies.txt", task.id));
                            let _ = std::fs::remove_file(cookie_file);
                            let _ = std::fs::remove_file(&p);
                            cleared += 1;
                        }
                    }
                }
            }
        }
    }

    json!({ "status": "success", "cleared_count": cleared })
}

pub fn handle_open_output_dir(base_dir: &Path, msg: &Value, config: &Config) -> Value {
    let mut dir_str = msg.get("download_dir").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if dir_str.is_empty() {
        dir_str = config.download_dir.trim().to_string();
    }
    if dir_str.is_empty() {
        dir_str = "../output".to_string();
    }

    let p = PathBuf::from(&dir_str);
    let target = if p.is_absolute() { p } else { base_dir.join(p) };
    let _ = std::fs::create_dir_all(&target);

    #[cfg(windows)]
    {
        let _ = Command::new("explorer.exe")
            .arg(&target)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }

    json!({
        "status": "success",
        "message": format!("Opened {:?}", target)
    })
}

pub fn handle_download(base_dir: &Path, msg: &Value, config: &Config) -> Value {
    let url = msg.get("url").and_then(|v| v.as_str()).unwrap_or("").trim();
    if url.is_empty() {
        return json!({ "status": "error", "error": "No URL provided" });
    }

    let raw_uuid = uuid::Uuid::new_v4().to_string();
    let task_id = raw_uuid[..8].to_string();

    let tasks_dir = get_tasks_dir(base_dir);
    let task_file = tasks_dir.join(format!("task_{}.json", task_id));

    let title = msg.get("title").and_then(|v| v.as_str()).unwrap_or(url).to_string();
    let download_type = msg.get("download_type").and_then(|v| v.as_str()).unwrap_or("video").to_string();
    let video_quality = msg.get("video_quality").and_then(|v| v.as_str()).unwrap_or("best").to_string();
    let video_format = msg.get("video_format").and_then(|v| v.as_str()).unwrap_or("mp4").to_string();
    let audio_format = msg.get("audio_format").and_then(|v| v.as_str()).unwrap_or("mp3").to_string();
    let normalize_audio = msg.get("normalize_audio").and_then(|v| v.as_bool()).unwrap_or(config.normalize_audio);
    let embed_thumbnail = msg.get("embed_thumbnail").and_then(|v| v.as_bool()).unwrap_or(config.embed_thumbnail);
    let embed_metadata = msg.get("embed_metadata").and_then(|v| v.as_bool()).unwrap_or(config.embed_metadata);
    let embed_subtitles = msg.get("embed_subtitles").and_then(|v| v.as_bool()).unwrap_or(config.embed_subtitles);
    let whisper_enabled = msg.get("whisper_enabled").and_then(|v| v.as_bool()).unwrap_or(config.whisper_enabled);
    let whisper_model = msg.get("whisper_model").and_then(|v| v.as_str()).unwrap_or(&config.whisper_model).to_string();
    let whisper_burn_subtitles = msg.get("whisper_burn_subtitles").and_then(|v| v.as_bool()).unwrap_or(config.whisper_burn_subtitles);
    let whisper_save_md = msg.get("whisper_save_md").and_then(|v| v.as_bool()).unwrap_or(config.whisper_save_md);
    let whisper_bin_path = config.whisper_bin_path.clone();
    let whisper_models_dir = config.whisper_models_dir.clone();
    let custom_args = msg.get("custom_args").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let download_dir = msg.get("download_dir").and_then(|v| v.as_str()).unwrap_or(&config.download_dir).trim().to_string();
    let cookies_from_browser = msg.get("cookies_from_browser").and_then(|v| v.as_str()).unwrap_or(&config.cookies_from_browser).trim().to_string();
    let mut cookies_file = msg.get("cookies_file").and_then(|v| v.as_str()).unwrap_or(&config.cookies_file).trim().to_string();

    let cookies_data = msg.get("cookies_data").and_then(|v| v.as_str()).unwrap_or("").trim();
    if !cookies_data.is_empty() {
        let task_cookies_p = tasks_dir.join(format!("task_{}_cookies.txt", task_id));
        if std::fs::write(&task_cookies_p, cookies_data).is_ok() {
            cookies_file = task_cookies_p.to_string_lossy().to_string();
        }
    }

    let now_sec = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;

    let task_info = TaskInfo {
        id: task_id.clone(),
        url: url.to_string(),
        title,
        download_type,
        video_quality,
        video_format,
        audio_format,
        normalize_audio,
        embed_thumbnail,
        embed_metadata,
        embed_subtitles,
        whisper_enabled,
        whisper_model,
        whisper_burn_subtitles,
        whisper_save_md,
        whisper_bin_path,
        whisper_models_dir,
        custom_args,
        download_dir,
        cookies_from_browser,
        cookies_file,
        show_console: false,
        status: "queued".to_string(),
        percent: 0.0,
        speed: "Starting...".to_string(),
        eta: "--:--".to_string(),
        total_size: "Unknown".to_string(),
        error: None,
        created_at: now_sec,
        started_at: None,
        finished_at: None,
        pid: None,
    };

    if let Ok(s) = serde_json::to_string_pretty(&task_info) {
        let _ = std::fs::write(&task_file, s);
    }

    // Launch worker detached process
    if let Ok(exe_path) = std::env::current_exe() {
        #[cfg(windows)]
        {
            let spawn_res = Command::new(exe_path)
                .arg("worker")
                .arg(&task_file)
                .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
                .spawn();

            if let Err(e) = spawn_res {
                return json!({ "status": "error", "error": format!("Failed to launch worker: {}", e) });
            }
        }
        #[cfg(not(windows))]
        {
            let spawn_res = Command::new(exe_path)
                .arg("worker")
                .arg(&task_file)
                .spawn();

            if let Err(e) = spawn_res {
                return json!({ "status": "error", "error": format!("Failed to launch worker: {}", e) });
            }
        }
    }

    json!({
        "status": "success",
        "task_id": task_id,
        "task": task_info
    })
}

pub fn run_native_host() -> Result<()> {
    let base_dir = get_base_dir();
    let mut config = config::load_config(&base_dir);

    while let Ok(Some(msg)) = read_native_message() {
        let action = msg.get("action").and_then(|v| v.as_str()).unwrap_or("download");

        let response = match action {
            "ping" => handle_ping(&config),
            "download" => handle_download(&base_dir, &msg, &config),
            "get_tasks" => handle_get_tasks(&base_dir),
            "cancel_task" => {
                let tid = msg.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
                handle_cancel_task(&base_dir, tid)
            }
            "clear_finished" => handle_clear_finished(&base_dir),
            "open_output_dir" => handle_open_output_dir(&base_dir, &msg, &config),
            "update_config" => {
                if let Some(new_cfg) = msg.get("config") {
                    if let Ok(mut map) = serde_json::from_value::<serde_json::Map<String, Value>>(new_cfg.clone()) {
                        let cur_val = serde_json::to_value(&config).unwrap_or(json!({}));
                        if let Some(cur_map) = cur_val.as_object() {
                            for (k, v) in cur_map {
                                if !map.contains_key(k) {
                                    map.insert(k.clone(), v.clone());
                                }
                            }
                        }
                        if let Ok(updated) = serde_json::from_value::<Config>(Value::Object(map)) {
                            config = updated;
                            let _ = config::save_config(&base_dir, &config);
                        }
                    }
                }
                json!({ "status": "success", "message": "Config updated", "config": config })
            }
            _ => json!({ "status": "error", "error": format!("Unknown action: {}", action) }),
        };

        let _ = send_native_message(&response);
    }

    Ok(())
}
