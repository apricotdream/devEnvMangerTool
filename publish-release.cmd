@echo off
setlocal EnableDelayedExpansion
cd /d "%~dp0"

title DevEnvManager Release Publisher

REM =====================================================
REM   Config
REM =====================================================
set "REPO=apricotdream/devEnvMangerTool"
set "GITEE_API=https://gitee.com/api/v5"
set "SRC_TAURI=%~dp0src-tauri"
set "OUT=%~dp0dist"
set "SOURCE_BRANCH=2026-08"
set "TARGET_BRANCH=main"
set "ARCH=x64"
set "RJSON=%~dp0_tmp_release.json"
set "UJSON=%~dp0_tmp_up.json"

REM =====================================================
REM   Version from Cargo.toml (single source)
REM =====================================================
set "VERSION="
for /f "tokens=2 delims==" %%a in ('findstr /b /c:"version " "%SRC_TAURI%\Cargo.toml"') do set "VERSION=%%~a"
set "VERSION=!VERSION: =!"
set "VERSION=!VERSION:"=!"
if not defined VERSION (
    echo [ERROR] Cannot read version from Cargo.toml
    exit /b 1
)
set "TAG=v!VERSION!"
set "BODY_TEXT=DevEnvManager v!VERSION! (auto release)"

echo ============================================
echo   DevEnvManager Release publish v!VERSION!
echo   repo : !REPO!
echo   tag  : !TAG!
echo   arch : !ARCH!
echo ============================================

REM =====================================================
REM   Preflight
REM =====================================================
if not defined GITEE_TOKEN (
    echo [ERROR] GITEE_TOKEN env var not set.
    echo         Run: set GITEE_TOKEN=your_token
    exit /b 1
)
where curl >nul 2>&1
if errorlevel 1 (
    echo [ERROR] curl.exe not found.
    exit /b 1
)

REM -----------------------------------------------------
REM Conflict check: Gitee release
REM -----------------------------------------------------
echo [*] Checking Gitee for existing release !TAG! ...
set "EXISTS="
for /f "delims=" %%j in ('curl -s "%GITEE_API%/repos/%REPO%/releases/tags/!TAG!?access_token=!GITEE_TOKEN!"') do set "EXISTS=!EXISTS!%%j"
echo !EXISTS! | findstr /i "tag_name" >nul
if not errorlevel 1 (
    echo [ERROR] Release !TAG! already exists on Gitee.
    exit /b 1
)
echo [OK] No existing release on Gitee.

REM -----------------------------------------------------
REM Conflict check: git tag
REM -----------------------------------------------------
git fetch origin >nul 2>&1
git ls-remote --tags origin !TAG! 2>nul | findstr /i "refs/tags" >nul
if not errorlevel 1 (
    echo [ERROR] Git tag !TAG! already exists on origin.
    exit /b 1
)
echo [OK] No existing git tag.

REM =====================================================
REM   1. Clean and build
REM =====================================================
echo [1/6] Cleaning dist\ ...
if exist "%OUT%" rmdir /s /q "%OUT%"
mkdir "%OUT%"

echo [2/6] cargo build --release ...
pushd "%SRC_TAURI%"
call cargo build --release
if errorlevel 1 (
    popd
    echo [ERROR] cargo build failed.
    exit /b 1
)
popd
echo [OK] build done.

set "TAURI_CMD="
"%USERPROFILE%\.cargo\bin\cargo.exe" tauri --version >nul 2>&1
if not errorlevel 1 (
    set "TAURI_CMD=cargo tauri build"
) else (
    npx --no-install @tauri-apps/cli --version >nul 2>&1
    if not errorlevel 1 (
        set "TAURI_CMD=npx @tauri-apps/cli build"
    ) else (
        echo [ERROR] tauri-cli not installed.
        echo         cargo install tauri-cli --locked
        echo         npm install -g @tauri-apps/cli
        exit /b 1
    )
)

echo [3/6] tauri build ^(NSIS^) : !TAURI_CMD!
pushd "%SRC_TAURI%"
call !TAURI_CMD!
if errorlevel 1 (
    popd
    echo [ERROR] tauri build failed.
    exit /b 1
)
popd
echo [OK] installer build done.

REM -----------------------------------------------------
REM   3b. Build the CLI binary (devenv.exe) from the workspace core.
REM   Workspace split: the devenv bin lives in devenv-core, NOT in src-tauri,
REM   so building src-tauri alone no longer emits target\release\devenv.exe.
REM   Run from the workspace root -> lands in %~dp0target\release\devenv.exe.
REM -----------------------------------------------------
echo [3b/6] cargo build CLI devenv.exe (devenv-core) ...
cargo build -p devenv-core --bin devenv --release
if errorlevel 1 (
    echo [ERROR] CLI build failed.
    exit /b 1
)
echo [OK] CLI build done.

