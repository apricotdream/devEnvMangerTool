@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul
title DevEnvManager - 工具链安装（需管理员权限）

echo ============================================================
echo   DevEnvManager Tauri 工具链安装脚本
echo   本脚本将安装: 1) MSVC C++ 构建工具  2) Rust 工具链
echo   约需 5~7GB 磁盘空间，耗时 20~40 分钟
echo ============================================================
echo.

REM ---------- 1) 检查管理员权限 ----------
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [错误] 当前不是管理员权限。
    echo        请右键本文件，选择"以管理员身份运行"。
    pause
    exit /b 1
)
echo [OK] 已确认管理员权限

REM ---------- 2) 下载 VS Build Tools 引导程序 ----------
echo.
echo [1/3] 下载 VS Build Tools 引导程序 (vs_BuildTools.exe)...
powershell -NoProfile -Command "Invoke-WebRequest -Uri 'https://aka.ms/vs/17/release/vs_BuildTools.exe' -OutFile '%TEMP%\vs_BuildTools.exe' -UseBasicParsing"
if not exist "%TEMP%\vs_BuildTools.exe" (
    echo [错误] 下载 vs_BuildTools.exe 失败，请检查网络后重试。
    pause
    exit /b 1
)
echo [OK] 下载完成

REM ---------- 3) 安装 MSVC C++ 构建工具 ----------
echo.
echo [2/3] 安装 MSVC C++ 构建工具（C++ 桌面负载，约 5-7GB，请耐心等待）...
echo      窗口会显示安装进度，完成后自动继续。
"%TEMP%\vs_BuildTools.exe" --passive --wait --norestart ^
    --add Microsoft.VisualStudio.Workload.VCTools ^
    --includeRecommended
if %errorlevel% neq 0 (
    echo [警告] VS Build Tools 安装返回非零退出码（%errorlevel%）。
    echo        可重启后重试，或打开 Visual Studio Installer 手动安装"C++ 桌面开发"负载。
    pause
    exit /b 1
)
echo [OK] MSVC 构建工具安装完成

REM ---------- 4) 安装 Rust 工具链 ----------
echo.
echo [3/3] 下载并安装 Rust 工具链 (rustup)...
set "RUSTUP_DIST_SERVER=https://rsproxy.cn"
set "RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup"
powershell -NoProfile -Command "Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile '%TEMP%\rustup-init.exe' -UseBasicParsing"
if not exist "%TEMP%\rustup-init.exe" (
    echo [错误] 下载 rustup-init.exe 失败。
    pause
    exit /b 1
)
"%TEMP%\rustup-init.exe" -y --default-toolchain stable --profile default
if %errorlevel% neq 0 (
    echo [错误] rustup 安装失败。
    pause
    exit /b 1
)
echo [OK] Rust 工具链安装完成

REM ---------- 5) 验证 ----------
echo.
echo ============================================================
echo   安装完成！请关闭本窗口，重开一个终端验证：
echo.
echo     cargo --version
echo     rustc --version
echo
echo   然后回到 DevEnvManager 目录执行：
echo     cd tauri
echo     cargo tauri dev
echo ============================================================
pause
