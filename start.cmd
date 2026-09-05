@echo off
setlocal
chcp 65001 >nul
cd /d "%~dp0"

title EnvManager 启动器

echo ============================================
echo   DevEnvManager - 开发环境管理工具
echo ============================================
echo.

REM ---------- 1. 检查 Rust 工具链 ----------
set "CARGO_BIN=%USERPROFILE%\.cargo\bin"
if not exist "%CARGO_BIN%\cargo.exe" (
    echo [错误] 未检测到 Rust 工具链 ^(cargo^)。
    echo        请先以管理员身份运行 setup-toolchain.cmd 安装。
    pause
    exit /b 1
)
set "PATH=%CARGO_BIN%;%PATH%"

REM ---------- 2. 关闭已运行的实例，避免多开 ----------
taskkill /IM dev-env-manager.exe /F >nul 2>&1

REM ---------- 3. 编译（增量，约 1-10 秒） ----------
echo [1/2] 编译应用...
cd /d "%~dp0src-tauri"
cargo build >nul 2>&1
if errorlevel 1 (
    echo [错误] 编译失败，请查看下方错误信息：
    cargo build
    echo.
    pause
    exit /b 1
)
echo [OK] 编译完成

REM ---------- 4. 启动 ----------
echo [2/2] 启动 DevEnvManager...
start "" "%~dp0target\debug\dev-env-manager.exe"
echo.
echo 应用已启动。如需开发调试，可在 src-tauri 目录运行：cargo tauri dev
timeout /t 2 >nul
exit /b 0
