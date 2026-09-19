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
    id: "ytdlp_video_whisper",
    title: "Download Video + Whisper AI Subtitles",
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

// Cookie helper to export browser cookies in Netscape format
async function getNetscapeCookies(targetUrl) {
  try {
    if (!chrome.cookies) return null;
    let urlObj;
    try {
      urlObj = new URL(targetUrl);
    } catch (e) {
      return null;
    }

    const domain = urlObj.hostname;
    let domainsToQuery = [domain];
    if (domain.includes("youtube.com") || domain.includes("youtu.be")) {
      domainsToQuery = [".youtube.com", "youtube.com", ".google.com", "google.com"];
    }

    let allCookies = [];
    for (const d of domainsToQuery) {
      try {
        const cookies = await chrome.cookies.getAll({ domain: d });
        if (Array.isArray(cookies)) {
          allCookies = allCookies.concat(cookies);
        }
      } catch (e) {}
    }

    if (allCookies.length === 0) {
      try {
        const cookies = await chrome.cookies.getAll({ url: targetUrl });
        if (Array.isArray(cookies)) {
          allCookies = allCookies.concat(cookies);
        }
      } catch (e) {}
    }

    if (allCookies.length === 0) return null;

    const seen = new Set();
    const lines = [
      "# Netscape HTTP Cookie File",
      "# http://curl.haxx.se/rfc/cookie_spec.html",
      "# This is a generated file!  Do not edit.",
      ""
    ];

    for (const c of allCookies) {
      const key = `${c.domain}|${c.name}|${c.path}`;
      if (seen.has(key)) continue;
      seen.add(key);

      const d = c.domain || "";
      const includeSubdomains = d.startsWith(".") ? "TRUE" : "FALSE";
      const path = c.path || "/";
      const secure = c.secure ? "TRUE" : "FALSE";
      const expires = c.expirationDate ? Math.round(c.expirationDate) : 0;
      const name = c.name || "";
      const value = c.value || "";

      lines.push(`${d}\t${includeSubdomains}\t${path}\t${secure}\t${expires}\t${name}\t${value}`);
    }

    return lines.join("\n");
  } catch (err) {
    return null;
  }
}

// Handle Context Menu Clicks
chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  const targetUrl = info.linkUrl || info.pageUrl || (tab && tab.url);
  if (!targetUrl) return;

  let downloadType = "video";
  let videoQuality = "best";
  let audioFormat = "mp3";

  let whisperEnabled = false;

  if (info.menuItemId === "ytdlp_video_best") {
    downloadType = "video";
    videoQuality = "best";
  } else if (info.menuItemId === "ytdlp_video_whisper") {
    downloadType = "video";
    videoQuality = "best";
    whisperEnabled = true;
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

  chrome.storage.sync.get([
    "savedEmbedThumbnail",
    "savedEmbedMetadata",
    "savedEmbedSubtitles",
    "savedWhisperEnabled",
    "savedWhisperModel",
    "savedWhisperBurnMode",
    "savedCookiesFromBrowser",
    "savedCookiesFile",
    "savedDownloadDir"
  ], async (items) => {
    let cookiesData = "";
    const cookiesFromBrowser = items.savedCookiesFromBrowser || "auto";

    if (cookiesFromBrowser === "auto" || !cookiesFromBrowser) {
      cookiesData = await getNetscapeCookies(targetUrl) || "";
    }

    const finalWhisper = whisperEnabled || (items.savedWhisperEnabled === true);
    const burnSubtitles = (items.savedWhisperBurnMode === "burn");

    sendToNativeHost({
      action: "download",
      url: targetUrl,
      title: (tab && tab.title) ? tab.title : targetUrl,
      download_type: downloadType,
      video_quality: videoQuality,
      video_format: "mp4",
      audio_format: audioFormat,
      embed_thumbnail: items.savedEmbedThumbnail !== undefined ? items.savedEmbedThumbnail : true,
      embed_metadata: items.savedEmbedMetadata !== undefined ? items.savedEmbedMetadata : true,
      embed_subtitles: finalWhisper ? false : (items.savedEmbedSubtitles !== undefined ? items.savedEmbedSubtitles : false),
      whisper_enabled: finalWhisper,
      whisper_burn_subtitles: burnSubtitles,
      whisper_model: items.savedWhisperModel || "large-v3-turbo",
      cookies_from_browser: cookiesData ? "" : (cookiesFromBrowser === "auto" ? "" : cookiesFromBrowser),
      cookies_file: items.savedCookiesFile || "",
      cookies_data: cookiesData,
      download_dir: items.savedDownloadDir || ""
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
});

// Handle Messages from Popup / Options
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  (async () => {
    try {
      if (message.action === "download") {
        if (!message.cookies_data && (!message.cookies_from_browser || message.cookies_from_browser === "auto")) {
          const cookieText = await getNetscapeCookies(message.url);
          if (cookieText) {
            message.cookies_data = cookieText;
            message.cookies_from_browser = "";
          }
        }
      }

      sendToNativeHost(message, (response) => {
        if (message.action === "download" || message.action === "cancel_task") {
          pollTasksStatus();
        }
        sendResponse(response);
      });
    } catch (e) {
      sendResponse({ status: "error", error: e.toString() });
    }
  })();
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
