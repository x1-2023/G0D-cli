@echo off
REM Prefer stable install path; fall back to versioned binaries if locked during upgrade.
if exist "%LOCALAPPDATA%\Programs\g0d\g0d.exe" (
  "%LOCALAPPDATA%\Programs\g0d\g0d.exe" %*
) else (
  for %%F in ("%LOCALAPPDATA%\Programs\g0d\g0d-*.exe") do (
    "%%~fF" %*
    exit /b %ERRORLEVEL%
  )
  echo g0d is not installed. Run install.ps1 or install-remote.ps1
  exit /b 1
)
