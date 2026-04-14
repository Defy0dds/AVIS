#!/usr/bin/env pwsh
# AVIS installer for Windows
# Usage: irm https://avis.sh/install.ps1 | iex

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Repo = "zunayed-io/AVIS"
$BinDir = Join-Path $env:USERPROFILE ".avis\bin"
$BinPath = Join-Path $BinDir "avis.exe"

Write-Host ""
Write-Host "  Installing AVIS..." -ForegroundColor Cyan
Write-Host ""

# Fetch latest release from GitHub API
try {
    $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
} catch {
    Write-Host "  ERROR: Failed to fetch latest release from GitHub." -ForegroundColor Red
    Write-Host "  $_" -ForegroundColor Red
    exit 1
}

$Version = $Release.tag_name
$Asset = $Release.assets | Where-Object { $_.name -eq "avis-windows-x64.exe" }

if (-not $Asset) {
    Write-Host "  ERROR: No Windows binary found in release $Version." -ForegroundColor Red
    Write-Host "  Available assets:" -ForegroundColor Yellow
    $Release.assets | ForEach-Object { Write-Host "    - $($_.name)" }
    exit 1
}

$DownloadUrl = $Asset.browser_download_url

# Create bin directory
if (-not (Test-Path $BinDir)) {
    New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
}

# Download binary
Write-Host "  Downloading avis $Version..." -ForegroundColor White
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $BinPath -UseBasicParsing
} catch {
    Write-Host "  ERROR: Download failed." -ForegroundColor Red
    Write-Host "  $_" -ForegroundColor Red
    exit 1
}

# Add to user PATH if not already present
$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($UserPath -split ';' | Where-Object { $_ -eq $BinDir }) {
    Write-Host "  PATH already contains $BinDir" -ForegroundColor Gray
} else {
    $NewPath = if ($UserPath) { "$UserPath;$BinDir" } else { $BinDir }
    [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
    Write-Host "  Added $BinDir to user PATH" -ForegroundColor Green
}

Write-Host ""
Write-Host "  AVIS $Version installed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "  Binary:  $BinPath" -ForegroundColor White
Write-Host "  Run:     avis --help" -ForegroundColor White
Write-Host ""
Write-Host "  Restart your terminal for PATH changes to take effect." -ForegroundColor Yellow
Write-Host ""
