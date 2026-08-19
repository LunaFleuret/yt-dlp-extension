const HOST_NAME = "com.ytdlp.downloader";

// Store notified tasks to prevent duplicate notifications
const notifiedTaskStates = new Map();

// Initialize Context Menus
chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: "ytdlp_parent",
    title: "Download with yt-dlp",
    contexts: ["page", "link"]
  });

  chrome.contextMenus.create({
    parentId: "ytdlp_parent",
    id: "ytdlp_video_best",
    title: "Download Video (Best Quality MP4)",
    contexts: ["page", "link"]
  });

  chrome.contextMenus.create({
    parentId: "ytdlp_parent",
    id: "ytdlp_video_1080",
    title: "Download Video (1080p MP4)",
    contexts: ["page", "link"]
  });

  chrome.contextMenus.create({
    parentId: "ytdlp_parent",
    id: "ytdlp_audio_mp3",
    title: "Download Audio (MP3 320k + Cover)",
    contexts: ["page", "link"]
  });

  chrome.contextMenus.create({
    parentId: "ytdlp_parent",
    id: "ytdlp_audio_flac",
    title: "Download Audio (FLAC Lossless)",
    contexts: ["page", "link"]
  });

  // Setup periodic task checking alarm
  chrome.alarms.create("check_ytdlp_tasks", { periodInMinutes: 0.1 });
});

// Periodic Task Polling for notifications and badge updates
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === "check_ytdlp_tasks") {
    pollTasksStatus();
  }
});

function pollTasksStatus() {
  sendToNativeHost({ action: "get_tasks" }, (response) => {
    if (response && response.status === "success" && Array.isArray(response.tasks)) {
      const tasks = response.tasks;
      const activeCount = tasks.filter(t => t.status === "downloading" || t.status === "queued" || t.status === "converting").length;

      // Update badge
      if (activeCount > 0) {
        chrome.action.setBadgeText({ text: String(activeCount) });
        chrome.action.setBadgeBackgroundColor({ color: "#4e6ef2" });
      } else {
        chrome.action.setBadgeText({ text: "" });
      }

      // Check task status transitions for notifications
      tasks.forEach(task => {
        const lastStatus = notifiedTaskStates.get(task.id);
        if (lastStatus !== task.status) {
          if (task.status === "completed") {
            showNotification("Download Complete", `${task.title || task.url}`);
            notifiedTaskStates.set(task.id, "completed");
          } else if (task.status === "error") {
            showNotification("Download Failed", `${task.title || task.url}\n${task.error || "Unknown error"}`, true);
            notifiedTaskStates.set(task.id, "error");
          }
        }
      });
    }
  });
}

// Handle Context Menu Clicks
chrome.contextMenus.onClicked.addListener((info, tab) => {
  const targetUrl = info.linkUrl || info.pageUrl || (tab && tab.url);
  if (!targetUrl) return;

  let downloadType = "video";
  let videoQuality = "best";
  let audioFormat = "mp3";

  if (info.menuItemId === "ytdlp_video_best") {
    downloadType = "video";
    videoQuality = "best";
  } else if (info.menuItemId === "ytdlp_video_1080") {
    downloadType = "video";
    videoQuality = "1080";
  } else if (info.menuItemId === "ytdlp_audio_mp3") {
    downloadType = "audio";
    audioFormat = "mp3";
  } else if (info.menuItemId === "ytdlp_audio_flac") {
    downloadType = "audio";
    audioFormat = "flac";
  }

  sendToNativeHost({
    action: "download",
    url: targetUrl,
    title: (tab && tab.title) ? tab.title : targetUrl,
    download_type: downloadType,
    video_quality: videoQuality,
    video_format: "mp4",
    audio_format: audioFormat,
    embed_thumbnail: true,
    embed_metadata: true
  }, (response) => {
    if (response && response.status === "success") {
      showNotification("Download Started", `Downloading:\n${targetUrl}`);
      pollTasksStatus();
    } else {
      const err = response ? response.error : "Failed to connect to native host";
      showNotification("Download Error", `Error: ${err}`, true);
    }
  });
});

// Handle Messages from Popup / Options
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  sendToNativeHost(message, (response) => {
    if (message.action === "download" || message.action === "cancel_task") {
      pollTasksStatus();
    }
    sendResponse(response);
  });
  return true; // Keep message channel open for async response
});

// Native Messaging Communication Helper
function sendToNativeHost(payload, callback) {
  try {
    chrome.runtime.sendNativeMessage(HOST_NAME, payload, (response) => {
      if (chrome.runtime.lastError) {
        callback({
          status: "error",
          error: chrome.runtime.lastError.message + " (Please ensure install_host.bat has been run)"
        });
      } else {
        callback(response || { status: "error", error: "Empty response from host" });
      }
    });
  } catch (err) {
    callback({ status: "error", error: err.toString() });
  }
}

// Notification Helper
function showNotification(title, message, isError = false) {
  chrome.notifications.create({
    type: "basic",
    iconUrl: "icons/icon128.png",
    title: title,
    message: message,
    priority: isError ? 2 : 1
  });
}
