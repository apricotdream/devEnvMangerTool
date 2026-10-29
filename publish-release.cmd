@echo off
cd /d "%~dp0"
REM Common release flow; --build-only builds without publishing.
python scripts\release.py --platform windows %*
exit /b %errorlevel%
