[CmdletBinding()]
param(
    [switch]$Release,
    [switch]$Test,
    [switch]$Install
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$Manifest = Join-Path $Root 'cli\Cargo.toml'
$Cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
$CargoPath = if ($Cargo) { $Cargo.Source } else { $null }
if (-not $CargoPath) {
    $Fallback = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    if (Test-Path -LiteralPath $Fallback) {
        $CargoPath = $Fallback
    }
}
if (-not $CargoPath) {
    throw 'cargo.exe was not found. Install Rust from https://rustup.rs first.'
}

$Profile = if ($Release -or $Install) { 'release' } else { 'dev' }
$Args = @('build', '--manifest-path', $Manifest)
if ($Profile -eq 'release') {
    $Args += '--release'
}

Write-Host "Building g0d ($Profile)..."
& $CargoPath @Args
if ($LASTEXITCODE -ne 0) {
    throw "Build failed with exit code $LASTEXITCODE"
}

if ($Test) {
    Write-Host 'Running tests...'
    $TestArgs = @('test', '--manifest-path', $Manifest, '--bins')
    if ($Profile -eq 'release') {
        $TestArgs += '--release'
    }
    & $CargoPath @TestArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Tests failed with exit code $LASTEXITCODE"
    }
}

$Binary = if ($Profile -eq 'release') {
    Join-Path $Root 'cli\target\release\g0d.exe'
} else {
    Join-Path $Root 'cli\target\debug\g0d.exe'
}

if (Test-Path -LiteralPath $Binary) {
    Copy-Item -LiteralPath $Binary -Destination (Join-Path $Root 'g0d.exe') -Force
    Write-Host "Binary: $Binary"
    & $Binary --version
}

if ($Install) {
    & (Join-Path $Root 'install.ps1') -BinaryPath $Binary -SkipBuild
}
