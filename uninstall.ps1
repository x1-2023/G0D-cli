[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$InstallDirectory = Join-Path $env:LOCALAPPDATA 'Programs\g0d'
$InstalledBinary = Join-Path $InstallDirectory 'g0d.exe'
$ShimPath = Join-Path $env:APPDATA 'npm\g0d.cmd'

if (Test-Path -LiteralPath $InstalledBinary) {
    Remove-Item -LiteralPath $InstalledBinary -Force
}
if (Test-Path -LiteralPath $ShimPath) {
    Remove-Item -LiteralPath $ShimPath -Force
}
if ((Test-Path -LiteralPath $InstallDirectory) -and -not (Get-ChildItem -LiteralPath $InstallDirectory -Force)) {
    Remove-Item -LiteralPath $InstallDirectory -Force
}

$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$Entries = @($UserPath -split ';' | Where-Object {
    $_ -and $_.TrimEnd('\') -ine $InstallDirectory.TrimEnd('\')
})
[Environment]::SetEnvironmentVariable('Path', ($Entries -join ';'), 'User')

Write-Host "Uninstalled g0d from $InstallDirectory"
Write-Host 'Open a new terminal to refresh PATH.'
