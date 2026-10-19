# Adds every character skin in a folder (default D:\TuffCharacterSkins) to the SD card as its own
# ARCropolis mod, without touching the skins already installed.
#
# Installed skins have priority: a costume slot (c00-c07) that an enabled mod already uses is
# never taken. A new skin keeps its own slot when that slot is free; otherwise it is moved
# ("reslotted") to the lowest free slot. If a fighter has no free slots left, that skin is skipped.
# Skins that are already on the SD card (same textures) are skipped too, so running this again is safe.
#
# Each folder or archive (.zip, plus .7z/.rar when 7-Zip is installed) in the skins folder is one
# skin. Archives holding several mods (e.g. "Option A" / "Option B" folders) give one skin per mod.
#
# Normally run by install.ps1. To see what it would do without changing anything:
#   powershell -ExecutionPolicy Bypass -File add_skins.ps1 -Sd D:\ -DryRun

param(
    [Parameter(Mandatory = $true)][string]$Sd,
    [string]$SkinsDir,
    [string]$Stamp = (Get-Date -Format "yyyy-MM-dd_HH-mm-ss"),
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "tuff_lib.ps1")

$Sd = $Sd.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
$modsDir = Join-Path $Sd "ultimate/mods"
$hdrMods = @("hdr", "hdr-assets", "hdr-stages")
# File types ARCropolis merges across mods instead of reporting a conflict.
$mergeable = @(".prcx", ".prcxml", ".xmsbt", ".patch3audio", ".stprm", ".stdat")

if (-not $SkinsDir) {
    $candidates = @("D:\TuffCharacterSkins", (Join-Path $Sd "TuffCharacterSkins"), (Join-Path (Split-Path $PSScriptRoot) "CharacterSkins"))
    foreach ($c in $candidates) { if (Test-Path $c) { $SkinsDir = $c; break } }
}
if (-not $SkinsDir -or -not (Test-Path $SkinsDir)) {
    Write-Host "No skins folder found (looked for D:\TuffCharacterSkins); no skins added."
    return
}
$SkinsDir = (Resolve-Path $SkinsDir).Path
Write-Host ""
Write-Host "Adding skins from $SkinsDir" -ForegroundColor Cyan
if ($DryRun) { Write-Host "(dry run: nothing will be changed)" -ForegroundColor Yellow }

# ---- what's already installed -------------------------------------------------------------------
$enabled = Get-EnabledMods $Sd
$occupied = @{}      # fighter -> list of used slot numbers
$installed = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
$texBySize = @{}     # file size -> full paths of installed skin textures (for duplicate detection)

function Add-Occupied([string]$fighter, [int]$slot) {
    if (-not $occupied.ContainsKey($fighter)) { $occupied[$fighter] = New-Object 'System.Collections.Generic.HashSet[int]' }
    [void]$occupied[$fighter].Add($slot)
}
function Test-Occupied([string]$fighter, [int]$slot) {
    return $occupied.ContainsKey($fighter) -and $occupied[$fighter].Contains($slot)
}

