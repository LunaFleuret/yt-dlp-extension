@echo off
setlocal

echo ===================================================
echo  yt-dlp Native Messaging Host Uninstaller
echo ===================================================
echo.

echo Removing from Google Chrome registry...
reg delete "HKCU\Software\Google\Chrome\NativeMessagingHosts\com.ytdlp.downloader" /f >nul 2>&1
if %errorlevel% equ 0 (
    echo [OK] Removed from Chrome.
) else (
    echo [INFO] Chrome registry entry not found or already removed.
)

echo Removing from Microsoft Edge registry...
reg delete "HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.ytdlp.downloader" /f >nul 2>&1
if %errorlevel% equ 0 (
    echo [OK] Removed from Edge.
) else (
    echo [INFO] Edge registry entry not found or already removed.
)

echo.
echo ===================================================
echo  Uninstallation completed!
echo ===================================================
pause
endlocal