REM -----------------------------------------------------
REM   4. Collect artifacts into dist\
REM -----------------------------------------------------
echo [4/6] Collecting artifacts ...
set "SETUP_EXE="
for %%f in ("%~dp0target\release\bundle\nsis\*setup.exe") do set "SETUP_EXE=%%f"
if not defined SETUP_EXE (
    echo [ERROR] NSIS installer not found.
    exit /b 1
)
copy /y "%SETUP_EXE%" "%OUT%\" >nul
echo [OK] Installer : %SETUP_EXE%

set "CLI_ZIP=%OUT%\devenv-cli-%VERSION%-windows-%ARCH%.zip"
powershell -NoProfile -Command "Compress-Archive -Path '%~dp0target\release\devenv.exe' -DestinationPath '%CLI_ZIP%' -Force"
if errorlevel 1 (
    echo [ERROR] CLI packaging failed.
    exit /b 1
)
echo [OK] CLI zip : %CLI_ZIP%

set "SHA256SUM=%OUT%\SHA256SUMS.txt"
if exist "%SHA256SUM%" del "%SHA256SUM%"
for %%f in ("%OUT%\*") do (
    for /f "delims=" %%h in ('certutil -hashfile "%%f" SHA256 ^| findstr /r /c:"^[0-9a-fA-F][0-9a-fA-F]*$"') do echo %%h  %%~nxf >> "%SHA256SUM%"
)
echo [OK] SHA256SUMS.txt generated.

REM -----------------------------------------------------
REM   5. Git: tag the current tip of origin/main
REM   (origin/main is already at the release content; do NOT move it)
REM -----------------------------------------------------
echo [5/6] Creating git tag !TAG! at origin/%TARGET_BRANCH% ...
git fetch origin >nul 2>&1
git tag !TAG! origin/%TARGET_BRANCH% 2>nul
if errorlevel 1 (
    echo [ERROR] Local tag !TAG! exists.
    exit /b 1
)
git push origin !TAG!
if errorlevel 1 (
    echo [ERROR] Pushing git tag failed.
    exit /b 1
)
echo [OK] Pushed git tag !TAG! at origin/%TARGET_BRANCH%.

REM =====================================================
REM   6. Gitee: create release and upload artifacts
REM =====================================================
echo [6/6] Creating Gitee release !TAG! ...
del "%RJSON%" >nul 2>&1
curl -s -f -X POST "%GITEE_API%/repos/%REPO%/releases" -F "access_token=!GITEE_TOKEN!" -F "tag_name=!TAG!" -F "target_commitish=!TARGET_BRANCH!" -F "name=!TAG!" -F "body=!BODY_TEXT!" -F "prerelease=true" -o "%RJSON%"
if errorlevel 1 (
    echo [ERROR] Create release failed. Response:
    type "%RJSON%" 2>nul
    del "%RJSON%" >nul 2>&1
    exit /b 1
)

set "RELEASE_ID="
for /f "usebackq delims=" %%i in (`powershell -NoProfile -Command "$r=Get-Content -Raw '%~dp0_tmp_release.json' | ConvertFrom-Json; $r.id"`) do set "RELEASE_ID=%%i"
if not defined RELEASE_ID (
    echo [ERROR] Could not parse release id. Response:
    type "%RJSON%" 2>nul
    del "%RJSON%" >nul 2>&1
    exit /b 1
)
del "%RJSON%" >nul 2>&1
echo [OK] Created release !TAG! id=!RELEASE_ID!

echo [*] Uploading artifacts ...
for %%f in ("%OUT%\*") do (
    echo      Upload %%~nxf ...
    del "%UJSON%" >nul 2>&1
    curl -s -f -X POST "%GITEE_API%/repos/%REPO%/releases/!RELEASE_ID!/attach_files" -F "access_token=!GITEE_TOKEN!" -F "file=@%%~ff" -o "%UJSON%"
    if errorlevel 1 (
        echo     [WARN] %%~nxf upload may have failed:
        type "%UJSON%" 2>nul
    ) else (
        echo     [OK] %%~nxf uploaded
    )
)
del "%UJSON%" >nul 2>&1

echo.
echo ============================================================
echo   Done. Verify release attachments on Gitee (incl SHA256SUMS.txt).
echo ============================================================
echo   dist: %OUT%
echo   tag : !TAG!
echo ============================================================
endlocal
