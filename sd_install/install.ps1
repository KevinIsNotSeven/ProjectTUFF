# Project TUFF SD installer.
# Copies everything in this folder (laid out like the SD card) onto the Switch SD card,
# backing up any file it replaces, and turns on the mods listed in enable_mods.txt in the
# ARCropolis preset. Run it with "Install to SD.bat" in the Project TUFF folder.

param([string]$Target)  # optional: install into this folder instead of auto-detecting the SD card

$ErrorActionPreference = "Stop"
$src = $PSScriptRoot

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
$skip = @("install.ps1", "enable_mods.txt", "README.txt")
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
function Hash40([string]$s) {
    $t = New-Object 'uint32[]' 256
    for ($i = 0; $i -lt 256; $i++) {
        $c = [uint32]$i
        for ($k = 0; $k -lt 8; $k++) { if ($c -band 1) { $c = [uint32](([uint32]3988292384) -bxor ($c -shr 1)) } else { $c = [uint32]($c -shr 1) } }
        $t[$i] = $c
    }
    $crc = [uint32]4294967295
    foreach ($b in [Text.Encoding]::ASCII.GetBytes($s)) { $crc = [uint32]($t[($crc -bxor $b) -band 0xFF] -bxor ($crc -shr 8)) }
    $crc = [uint32]($crc -bxor [uint32]4294967295)
    return [uint64]$crc + ([uint64]$s.Length -shl 32)
}

$listFile = Join-Path $src "enable_mods.txt"
if (Test-Path $listFile) {
    $presets = Get-ChildItem -Path (Join-Path $sd "ultimate\arcropolis\config") -Recurse -File -Filter "presets" -ErrorAction SilentlyContinue
    if (-not $presets) {
        Write-Host "No ARCropolis preset file found; turn the mods on in the ARCropolis mod menu instead." -ForegroundColor Yellow
    }
    foreach ($p in $presets) {
        $text = [IO.File]::ReadAllText($p.FullName).Trim()
        $ids = @()
        if ($text -match '^\[(.*)\]$' -and $Matches[1].Trim() -ne "") { $ids = $Matches[1].Split(',') | ForEach-Object { [uint64]$_.Trim() } }
        $added = @()
        foreach ($name in Get-Content $listFile) {
            $name = $name.Trim()
            if ($name -eq "") { continue }
            if (-not (Test-Path (Join-Path $sd "ultimate\mods\$name"))) { Write-Host "  (skipping ${name}: folder not on SD)" -ForegroundColor Yellow; continue }
            $h = Hash40 "sd:/ultimate/mods/$name"
            if ($ids -notcontains $h) { $ids += $h; $added += $name }
        }
        if ($added.Count -gt 0) {
            Copy-Item $p.FullName "$($p.FullName).bak-$stamp" -Force
            [IO.File]::WriteAllText($p.FullName, "[" + ($ids -join ",") + "]", (New-Object Text.UTF8Encoding($false)))
            Write-Host "Turned on in ARCropolis: $($added -join ', ')"
        } else {
            Write-Host "Mods in enable_mods.txt were already turned on."
        }
    }
}

Write-Host ""
Write-Host "Done. Safely eject the SD card, then put it in the Switch." -ForegroundColor Green
