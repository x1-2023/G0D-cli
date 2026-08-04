# One-liner install (after a GitHub Release exists):
#   irm https://raw.githubusercontent.com/x1-2023/G0D-cli/main/install-remote.ps1 | iex
#
# Optional:
#   irm ... | iex -Tag v1.13.0
#   $env:G0D_INSTALL_DIR = 'D:\tools\g0d'
#
# This script only replaces the binary under %LOCALAPPDATA%\Programs\g0d.
# Config + sessions live elsewhere and are never deleted:
#   %APPDATA%\g0d\config.toml
#   %APPDATA%\g0d\sessions\
#   %LOCALAPPDATA%\g0d\history.txt

[CmdletBinding()]
param(
    [string]$Repo = 'x1-2023/G0D-cli',
    [string]$Tag,
    [string]$AssetName = 'g0d-windows-x64.exe',
    [string]$InstallDirectory = $env:G0D_INSTALL_DIR
)

$ErrorActionPreference = 'Stop'

if (-not $InstallDirectory) {
    $InstallDirectory = Join-Path $env:LOCALAPPDATA 'Programs\g0d'
}

function Get-Release {
    param([string]$Repository, [string]$ReleaseTag)
    $headers = @{
        'User-Agent' = 'g0d-install-remote'
        'Accept'     = 'application/vnd.github+json'
    }
    if ($env:GITHUB_TOKEN) {
        $headers['Authorization'] = "Bearer $($env:GITHUB_TOKEN)"
    }
    if ($ReleaseTag) {
        $url = "https://api.github.com/repos/$Repository/releases/tags/$ReleaseTag"
    } else {
        $url = "https://api.github.com/repos/$Repository/releases/latest"
    }
    return Invoke-RestMethod -Uri $url -Headers $headers
}

Write-Host "Fetching release metadata for $Repo ..."
try {
    $release = Get-Release -Repository $Repo -ReleaseTag $Tag
} catch {
    throw @"
Could not fetch GitHub release for $Repo.
Create a Release with asset '$AssetName' first, e.g.:
  gh release create v1.13.0 .\cli\target\release\g0d.exe --title v1.13.0
Original error: $_
"@
}

$asset = $release.assets | Where-Object { $_.name -eq $AssetName } | Select-Object -First 1
if (-not $asset) {
    # Fallback: any .exe that looks like g0d
    $asset = $release.assets |
        Where-Object { $_.name -match '^g0d.*\.exe$' } |
        Select-Object -First 1
}
if (-not $asset) {
    $names = @($release.assets | ForEach-Object { $_.name }) -join ', '
    throw "Release '$($release.tag_name)' has no Windows binary asset. Found: $names"
}

$InstalledBinary = Join-Path $InstallDirectory 'g0d.exe'
$TempBinary = Join-Path $env:TEMP ("g0d-install-{0}.exe" -f [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null

Write-Host "Downloading $($asset.name) from $($release.tag_name) ..."
$headers = @{ 'User-Agent' = 'g0d-install-remote' }
if ($env:GITHUB_TOKEN) {
    $headers['Authorization'] = "Bearer $($env:GITHUB_TOKEN)"
}
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $TempBinary -Headers $headers

# If the current binary is locked, install side-by-side then swap via shim.
$target = $InstalledBinary
if (Test-Path -LiteralPath $InstalledBinary) {
    try {
        [System.IO.File]::Open($InstalledBinary, 'Open', 'ReadWrite', 'None').Dispose()
    } catch {
        $target = Join-Path $InstallDirectory ("g0d-{0}.exe" -f ($release.tag_name -replace '[^0-9A-Za-z._-]', ''))
        Write-Host "Existing g0d.exe is in use; installing as $(Split-Path $target -Leaf)"
    }
}
Copy-Item -LiteralPath $TempBinary -Destination $target -Force
Remove-Item -LiteralPath $TempBinary -Force -ErrorAction SilentlyContinue

# Prefer stable name when possible.
if ($target -ne $InstalledBinary -and -not (Test-Path -LiteralPath $InstalledBinary)) {
    Copy-Item -LiteralPath $target -Destination $InstalledBinary -Force
    $target = $InstalledBinary
}

$ShimDirectory = Join-Path $env:APPDATA 'npm'
$ShimPath = Join-Path $ShimDirectory 'g0d.cmd'
New-Item -ItemType Directory -Path $ShimDirectory -Force | Out-Null
@(
    '@echo off'
    " `"$target`" %*"
) | Set-Content -LiteralPath $ShimPath -Encoding ASCII

$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$Entries = @($UserPath -split ';' | Where-Object { $_ })
foreach ($dir in @($InstallDirectory, $ShimDirectory)) {
    $present = $Entries | Where-Object { $_.TrimEnd('\') -ieq $dir.TrimEnd('\') }
    if (-not $present) {
        $Entries += $dir
    }
}
[Environment]::SetEnvironmentVariable('Path', ($Entries -join ';'), 'User')
if (($env:Path -split ';') -notcontains $InstallDirectory) {
    $env:Path = "$InstallDirectory;$env:Path"
}

Write-Host ""
Write-Host "Installed: $target"
Write-Host "Release:   $($release.tag_name)"
Write-Host "Shim:      $ShimPath"
Write-Host ""
Write-Host "Sessions/config are NOT touched:"
Write-Host "  config   %APPDATA%\g0d\config.toml"
Write-Host "  sessions %APPDATA%\g0d\sessions\"
Write-Host "  history  %LOCALAPPDATA%\g0d\history.txt"
Write-Host ""
Write-Host "Open a new terminal, then run: g0d"
& $target --version