foreach ($m in $enabled) {
    $root = Join-Path $modsDir $m
    foreach ($f in Get-ChildItem -Path $root -Recurse -File) {
        $rel = $f.FullName.Substring($root.Length + 1).Replace('\', '/').ToLowerInvariant()
        if (-not $rel.Contains('/')) { continue }   # info.toml, config.json, previews
        [void]$installed.Add($rel)
        # HDR's own mods edit some fighter files but aren't skins, so they don't claim slots.
        if ($hdrMods -notcontains $m -and $rel -match '^fighter/([^/]+)/model/body/c(\d\d)/') {
            Add-Occupied $Matches[1] ([int]$Matches[2])
            if ($f.Extension -eq ".nutexb") {
                if (-not $texBySize.ContainsKey($f.Length)) { $texBySize[$f.Length] = @() }
                $texBySize[$f.Length] += $f.FullName
            }
        }
    }
}
foreach ($k in ($occupied.Keys | Sort-Object)) {
    Write-Host ("  in use: {0} {1}" -f $k, (($occupied[$k] | Sort-Object | ForEach-Object { "c{0:D2}" -f $_ }) -join " "))
}

$hashCache = @{}
function Get-Md5([string]$path) {
    if (-not $hashCache.ContainsKey($path)) { $hashCache[$path] = (Get-FileHash -LiteralPath $path -Algorithm MD5).Hash }
    return $hashCache[$path]
}

# ---- unpack the skins ---------------------------------------------------------------------------
$sevenZip = @("$env:ProgramFiles\7-Zip\7z.exe", "${env:ProgramFiles(x86)}\7-Zip\7z.exe") | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
$tmp = Join-Path $env:TEMP "tuff-skins"
if (Test-Path $tmp) { Remove-Item $tmp -Recurse -Force }
New-Item -ItemType Directory -Force $tmp | Out-Null

function Get-SafeName([string]$s) {
    $n = ($s.ToLowerInvariant() -replace '[^a-z0-9]+', '-').Trim('-')
    if ($n -eq "") { $n = "skin" }
    return $n
}

$sources = @()   # each: @{ Name; Base }
foreach ($item in Get-ChildItem -Path $SkinsDir | Sort-Object Name) {
    if ($item.PSIsContainer) { $sources += @{ Name = $item.Name; Base = $item.FullName }; continue }
    $ext = $item.Extension.ToLowerInvariant()
    if (@(".zip", ".7z", ".rar") -notcontains $ext) { continue }
    $out = Join-Path $tmp (Get-SafeName $item.BaseName)
    New-Item -ItemType Directory -Force $out | Out-Null
    try {
        if ($sevenZip) {
            & $sevenZip x $item.FullName "-o$out" -y | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "7-Zip exit code $LASTEXITCODE" }
        } elseif ($ext -eq ".zip") {
            Expand-Archive -LiteralPath $item.FullName -DestinationPath $out -Force
        } else {
            Write-Host "  SKIPPED $($item.Name): install 7-Zip to unpack $ext files (or unzip it into a folder here)." -ForegroundColor Yellow
            continue
        }
    } catch {
        Write-Host "  SKIPPED $($item.Name): couldn't unpack it ($($_.Exception.Message))." -ForegroundColor Yellow
        continue
    }
    $sources += @{ Name = $item.BaseName; Base = $out }
}

# A mod root is the folder holding fighter\<name>\model\body\cXX.
function Find-ModRoots([string]$base) {
    $roots = @(Get-ChildItem -Path $base -Recurse -Directory |
        Where-Object { $_.Name -match '^c\d\d$' -and $_.Parent.Name -eq "body" -and $_.Parent.Parent.Name -eq "model" -and $_.Parent.Parent.Parent.Parent.Name -eq "fighter" } |
        ForEach-Object { $_.Parent.Parent.Parent.Parent.Parent.FullName } | Sort-Object -Unique)
    return @($roots | Where-Object { $r = $_; -not ($roots | Where-Object { $_ -ne $r -and $r.StartsWith($_ + [IO.Path]::DirectorySeparatorChar) }) })
}

# ---- reslotting ---------------------------------------------------------------------------------
$script:slotMap = @{}   # old slot -> new slot (-1 = no room, file dropped)
$script:dropped = $false

function Map-Slot([string]$digits) {
    $old = [int]$digits
    if (-not $script:slotMap.ContainsKey($old)) { return $digits }
    $new = $script:slotMap[$old]
    if ($new -lt 0) { $script:dropped = $true; return $digits }
    return "{0:D2}" -f $new
}

# Returns the path with its costume slot moved, or $null if that slot has no room.
function Remap-Path([string]$rel) {
    $script:dropped = $false
    $r = $rel
    if ($r -match '^(fighter|camera/fighter|effect/fighter)/') {
        $r = [regex]::Replace($r, '(?<=/)c(\d\d)(?=/|$)', { param($m) "c" + (Map-Slot $m.Groups[1].Value) }, 'IgnoreCase')
        $r = [regex]::Replace($r, '_c(\d\d)(?=[._/]|$)', { param($m) "_c" + (Map-Slot $m.Groups[1].Value) }, 'IgnoreCase')
    } elseif ($r -match '^sound/bank/') {
        $r = [regex]::Replace($r, '_c(\d\d)(?=\.)', { param($m) "_c" + (Map-Slot $m.Groups[1].Value) }, 'IgnoreCase')
    } elseif ($r -match '^ui/replace(_patch)?/chara/') {
        $r = [regex]::Replace($r, '_(\d\d)(?=\.bntx$)', { param($m) "_" + (Map-Slot $m.Groups[1].Value) }, 'IgnoreCase')
    }
    if ($script:dropped) { return $null }
    return $r
}

