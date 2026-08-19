document.addEventListener("DOMContentLoaded", () => {
  const defaultTypeSelect = document.getElementById("defaultType");
  const defaultVideoQualitySelect = document.getElementById("defaultVideoQuality");
  const defaultAudioFormatSelect = document.getElementById("defaultAudioFormat");
  const downloadDirInput = document.getElementById("downloadDir");
  const embedThumbnailCheckbox = document.getElementById("embedThumbnail");
  const embedMetadataCheckbox = document.getElementById("embedMetadata");
  const embedSubtitlesCheckbox = document.getElementById("embedSubtitles");

  const testConnBtn = document.getElementById("testConnBtn");
  const connResult = document.getElementById("connResult");
  const saveBtn = document.getElementById("saveBtn");
  const saveStatus = document.getElementById("saveStatus");

  // Load settings
  chrome.storage.sync.get([
    "savedType",
    "savedVideoQuality",
    "savedAudioFormat",
    "savedDownloadDir",
    "savedEmbedThumbnail",
    "savedEmbedMetadata",
    "savedEmbedSubtitles"
  ], (items) => {
    if (items.savedType) defaultTypeSelect.value = items.savedType;
    if (items.savedVideoQuality) defaultVideoQualitySelect.value = items.savedVideoQuality;
    if (items.savedAudioFormat) defaultAudioFormatSelect.value = items.savedAudioFormat;
    if (items.savedDownloadDir !== undefined) downloadDirInput.value = items.savedDownloadDir;
    if (items.savedEmbedThumbnail !== undefined) embedThumbnailCheckbox.checked = items.savedEmbedThumbnail;
    if (items.savedEmbedMetadata !== undefined) embedMetadataCheckbox.checked = items.savedEmbedMetadata;
    if (items.savedEmbedSubtitles !== undefined) embedSubtitlesCheckbox.checked = items.savedEmbedSubtitles;
  });

  // Test Host Connection
  testConnBtn.addEventListener("click", () => {
    connResult.className = "conn-result";
    connResult.textContent = "接続確認中...";

    chrome.runtime.sendMessage({ action: "ping" }, (response) => {
      if (response && response.status === "pong") {
        connResult.className = "conn-result success";
        connResult.textContent = "[OK] ネイティブホストとの通信に成功しました。";
      } else {
        connResult.className = "conn-result error";
        const err = (response && response.error) ? response.error : "ホストとの通信に失敗しました。";
        connResult.textContent = `[Error] ${err}`;
      }
    });
  });

  // Save Settings
  saveBtn.addEventListener("click", () => {
    const configData = {
      savedType: defaultTypeSelect.value,
      savedVideoQuality: defaultVideoQualitySelect.value,
      savedAudioFormat: defaultAudioFormatSelect.value,
      savedDownloadDir: downloadDirInput.value.trim(),
      savedEmbedThumbnail: embedThumbnailCheckbox.checked,
      savedEmbedMetadata: embedMetadataCheckbox.checked,
      savedEmbedSubtitles: embedSubtitlesCheckbox.checked
    };

    chrome.storage.sync.set(configData, () => {
      chrome.runtime.sendMessage({
        action: "update_config",
        config: {
          download_dir: configData.savedDownloadDir,
          embed_thumbnail: configData.savedEmbedThumbnail,
          embed_metadata: configData.savedEmbedMetadata,
          embed_subtitles: configData.savedEmbedSubtitles
        }
      }, () => {
        saveStatus.textContent = "設定を保存しました。";
        setTimeout(() => {
          saveStatus.textContent = "";
        }, 3000);
      });
    });
  });
});
