@echo off
if exist "%LOCALAPPDATA%\Programs\g0d\g0d-1.9.2.exe" (
  "%LOCALAPPDATA%\Programs\g0d\g0d-1.9.2.exe" %*
) else if exist "%LOCALAPPDATA%\Programs\g0d\g0d-1.9.1.exe" (
  "%LOCALAPPDATA%\Programs\g0d\g0d-1.9.1.exe" %*
) else (
  "%LOCALAPPDATA%\Programs\g0d\g0d.exe" %*
)
