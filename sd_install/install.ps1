# Project TUFF SD installer.
# Copies everything in this folder (laid out like the SD card) onto the Switch SD card,
# backing up any file it replaces, and turns on the mods listed in enable_mods.txt in the
# ARCropolis preset. Run it with "Install to SD.bat" in the Project TUFF folder.

param(
    [string]$Target,    # optional: install into this folder instead of auto-detecting the SD card
    [string]$SkinsDir   # optional: where the extra skins are (default D:\TuffCharacterSkins)
)

$ErrorActionPreference = "Stop"
$src = $PSScriptRoot
. (Join-Path $src "tuff_lib.ps1")

function Fail($msg) { Write-Host ""; Write-Host "ERROR: $msg" -ForegroundColor Red; exit 1 }

# ---- find the SD card --------------------------------------------------------------------------
$sd = $null
if ($Target) { $sd = (Resolve-Path $Target).Path.TrimEnd('\') + '\' }
else { foreach ($d in Get-PSDrive -PSProvider FileSystem) {
    $root = $d.Root
    if ((Test-Path (Join-Path $root "atmosphere\contents\01006A800016E000")) -and (Test-Path (Join-Path $root "ultimate\mods"))) {
        $sd = $root
        break
    }
} }
if (-not $sd) { Fail "Couldn't find the Switch SD card. Put it in the PC and run this again." }
Write-Host "SD card found at $sd" -ForegroundColor Green

# ---- copy files, backing up anything replaced ---------------------------------------------------
$stamp = Get-Date -Format "yyyy-MM-dd_HH-mm-ss"
$backup = Join-Path $sd "hdr-melee-fox-dev\backup\installer-$stamp"
$skip = @("install.ps1", "add_skins.ps1", "tuff_lib.ps1", "enable_mods.txt", "README.txt")
$files = Get-ChildItem -Path $src -Recurse -File | Where-Object { $skip -notcontains $_.Name -or $_.DirectoryName -ne $src }
$copied = 0; $backedUp = 0
foreach ($f in $files) {
    $rel = $f.FullName.Substring($src.Length).TrimStart('\')
    $dest = Join-Path $sd $rel
    if (Test-Path $dest) {
        if ((Get-FileHash $dest).Hash -eq (Get-FileHash $f.FullName).Hash) { continue }
        $bak = Join-Path $backup $rel
        New-Item -ItemType Directory -Force (Split-Path $bak) | Out-Null
        Copy-Item $dest $bak -Force
        $backedUp++
    }
    New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
    Copy-Item $f.FullName $dest -Force
    $copied++
}
Write-Host "Copied $copied file(s); backed up $backedUp replaced file(s)."
if ($backedUp -gt 0) { Write-Host "Backups: $backup" }

# ---- enable mods in the ARCropolis preset -------------------------------------------------------
$listFile = Join-Path $src "enable_mods.txt"
if (Test-Path $listFile) { Enable-Mods $sd @(Get-Content $listFile) $stamp }

# ---- add the extra skins (installed skins keep their slots) -------------------------------------
& (Join-Path $src "add_skins.ps1") -Sd $sd -SkinsDir $SkinsDir -Stamp $stamp

Write-Host ""
Write-Host "Done. Safely eject the SD card, then put it in the Switch." -ForegroundColor Green
