# datadiff installer for Windows: downloads the latest release binary —
# no compiler needed.
#
# Usage (PowerShell):
#   irm https://raw.githubusercontent.com/cloudroad-io/datadiff/main/install.ps1 | iex
#
# Optional: $env:DATADIFF_INSTALL_DIR = 'C:\custom\dir' before running
# (default: %LOCALAPPDATA%\Programs\datadiff)
$ErrorActionPreference = 'Stop'

$Repo = 'cloudroad-io/datadiff'
$InstallDir = if ($env:DATADIFF_INSTALL_DIR) {
    $env:DATADIFF_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA 'Programs\datadiff'
}

$release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
$tag = $release.tag_name
$asset = "datadiff-$tag-x86_64-pc-windows-msvc.zip"
$url = "https://github.com/$Repo/releases/download/$tag/$asset"

$tmp = New-Item -ItemType Directory -Path (Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName()))
try {
    Write-Host "Downloading $url"
    Invoke-WebRequest $url -OutFile (Join-Path $tmp $asset)
    Expand-Archive (Join-Path $tmp $asset) -DestinationPath $tmp
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item (Join-Path $tmp 'datadiff.exe') (Join-Path $InstallDir 'datadiff.exe') -Force
} finally {
    Remove-Item -Recurse -Force $tmp
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$InstallDir", 'User')
    Write-Host "Added $InstallDir to the user PATH (restart the terminal to pick it up)"
}
Write-Host "Installed datadiff $tag to $InstallDir\datadiff.exe"
