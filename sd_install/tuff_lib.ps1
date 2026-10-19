# Shared helpers for install.ps1 and add_skins.ps1 (ARCropolis preset handling).

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

function Get-PresetFiles([string]$sd) {
    return @(Get-ChildItem -Path (Join-Path $sd "ultimate/arcropolis/config") -Recurse -File -Filter "presets" -ErrorAction SilentlyContinue)
}

function Read-PresetIds([string]$path) {
    $text = [IO.File]::ReadAllText($path).Trim()
    $ids = @()
    if ($text -match '^\[(.*)\]$' -and $Matches[1].Trim() -ne "") { $ids = @($Matches[1].Split(',') | ForEach-Object { [uint64]$_.Trim() }) }
    return ,$ids
}

# Names of the mod folders ARCropolis has turned on. With no preset file, every folder that
# isn't hidden with a leading "." counts as on.
function Get-EnabledMods([string]$sd) {
    $modsDir = Join-Path $sd "ultimate/mods"
    $folders = @(Get-ChildItem -Path $modsDir -Directory -ErrorAction SilentlyContinue | Where-Object { -not $_.Name.StartsWith(".") })
    $presets = Get-PresetFiles $sd
    if ($presets.Count -eq 0) { return @($folders | ForEach-Object { $_.Name }) }
    $ids = @()
    foreach ($p in $presets) { $ids += Read-PresetIds $p.FullName }
    return @($folders | Where-Object { $ids -contains (Hash40 "sd:/ultimate/mods/$($_.Name)") } | ForEach-Object { $_.Name })
}

# Turns the given mod folders on in every ARCropolis preset file.
function Enable-Mods([string]$sd, [string[]]$names, [string]$stamp) {
    $presets = Get-PresetFiles $sd
    if ($presets.Count -eq 0) {
        Write-Host "No ARCropolis preset file found; turn the mods on in the ARCropolis mod menu instead." -ForegroundColor Yellow
        return
    }
    foreach ($p in $presets) {
        $ids = Read-PresetIds $p.FullName
        $added = @()
        foreach ($name in $names) {
            $name = $name.Trim()
            if ($name -eq "") { continue }
            if (-not (Test-Path (Join-Path $sd "ultimate/mods/$name"))) { Write-Host "  (skipping ${name}: folder not on SD)" -ForegroundColor Yellow; continue }
            $h = Hash40 "sd:/ultimate/mods/$name"
            if ($ids -notcontains $h) { $ids += $h; $added += $name }
        }
        if ($added.Count -gt 0) {
            Copy-Item $p.FullName "$($p.FullName).bak-$stamp" -Force
            [IO.File]::WriteAllText($p.FullName, "[" + ($ids -join ",") + "]", (New-Object Text.UTF8Encoding($false)))
            Write-Host "Turned on in ARCropolis: $($added -join ', ')"
        } else {
            Write-Host "Those mods were already turned on."
        }
    }
}
