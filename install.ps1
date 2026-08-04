[CmdletBinding()]
param(
    [string]$BinaryPath,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$SourceRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ManifestPath = Join-Path $SourceRoot 'cli\Cargo.toml'

if (-not $BinaryPath) {
    $BinaryPath = Join-Path $SourceRoot 'cli\target\release\g0d.exe'
}

if (-not $SkipBuild) {
    $Cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
    $CargoPath = if ($Cargo) { $Cargo.Source } else { $null }
    if (-not $Cargo) {
        $FallbackCargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
        if (Test-Path -LiteralPath $FallbackCargo) {
            $CargoPath = $FallbackCargo
        }
    }
    if (-not $CargoPath) {
        throw 'cargo.exe was not found. Install Rust or pass -BinaryPath with -SkipBuild.'
    }
    & $CargoPath build --release --manifest-path $ManifestPath
    if ($LASTEXITCODE -ne 0) {
        throw "Release build failed with exit code $LASTEXITCODE."
    }
}

$ResolvedBinary = (Resolve-Path -LiteralPath $BinaryPath).Path
$InstallDirectory = Join-Path $env:LOCALAPPDATA 'Programs\g0d'
$InstalledBinary = Join-Path $InstallDirectory 'g0d.exe'
$ShimDirectory = Join-Path $env:APPDATA 'npm'
$ShimPath = Join-Path $ShimDirectory 'g0d.cmd'
New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null
Copy-Item -LiteralPath $ResolvedBinary -Destination $InstalledBinary -Force
New-Item -ItemType Directory -Path $ShimDirectory -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $SourceRoot 'g0d.cmd') -Destination $ShimPath -Force

$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$Entries = @($UserPath -split ';' | Where-Object { $_ })
$AlreadyPresent = $Entries | Where-Object {
    $_.TrimEnd('\') -ieq $InstallDirectory.TrimEnd('\')
}
if (-not $AlreadyPresent) {
    $NewPath = (@($Entries) + $InstallDirectory) -join ';'
    [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
}
$ShimPresent = $Entries | Where-Object {
    $_.TrimEnd('\') -ieq $ShimDirectory.TrimEnd('\')
}
if (-not $ShimPresent) {
    $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    [Environment]::SetEnvironmentVariable('Path', "$UserPath;$ShimDirectory", 'User')
}
if (($env:Path -split ';') -notcontains $InstallDirectory) {
    $env:Path = "$InstallDirectory;$env:Path"
}

Write-Host "Installed: $InstalledBinary"
Write-Host "Command shim: $ShimPath"
Write-Host 'Open a new terminal, enter any project directory, then run: g0d'
& $InstalledBinary --version
