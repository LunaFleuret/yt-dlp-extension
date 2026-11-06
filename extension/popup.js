document.addEventListener("DOMContentLoaded", async () => {
  // Elements
  const tabNewBtn = document.getElementById("tabNewBtn");
  const tabTasksBtn = document.getElementById("tabTasksBtn");
  const viewNew = document.getElementById("viewNew");
  const viewTasks = document.getElementById("viewTasks");
  const activeTasksBadge = document.getElementById("activeTasksBadge");

  const pageTitleEl = document.getElementById("pageTitle");
  const pageUrlInput = document.getElementById("pageUrl");
  const openOptionsBtn = document.getElementById("openOptionsBtn");

  // Type Segments & Panels
  const typeRadios = document.querySelectorAll('input[name="downloadType"]');
  const segVideo = document.getElementById("segVideo");
  const segAudio = document.getElementById("segAudio");
  const segCustom = document.getElementById("segCustom");
  const panelVideo = document.getElementById("panelVideo");
  const panelAudio = document.getElementById("panelAudio");
  const panelCustom = document.getElementById("panelCustom");

  // Form Controls
  const videoQualitySelect = document.getElementById("videoQuality");
  const videoFormatSelect = document.getElementById("videoFormat");
  const videoNormalizeAudioCheckbox = document.getElementById("videoNormalizeAudio");
  const videoThumbnailCheckbox = document.getElementById("videoThumbnail");
  const videoSubtitlesCheckbox = document.getElementById("videoSubtitles");
  const whisperSubtitlesCheckbox = document.getElementById("whisperSubtitles");
  const whisperSubOptions = document.getElementById("whisperSubOptions");
  const whisperBurnModeSelect = document.getElementById("whisperBurnMode");

  const audioFormatSelect = document.getElementById("audioFormat");
  const audioNormalizeAudioCheckbox = document.getElementById("audioNormalizeAudio");
  const audioThumbnailCheckbox = document.getElementById("audioThumbnail");
  const audioWhisperCheckbox = document.getElementById("audioWhisper");

  const customArgsInput = document.getElementById("customArgs");

  const downloadBtn = document.getElementById("downloadBtn");
  const btnText = downloadBtn.querySelector(".btn-text");
  const btnSpinner = document.getElementById("btnSpinner");

  const downloadWhisperBtn = document.getElementById("downloadWhisperBtn");
  const whisperBtnText = downloadWhisperBtn.querySelector(".btn-text");
  const whisperBtnSpinner = document.getElementById("whisperBtnSpinner");

  const statusBanner = document.getElementById("statusBanner");
  const statusText = document.getElementById("statusText");

  const tasksList = document.getElementById("tasksList");
  const emptyTasksState = document.getElementById("emptyTasksState");
  const tasksCountLabel = document.getElementById("tasksCountLabel");
  const clearFinishedBtn = document.getElementById("clearFinishedBtn");

  let pollInterval = null;
  let currentTabTitle = "";

  // Whisper & Subtitles toggle correlation
  whisperSubtitlesCheckbox.addEventListener("change", () => {
    whisperSubOptions.classList.toggle("hidden", !whisperSubtitlesCheckbox.checked);
    if (whisperSubtitlesCheckbox.checked) {
      videoSubtitlesCheckbox.checked = false;
    }
  });

  videoSubtitlesCheckbox.addEventListener("change", () => {
    if (videoSubtitlesCheckbox.checked) {
      whisperSubtitlesCheckbox.checked = false;
      whisperSubOptions.classList.add("hidden");
    }
  });

  // Tab switching
  tabNewBtn.addEventListener("click", () => switchTab("new"));
  tabTasksBtn.addEventListener("click", () => switchTab("tasks"));

  function switchTab(tabName) {
    if (tabName === "new") {
      tabNewBtn.classList.add("active");
      tabTasksBtn.classList.remove("active");
      viewNew.classList.remove("hidden");
      viewTasks.classList.add("hidden");
    } else {
      tabTasksBtn.classList.add("active");
      tabNewBtn.classList.remove("active");
      viewTasks.classList.remove("hidden");
      viewNew.classList.add("hidden");
      renderTasks();
    }
  }

  // Type Segment switching
  function switchDownloadType(type) {
    segVideo.classList.toggle("active", type === "video");
    segAudio.classList.toggle("active", type === "audio");
    segCustom.classList.toggle("active", type === "custom");

    panelVideo.classList.toggle("hidden", type !== "video");
    panelAudio.classList.toggle("hidden", type !== "audio");
    panelCustom.classList.toggle("hidden", type !== "custom");
  }

  typeRadios.forEach(radio => {
    radio.addEventListener("change", (e) => {
      switchDownloadType(e.target.value);
    });
  });

  // Load saved preferences
  chrome.storage.sync.get([
    "savedType",
    "savedVideoQuality",
    "savedVideoFormat",
    "savedAudioFormat",
    "savedCustomArgs",
    "savedVideoNormalizeAudio",
    "savedAudioNormalizeAudio",
    "savedVideoThumbnail",
    "savedVideoSubtitles",
    "savedWhisperSubtitles",
    "savedWhisperBurnMode",
    "savedAudioThumbnail",
    "savedAudioWhisper"
  ], (items) => {
    if (items.savedType) {
      const radio = document.querySelector(`input[name="downloadType"][value="${items.savedType}"]`);
      if (radio) {
        radio.checked = true;
        switchDownloadType(items.savedType);
      }
    }
    if (items.savedVideoQuality) videoQualitySelect.value = items.savedVideoQuality;
    if (items.savedVideoFormat) videoFormatSelect.value = items.savedVideoFormat;
    if (items.savedAudioFormat) audioFormatSelect.value = items.savedAudioFormat;
    if (items.savedCustomArgs && customArgsInput) customArgsInput.value = items.savedCustomArgs;
    if (items.savedVideoNormalizeAudio !== undefined) videoNormalizeAudioCheckbox.checked = items.savedVideoNormalizeAudio;
    if (items.savedAudioNormalizeAudio !== undefined) audioNormalizeAudioCheckbox.checked = items.savedAudioNormalizeAudio;
    if (items.savedVideoThumbnail !== undefined) videoThumbnailCheckbox.checked = items.savedVideoThumbnail;
    if (items.savedVideoSubtitles !== undefined) videoSubtitlesCheckbox.checked = items.savedVideoSubtitles;
    if (items.savedWhisperSubtitles !== undefined) {
      whisperSubtitlesCheckbox.checked = items.savedWhisperSubtitles;
      whisperSubOptions.classList.toggle("hidden", !items.savedWhisperSubtitles);
    }
    if (items.savedWhisperBurnMode) whisperBurnModeSelect.value = items.savedWhisperBurnMode;
    if (items.savedAudioThumbnail !== undefined) audioThumbnailCheckbox.checked = items.savedAudioThumbnail;
    if (items.savedAudioWhisper !== undefined) audioWhisperCheckbox.checked = items.savedAudioWhisper;
  });

  // Helper to sanitize YouTube playlist/mix URLs
  function sanitizeUrl(rawUrl) {
    if (!rawUrl) return "";
    let u = rawUrl.trim();
    const m = u.match(/(?:youtube\.com\/watch\?v=|youtu\.be\/)([\w-]+)/);
    if (m && u.includes("list=")) {
      const videoId = m[1];
      if (u.includes("music.youtube.com")) {
        return `https://music.youtube.com/watch?v=${videoId}`;
      }
      return `https://www.youtube.com/watch?v=${videoId}`;
    }
    return u;
  }

  // Fetch active tab
  try {
    const [activeTab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (activeTab) {
      currentTabTitle = activeTab.title || "";
      pageTitleEl.textContent = activeTab.title || "タイトルなし";
      pageUrlInput.value = sanitizeUrl(activeTab.url || "");
    }
  } catch (e) {
    pageTitleEl.textContent = "ページ情報を取得できませんでした";
  }

  // Open Options
  openOptionsBtn.addEventListener("click", () => {
    if (chrome.runtime.openOptionsPage) {
      chrome.runtime.openOptionsPage();
    } else {
      window.open(chrome.runtime.getURL("options.html"));
    }
  });

  // Unified Download Handler
  async function triggerDownload(forceWhisper = null) {
    const url = sanitizeUrl(pageUrlInput.value.trim());
    if (!url) {
      showStatus("有効な URL を入力してください。", "error");
      return;
    }

    const selectedType = document.querySelector('input[name="downloadType"]:checked').value;
    const videoQuality = videoQualitySelect.value;
    const videoFormat = videoFormatSelect.value;
    const audioFormat = audioFormatSelect.value;
    const customArgs = customArgsInput.value.trim();

    const embedThumbnail = selectedType === "video" ? videoThumbnailCheckbox.checked : audioThumbnailCheckbox.checked;
    const normalizeAudio = selectedType === "video" ? videoNormalizeAudioCheckbox.checked : audioNormalizeAudioCheckbox.checked;

    // Resolve Whisper flag
    let isWhisper = false;
    if (forceWhisper !== null) {
      isWhisper = forceWhisper;
    } else {
      isWhisper = (selectedType === "video") ? whisperSubtitlesCheckbox.checked : audioWhisperCheckbox.checked;
    }

    const embedSubtitles = (selectedType === "video" && !isWhisper) ? videoSubtitlesCheckbox.checked : false;
    const burnSubtitles = (whisperBurnModeSelect.value === "burn");

    // Save current choices to storage
    chrome.storage.sync.set({
      savedType: selectedType,
      savedVideoQuality: videoQuality,
      savedVideoFormat: videoFormat,
      savedAudioFormat: audioFormat,
      savedCustomArgs: customArgs,
      savedVideoNormalizeAudio: videoNormalizeAudioCheckbox.checked,
      savedAudioNormalizeAudio: audioNormalizeAudioCheckbox.checked,
      savedVideoThumbnail: videoThumbnailCheckbox.checked,
      savedVideoSubtitles: videoSubtitlesCheckbox.checked,
      savedWhisperSubtitles: whisperSubtitlesCheckbox.checked,
      savedWhisperBurnMode: whisperBurnModeSelect.value,
      savedAudioThumbnail: audioThumbnailCheckbox.checked,
      savedAudioWhisper: audioWhisperCheckbox.checked
    });

    setLoading(true, isWhisper);

    // Fetch saved cookie & folder settings
    chrome.storage.sync.get(["savedCookiesFromBrowser", "savedCookiesFile", "savedDownloadDir", "savedWhisperModel"], (settings) => {
      const payload = {
        action: "download",
        url: url,
        title: currentTabTitle || url,
        download_type: selectedType,
        video_quality: videoQuality,
        video_format: videoFormat,
        audio_format: audioFormat,
        normalize_audio: normalizeAudio,
        embed_thumbnail: embedThumbnail,
        embed_metadata: true,
        embed_subtitles: embedSubtitles,
        whisper_enabled: isWhisper,
        whisper_burn_subtitles: burnSubtitles,
        whisper_model: settings.savedWhisperModel || "large-v3-turbo",
        custom_args: customArgs,
        cookies_from_browser: settings.savedCookiesFromBrowser || "auto",
        cookies_file: settings.savedCookiesFile || "",
        download_dir: settings.savedDownloadDir || ""
      };

      chrome.runtime.sendMessage(payload, (response) => {
        setLoading(false);
        if (response && response.status === "success") {
          switchTab("tasks");
          renderTasks();
        } else {
          const errMsg = (response && response.error) ? response.error : "ネイティブホストとの通信に失敗しました。";
          showStatus(errMsg, "error");
        }
      });
    });
  }

  // Button 1: Normal Download
  downloadBtn.addEventListener("click", () => {
    triggerDownload(whisperSubtitlesCheckbox.checked);
  });

  // Button 2: Dedicated Whisper Button
  downloadWhisperBtn.addEventListener("click", () => {
    triggerDownload(true);
  });

  // Open output folder in Files app / explorer
  const openFolderBtn = document.getElementById("openFolderBtn");
  if (openFolderBtn) {
    openFolderBtn.addEventListener("click", () => {
      const origText = openFolderBtn.textContent;
      openFolderBtn.textContent = "開いています...";
      chrome.runtime.sendMessage({ action: "open_output_dir" }, (response) => {
        setTimeout(() => {
          openFolderBtn.textContent = origText;
        }, 1000);
        if (response && response.status === "error") {
          showStatus(response.error || "フォルダを開けませんでした。", "error");
        }
      });
    });
  }


  // Clear finished tasks
  clearFinishedBtn.addEventListener("click", () => {
    chrome.runtime.sendMessage({ action: "clear_finished" }, () => {
      renderTasks();
    });
  });


  // Status label translation helper
  function getStatusLabel(status) {
    switch (status) {
      case "queued": return "待機中";
      case "downloading": return "ダウンロード中";
      case "transcribing": return "文字起こし中";
      case "converting": return "変換・タグ埋込中";
      case "completed": return "完了";
      case "error": return "エラー";
      case "cancelled": return "キャンセル済";
      default: return status;
    }
  }

  // Task Rendering
  function renderTasks() {
    chrome.runtime.sendMessage({ action: "get_tasks" }, (response) => {
      if (!response || response.status !== "success" || !Array.isArray(response.tasks)) {
        return;
      }

      const tasks = response.tasks;
      const activeTasks = tasks.filter(t => t.status === "downloading" || t.status === "queued" || t.status === "transcribing" || t.status === "converting");

      if (activeTasks.length > 0) {
        activeTasksBadge.textContent = activeTasks.length;
        activeTasksBadge.classList.remove("hidden");
      } else {
        activeTasksBadge.classList.add("hidden");
      }

      tasksCountLabel.textContent = `${tasks.length} 件のタスク`;

      if (tasks.length === 0) {
        emptyTasksState.classList.remove("hidden");
        const existingCards = tasksList.querySelectorAll(".task-card");
        existingCards.forEach(c => c.remove());
        return;
      }

      emptyTasksState.classList.add("hidden");

      const currentIds = new Set(tasks.map(t => t.id));
      const existingCards = tasksList.querySelectorAll(".task-card");
      existingCards.forEach(card => {
        if (!currentIds.has(card.dataset.id)) {
          card.remove();
        }
      });

      tasks.forEach(task => {
        let card = tasksList.querySelector(`.task-card[data-id="${task.id}"]`);
        if (!card) {
          card = document.createElement("div");
          card.className = "task-card";
          card.dataset.id = task.id;
          tasksList.appendChild(card);
        }

        const isRunning = task.status === "downloading" || task.status === "queued" || task.status === "transcribing" || task.status === "converting";
        const percent = Math.min(100, Math.max(0, task.percent || 0));

        let progressClass = "progress-fill";
        if (task.status === "transcribing") progressClass += " transcribing";
        if (task.status === "converting") progressClass += " converting";
        if (task.status === "completed") progressClass += " completed";
        if (task.status === "error") progressClass += " error";

        let errorHtml = "";
        if (task.error) {
          const isAgeOrCookieError = /confirm your age|cookies-from-browser|--cookies|Sign in/i.test(task.error);
          errorHtml = `
            <div class="task-error-msg">
              ${escapeHtml(task.error)}
              ${isAgeOrCookieError ? `<div class="task-error-hint">※年齢確認またはログインが必要です。右上の設定アイコンから「ブラウザのCookie連携」を有効にしてください。</div>` : ""}
            </div>
          `;
        }

        card.innerHTML = `
          <div class="task-card-header">
            <div class="task-title" title="${escapeHtml(task.title || task.url)}">
              ${escapeHtml(task.title || task.url)}
            </div>
            <span class="task-badge ${task.status}">${getStatusLabel(task.status)}</span>
          </div>

          <div class="progress-container">
            <div class="${progressClass}" style="width: ${percent}%;"></div>
          </div>

          <div class="task-meta">
            <div class="task-stats">
              <span><strong>${percent.toFixed(1)}%</strong></span>
              ${task.total_size && task.total_size !== "Unknown" ? `<span>(${escapeHtml(task.total_size)})</span>` : ""}
              ${task.speed ? `<span class="task-speed">${escapeHtml(task.speed)}</span>` : ""}
              ${task.eta && task.eta !== "--:--" ? `<span>残り: ${escapeHtml(task.eta)}</span>` : ""}
            </div>
            ${isRunning ? `<button class="task-cancel-btn" data-action="cancel" data-id="${task.id}">キャンセル</button>` : ""}
          </div>

          ${errorHtml}
        `;

        const cancelBtn = card.querySelector('[data-action="cancel"]');
        if (cancelBtn) {
          cancelBtn.addEventListener("click", () => {
            chrome.runtime.sendMessage({ action: "cancel_task", task_id: task.id }, () => {
              renderTasks();
            });
          });
        }
      });
    });
  }

  function startPolling() {
    renderTasks();
    if (!pollInterval) {
      pollInterval = setInterval(renderTasks, 700);
    }
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  startPolling();

  // Switch to tasks tab if tasks are currently running
  chrome.runtime.sendMessage({ action: "get_tasks" }, (response) => {
    if (response && response.status === "success" && Array.isArray(response.tasks)) {
      const active = response.tasks.filter(t => t.status === "downloading" || t.status === "queued" || t.status === "transcribing" || t.status === "converting");
      if (active.length > 0) {
        switchTab("tasks");
      }
    }
  });

  window.addEventListener("unload", () => {
    stopPolling();
  });

  function setLoading(isLoading, isWhisper = false) {
    downloadBtn.disabled = isLoading;
    downloadWhisperBtn.disabled = isLoading;
    if (isLoading) {
      if (isWhisper) {
        whisperBtnText.textContent = "開始中...";
        whisperBtnSpinner.classList.remove("hidden");
      } else {
        btnText.textContent = "開始中...";
        btnSpinner.classList.remove("hidden");
      }
    } else {
      btnText.textContent = "通常ダウンロード";
      btnSpinner.classList.add("hidden");
      whisperBtnText.textContent = "Whisper文字起こし";
      whisperBtnSpinner.classList.add("hidden");
    }
  }

  function showStatus(message, type) {
    statusBanner.className = `status-banner ${type}`;
    statusText.textContent = message;
    statusBanner.classList.remove("hidden");
  }

  function hideStatus() {
    statusBanner.classList.add("hidden");
  }

  function escapeHtml(str) {
    if (!str) return "";
    return String(str)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#039;");
  }
});
