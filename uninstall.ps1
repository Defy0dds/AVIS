#!/usr/bin/env pwsh
# AVIS uninstaller for Windows
# Usage: irm https://avis.sh/uninstall.ps1 | iex

$ErrorActionPreference = 'Stop'

$AvisDir = Join-Path $env:USERPROFILE ".avis"
$BinDir = Join-Path $AvisDir "bin"

Write-Host ""
Write-Host "  Uninstalling AVIS..." -ForegroundColor Cyan
Write-Host ""

# Remove binary
if (Test-Path $BinDir) {
    Remove-Item -Path $BinDir -Recurse -Force
    Write-Host "  Removed $BinDir" -ForegroundColor Green
} else {
    Write-Host "  Binary directory not found (already removed?)" -ForegroundColor Gray
}

# Remove from user PATH
$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($UserPath) {
    $Entries = $UserPath -split ';' | Where-Object { $_ -ne $BinDir -and $_ -ne '' }
    $NewPath = $Entries -join ';'
    if ($NewPath -ne $UserPath) {
        [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
        Write-Host "  Removed $BinDir from user PATH" -ForegroundColor Green
    } else {
        Write-Host "  PATH entry not found (already removed?)" -ForegroundColor Gray
    }
}

Write-Host ""
Write-Host "  AVIS binary uninstalled." -ForegroundColor Green
Write-Host ""

# Warn about data directory — never delete silently
if (Test-Path $AvisDir) {
    Write-Host "  Your identities and credentials are still in:" -ForegroundColor Yellow
    Write-Host "    $AvisDir" -ForegroundColor White
    Write-Host ""
    Write-Host "  To remove all data, run:" -ForegroundColor Yellow
    Write-Host "    Remove-Item -Path `"$AvisDir`" -Recurse -Force" -ForegroundColor White
}

Write-Host ""
Write-Host "  Restart your terminal for PATH changes to take effect." -ForegroundColor Yellow
Write-Host ""
