# Build release binary, copy asset name for GitHub Releases, optionally publish with gh.
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\scripts\publish-release.ps1
#   powershell -ExecutionPolicy Bypass -File .\scripts\publish-release.ps1 -Tag v1.13.0 -Publish

[CmdletBinding()]
param(
    [string]$Tag,
    [switch]$Publish,
    [string]$Notes = 'g0d Windows release'
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path -LiteralPath $Cargo)) {
    $cmd = Get-Command cargo.exe -ErrorAction SilentlyContinue
    if ($cmd) { $Cargo = $cmd.Source } else { throw 'cargo.exe not found' }
}

$Manifest = Join-Path $Root 'cli\Cargo.toml'
& $Cargo build --release --manifest-path $Manifest
if ($LASTEXITCODE -ne 0) { throw "cargo build failed: $LASTEXITCODE" }

$Built = Join-Path $Root 'cli\target\release\g0d.exe'
if (-not (Test-Path -LiteralPath $Built)) { throw "Missing $Built" }

$Dist = Join-Path $Root 'dist'
New-Item -ItemType Directory -Path $Dist -Force | Out-Null
$Asset = Join-Path $Dist 'g0d-windows-x64.exe'
Copy-Item -LiteralPath $Built -Destination $Asset -Force
Copy-Item -LiteralPath $Built -Destination (Join-Path $Root 'g0d.exe') -Force

$Version = (& $Built --version) -replace '^g0d\s+', ''
if (-not $Tag) { $Tag = "v$Version" }

Write-Host "Asset: $Asset"
Write-Host "Tag:   $Tag"
Write-Host "Ver:   $Version"

if ($Publish) {
    $gh = Get-Command gh.exe -ErrorAction SilentlyContinue
    if (-not $gh) { throw 'gh CLI not found. Install GitHub CLI or upload dist\g0d-windows-x64.exe manually.' }
    & $gh.Source release create $Tag $Asset --title $Tag --notes $Notes --repo x1-2023/G0D-cli
    if ($LASTEXITCODE -ne 0) { throw "gh release create failed: $LASTEXITCODE" }
    Write-Host "Published $Tag"
    Write-Host 'Users can install with:'
    Write-Host '  irm https://raw.githubusercontent.com/x1-2023/G0D-cli/main/install-remote.ps1 | iex'
} else {
    Write-Host 'Built only. To publish:'
    Write-Host "  gh release create $Tag `"$Asset`" --title $Tag --notes `"$Notes`""
}