function Remap-Value($v) {
    if ($null -eq $v) { return $null }
    if ($v -is [string]) { return (Remap-Path $v) }
    $list = New-Object System.Collections.ArrayList
    foreach ($x in $v) { $y = Remap-Value $x; if ($null -ne $y) { [void]$list.Add($y) } }
    return ,([object[]]$list.ToArray())
}

# Moves the slots in a config.json. share-to-vanilla keys and new-dir-infos-base values name the
# game's own files, so they stay.
function Remap-Config($cfg) {
    $out = [ordered]@{}
    foreach ($section in $cfg.PSObject.Properties) {
        $val = $section.Value
        if ($val -is [System.Management.Automation.PSCustomObject]) {
            $o = [ordered]@{}
            foreach ($p in $val.PSObject.Properties) {
                $key = $p.Name
                if ($section.Name -ne "share-to-vanilla") { $key = Remap-Path $p.Name }
                if ($null -eq $key) { continue }
                # new-dir-infos-base values are the game's own folders the new slot is based on.
                if ($section.Name -eq "new-dir-infos-base") { $o[$key] = $p.Value; continue }
                $nv = Remap-Value $p.Value
                if ($null -eq $nv) { continue }
                $o[$key] = $nv
            }
            $out[$section.Name] = $o
        } elseif ($val -is [array]) {
            $out[$section.Name] = Remap-Value $val
        } else {
            $out[$section.Name] = $val
        }
    }
    return $out
}

# ---- install each skin --------------------------------------------------------------------------
$newMods = @()
$report = @()

