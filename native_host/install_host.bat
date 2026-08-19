@echo off
setlocal EnableDelayedExpansion

echo ===================================================
echo  yt-dlp Native Messaging Host Installer
echo ===================================================
echo.

set "SCRIPT_DIR=%~dp0"
set "MANIFEST_PATH=%SCRIPT_DIR%com.ytdlp.downloader.json"
set "LAUNCHER_PATH=%SCRIPT_DIR%host_launcher.bat"

:: Escape backslashes for JSON
set "ESCAPED_LAUNCHER_PATH=%LAUNCHER_PATH:\=\\%"

:: Generate com.ytdlp.downloader.json with the current absolute path
echo Generating host manifest...
(
    echo {
    echo   "name": "com.ytdlp.downloader",
    echo   "description": "yt-dlp Native Messaging Host",
    echo   "path": "!ESCAPED_LAUNCHER_PATH!",
    echo   "type": "stdio",
    echo   "allowed_origins": [
    echo     "chrome-extension://mkhadenblpnfkappanocjmlpdlgfbfdi/"
    echo   ]
    echo }
) > "%MANIFEST_PATH%"

echo Manifest generated at: %MANIFEST_PATH%
echo.

:: Register to Google Chrome
echo Registering to Google Chrome...
reg add "HKCU\Software\Google\Chrome\NativeMessagingHosts\com.ytdlp.downloader" /ve /t REG_SZ /d "%MANIFEST_PATH%" /f >nul
if !errorlevel! equ 0 (
    echo [OK] Registered to Chrome.
) else (
    echo [ERROR] Failed to register to Chrome.
)

:: Register to Microsoft Edge
echo Registering to Microsoft Edge...
reg add "HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.ytdlp.downloader" /ve /t REG_SZ /d "%MANIFEST_PATH%" /f >nul
if !errorlevel! equ 0 (
    echo [OK] Registered to Edge.
) else (
    echo [ERROR] Failed to register to Edge.
)

echo.
echo ===================================================
echo  Installation completed!
echo ===================================================
pause
endlocal
