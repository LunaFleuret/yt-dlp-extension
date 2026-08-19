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
  const videoThumbnailCheckbox = document.getElementById("videoThumbnail");
  const videoSubtitlesCheckbox = document.getElementById("videoSubtitles");

  const audioFormatSelect = document.getElementById("audioFormat");
  const audioThumbnailCheckbox = document.getElementById("audioThumbnail");

  const customArgsInput = document.getElementById("customArgs");

  const downloadBtn = document.getElementById("downloadBtn");
  const btnText = downloadBtn.querySelector(".btn-text");
  const btnSpinner = document.getElementById("btnSpinner");
  const statusBanner = document.getElementById("statusBanner");
  const statusText = document.getElementById("statusText");

  const tasksList = document.getElementById("tasksList");
  const emptyTasksState = document.getElementById("emptyTasksState");
  const tasksCountLabel = document.getElementById("tasksCountLabel");
  const clearFinishedBtn = document.getElementById("clearFinishedBtn");

  let pollInterval = null;
  let currentTabTitle = "";

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
    "savedVideoThumbnail",
    "savedVideoSubtitles",
    "savedAudioThumbnail"
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
    if (items.savedVideoThumbnail !== undefined) videoThumbnailCheckbox.checked = items.savedVideoThumbnail;
    if (items.savedVideoSubtitles !== undefined) videoSubtitlesCheckbox.checked = items.savedVideoSubtitles;
    if (items.savedAudioThumbnail !== undefined) audioThumbnailCheckbox.checked = items.savedAudioThumbnail;
  });

  // Fetch active tab
  try {
    const [activeTab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (activeTab) {
      currentTabTitle = activeTab.title || "";
      pageTitleEl.textContent = activeTab.title || "タイトルなし";
      pageUrlInput.value = activeTab.url || "";
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

  // Download Handler
  downloadBtn.addEventListener("click", async () => {
    const url = pageUrlInput.value.trim();
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
    const embedSubtitles = selectedType === "video" ? videoSubtitlesCheckbox.checked : false;

    // Save current choices to storage
    chrome.storage.sync.set({
      savedType: selectedType,
      savedVideoQuality: videoQuality,
      savedVideoFormat: videoFormat,
      savedAudioFormat: audioFormat,
      savedCustomArgs: customArgs,
      savedVideoThumbnail: videoThumbnailCheckbox.checked,
      savedVideoSubtitles: videoSubtitlesCheckbox.checked,
      savedAudioThumbnail: audioThumbnailCheckbox.checked
    });

    setLoading(true);
    hideStatus();

    const payload = {
      action: "download",
      url: url,
      title: currentTabTitle || url,
      download_type: selectedType,
      video_quality: videoQuality,
      video_format: videoFormat,
      audio_format: audioFormat,
      embed_thumbnail: embedThumbnail,
      embed_metadata: true,
      embed_subtitles: embedSubtitles,
      custom_args: customArgs
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

  // Open output folder in explorer
  const openFolderBtn = document.getElementById("openFolderBtn");
  if (openFolderBtn) {
    openFolderBtn.addEventListener("click", () => {
      chrome.runtime.sendMessage({ action: "open_output_dir" });
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
      const activeTasks = tasks.filter(t => t.status === "downloading" || t.status === "queued" || t.status === "converting");

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

        const isRunning = task.status === "downloading" || task.status === "queued" || task.status === "converting";
        const percent = Math.min(100, Math.max(0, task.percent || 0));

        let progressClass = "progress-fill";
        if (task.status === "converting") progressClass += " converting";
        if (task.status === "completed") progressClass += " completed";
        if (task.status === "error") progressClass += " error";

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

          ${task.error ? `<div class="task-error-msg">${escapeHtml(task.error)}</div>` : ""}
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
      const active = response.tasks.filter(t => t.status === "downloading" || t.status === "queued" || t.status === "converting");
      if (active.length > 0) {
        switchTab("tasks");
      }
    }
  });

  window.addEventListener("unload", () => {
    stopPolling();
  });

  function setLoading(isLoading) {
    downloadBtn.disabled = isLoading;
    if (isLoading) {
      btnText.textContent = "開始中...";
      btnSpinner.classList.remove("hidden");
    } else {
      btnText.textContent = "ダウンロード開始";
      btnSpinner.classList.add("hidden");
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
