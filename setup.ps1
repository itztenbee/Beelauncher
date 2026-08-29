# Beelauncher setup script
# Rechtsklick auf diese Datei -> "Mit PowerShell ausfuehren", oder in einem
# PowerShell-Fenster: .\setup.ps1
#
# Installiert automatisch: Rust, Visual Studio Build Tools (C++ Workload),
# und die npm-Pakete. Danach noch manuell: Icon generieren + Build starten
# (siehe Ausgabe am Ende).

$ErrorActionPreference = "Stop"

Write-Host "== Beelauncher Setup ==" -ForegroundColor Cyan

# 1. Rust
if (Get-Command cargo -ErrorAction SilentlyContinue) {
    Write-Host "Rust ist schon installiert: $(cargo --version)" -ForegroundColor Green
} else {
    Write-Host "Installiere Rust via winget..." -ForegroundColor Yellow
    winget install --id Rustlang.Rustup -e --accept-package-agreements --accept-source-agreements
    Write-Host "Rust installiert. WICHTIG: Terminal neu starten, sonst wird cargo nicht erkannt!" -ForegroundColor Yellow
}

# 2. Node.js
if (Get-Command node -ErrorAction SilentlyContinue) {
    Write-Host "Node.js ist schon installiert: $(node --version)" -ForegroundColor Green
} else {
    Write-Host "Installiere Node.js via winget..." -ForegroundColor Yellow
    winget install --id OpenJS.NodeJS.LTS -e --accept-package-agreements --accept-source-agreements
}

# 3. Visual Studio Build Tools (C++ Workload) -- noetig damit Rust unter Windows linken kann
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasCppTools = $false
if (Test-Path $vsWhere) {
    $installed = & $vsWhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($installed) { $hasCppTools = $true }
}

if ($hasCppTools) {
    Write-Host "C++ Build Tools sind schon installiert." -ForegroundColor Green
} else {
    Write-Host "Installiere Visual Studio Build Tools (C++ Workload) via winget..." -ForegroundColor Yellow
    Write-Host "Das dauert ein paar Minuten und braucht ca. 2-3 GB Speicherplatz." -ForegroundColor Yellow
    winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-package-agreements --accept-source-agreements --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools"
}

# 4. npm Pakete installieren (im gleichen Ordner wie dieses Skript)
$projectDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectDir

if (Get-Command npm -ErrorAction SilentlyContinue) {
    Write-Host "Installiere npm-Pakete..." -ForegroundColor Yellow
    npm install
} else {
    Write-Host "npm nicht gefunden -- Terminal neu starten und dieses Skript nochmal ausfuehren." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "== Setup fertig ==" -ForegroundColor Cyan
Write-Host "Naechste Schritte:" -ForegroundColor White
Write-Host "  1. Falls Rust/Node gerade neu installiert wurden: Terminal SCHLIESSEN und neu oeffnen" -ForegroundColor White
Write-Host "  2. Icon generieren:  npx tauri icon pfad\zu\deinem-bild.png" -ForegroundColor White
Write-Host "  3. Testen:           npm run tauri dev" -ForegroundColor White
Write-Host "  4. Fertig bauen:     npm run tauri build" -ForegroundColor White