foreach ($src in $sources) {
    $roots = Find-ModRoots $src.Base
    if ($roots.Count -eq 0) { $report += "SKIPPED $($src.Name): no fighter costume found in it."; continue }
    foreach ($root in $roots) {
        $label = $src.Name
        if ($roots.Count -gt 1 -or $root -ne $src.Base) {
            $sub = $root.Substring($src.Base.Length).Trim('\', '/')
            if ($sub -ne "") { $label = "$($src.Name) ($sub)" }
        }
        $modName = "skin-" + (Get-SafeName $label)
        if ($modName.Length -gt 60) { $modName = $modName.Substring(0, 60).TrimEnd('-') }
        if (Test-Path (Join-Path $modsDir $modName)) { $report += "already installed: $label ($modName)"; continue }

        $files = @(Get-ChildItem -Path $root -Recurse -File)
        $slots = New-Object 'System.Collections.Generic.SortedSet[int]'
        $fighters = New-Object 'System.Collections.Generic.SortedSet[string]'
        $bodyTex = @()
        foreach ($f in $files) {
            $rel = $f.FullName.Substring($root.Length + 1).Replace('\', '/').ToLowerInvariant()
            if ($rel -match '^fighter/([^/]+)/model/body/c(\d\d)/') {
                [void]$fighters.Add($Matches[1]); [void]$slots.Add([int]$Matches[2])
                if ($f.Extension -eq ".nutexb") { $bodyTex += $f }
            }
        }

        # Same textures as a skin that's already installed -> it's the same skin.
        if ($bodyTex.Count -gt 0) {
            $same = 0
            foreach ($t in $bodyTex) {
                if (-not $texBySize.ContainsKey($t.Length)) { continue }
                $h = Get-Md5 $t.FullName
                if ($texBySize[$t.Length] | Where-Object { (Get-Md5 $_) -eq $h }) { $same++ }
            }
            if ($same * 2 -ge $bodyTex.Count) { $report += "already installed: $label (same textures as an installed skin)"; continue }
        }

        # Pick slots. Every fighter in the skin (e.g. both Ice Climbers) moves together.
        $isFree = { param($s) -not ($fighters | Where-Object { Test-Occupied $_ $s }) }
        $script:slotMap = @{}
        $taken = New-Object 'System.Collections.Generic.HashSet[int]'
        foreach ($s in $slots) { if (& $isFree $s) { $script:slotMap[$s] = $s; [void]$taken.Add($s) } }
        foreach ($s in $slots) {
            if ($script:slotMap.ContainsKey($s)) { continue }
            $script:slotMap[$s] = -1
            if ($s -gt 7) { continue }   # extra slots (c08+) are only used where the skin put them
            for ($t = 0; $t -le 7; $t++) {
                if (-not $taken.Contains($t) -and -not $slots.Contains($t) -and (& $isFree $t)) { $script:slotMap[$s] = $t; [void]$taken.Add($t); break }
            }
        }
        $kept = @($slots | Where-Object { $script:slotMap[$_] -ge 0 })
        $who = ($fighters -join "+")
        if ($kept.Count -eq 0) { $report += "SKIPPED ${label}: no free $who slots left."; continue }
        $moved = @($slots | Where-Object { $script:slotMap[$_] -ge 0 -and $script:slotMap[$_] -ne $_ })
        $slotText = (($slots | ForEach-Object {
            $n = $script:slotMap[$_]
            if ($n -lt 0) { "c{0:D2}->(no room)" -f $_ } elseif ($n -ne $_) { "c{0:D2}->c{1:D2}" -f $_, $n } else { "c{0:D2}" -f $_ }
        }) -join " ")

        # Copy, moving slots and leaving out any file another enabled mod already provides.
        $dest = Join-Path $modsDir $modName
        $conflicts = @(); $count = 0; $targets = @()
        foreach ($f in $files) {
            $rel = $f.FullName.Substring($root.Length + 1).Replace('\', '/')
            if (-not $rel.Contains('/')) {
                if ($rel -ieq "config.json") { continue }
                $targets += @{ From = $f.FullName; To = $rel }; continue
            }
            $new = Remap-Path $rel
            if ($null -eq $new) { continue }
            if ($installed.Contains($new) -and $mergeable -notcontains $f.Extension.ToLowerInvariant()) { $conflicts += $new; continue }
            $targets += @{ From = $f.FullName; To = $new }
            $count++
        }
        if ($count -eq 0) { $report += "SKIPPED ${label}: every file clashes with an installed mod."; continue }

        $cfgPath = Join-Path $root "config.json"
        $cfgText = $null
        if (Test-Path $cfgPath) {
            if ($moved.Count -eq 0 -and $kept.Count -eq $slots.Count) { $cfgText = [IO.File]::ReadAllText($cfgPath) }
            else {
                try { $cfgText = (Remap-Config ([IO.File]::ReadAllText($cfgPath) | ConvertFrom-Json)) | ConvertTo-Json -Depth 20 }
                catch { $report += "SKIPPED ${label}: couldn't read its config.json ($($_.Exception.Message))."; continue }
            }
        }

        if (-not $DryRun) {
            foreach ($t in $targets) {
                $to = Join-Path $dest $t.To
                New-Item -ItemType Directory -Force (Split-Path $to) | Out-Null
                Copy-Item -LiteralPath $t.From -Destination $to -Force
            }
            if ($cfgText) { [IO.File]::WriteAllText((Join-Path $dest "config.json"), $cfgText, (New-Object Text.UTF8Encoding($false))) }
            [IO.File]::WriteAllText((Join-Path $dest "tuff_skin_source.txt"), "Added by Project TUFF add_skins.ps1 on $Stamp`r`nSource: $label`r`nSlots: $slotText`r`n")
        }
        foreach ($t in $targets) { if ($t.To.Contains('/')) { [void]$installed.Add($t.To.ToLowerInvariant()) } }
        foreach ($fi in $fighters) { foreach ($s in $kept) { Add-Occupied $fi $script:slotMap[$s] } }
        foreach ($t in $bodyTex) {
            if (-not $texBySize.ContainsKey($t.Length)) { $texBySize[$t.Length] = @() }
            $texBySize[$t.Length] += $t.FullName
        }
        $newMods += $modName

        $line = "ADDED ${label} -> $modName : $who $slotText"
        if ($conflicts.Count -gt 0) { $line += " ($($conflicts.Count) file(s) left out because an installed mod already has them, e.g. $($conflicts[0]))" }
        $report += $line
    }
}

Write-Host ""
foreach ($r in $report) {
    $color = "Gray"
    if ($r.StartsWith("ADDED")) { $color = "Green" } elseif ($r.StartsWith("SKIPPED")) { $color = "Yellow" }
    Write-Host "  $r" -ForegroundColor $color
}
if ($newMods.Count -gt 0 -and -not $DryRun) { Enable-Mods $Sd $newMods $Stamp }
if ($newMods.Count -eq 0) { Write-Host "  No new skins to add." }
Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
