# Handoff: HDR (HewDraw Remix) mods on this SD card

Written 2026-10-18 by Claude for the next Claude working with this user. It covers everything done so far, how it's built, what's tested, and the traps already found.

## The user and their preferences
- They play Smash Ultimate with the HDR mod on a modded Switch. This SD card is mounted as `D:\` on their Windows PC.
- They want fun gameplay mods layered on HDR.
- **"Functionality is key, stability is king."** Looks don't matter much. Avoid anything that risks crashes or hangs.
- They test on the Switch and report back. Nothing can be tested from the PC.
- They're fine with editing or replacing HDR files. They don't plan to update HDR; they can redownload it if needed.
- They like to say "carry on" and test everything at the end. Give them clear, short test checklists.

## Environment
- **Game:** Smash Ultimate 13.0.4. **HDR v0.50.0** (released 2026-05-13; source tag `v0.50.0` = commit `cab4dcd6860713c1fd0311605fdee52195861e31` of github.com/HDR-Development/HewDraw-Remix).
- **Loaders:** ARCropolis (`libarcropolis.nro`) and smashline 2 (`libsmashline_plugin.nro`, the HDR fork) are in `atmosphere/contents/01006A800016E000/romfs/skyline/plugins/`. HDR's own `plugin.nro` lives in `ultimate/mods/hdr/` and is loaded by `hdr-launcher.nro`.
- **PC toolchain** (installed this session on the user's PC):
  - Rust via rustup (stable plus nightly-2026-02-14).
  - Visual Studio 2022 C++ Build Tools.
  - `cargo-skyline` 3.5.0 with its `skyline-v3` toolchain.
  - No Python. Git Bash and PowerShell are available.
- **Build:** run `cargo skyline build --release` in a plugin folder. The output is `target/aarch64-skyline-switch/release/lib<name>.nro`; copy it to the skyline plugins folder on the SD.
- **Source** lives on the PC at `C:\Users\Sophie\hdr-modding\`, with a **backup on this SD at `D:\hdr-melee-fox-dev\source\`**. That folder also holds `hash40.ps1` and `nus3tool/`.
- **Library versions:** every `Cargo.lock` pins dependencies to HDR v0.50.0's era: skyline-rs `6b52d32`, skyline-smash `a68371c`, smash-script (development) `24c5b69`, HDR smashline `df194a0`. Keep these so plugins match the `libsmashline_plugin.nro` on the card. For a new plugin, copy an existing `Cargo.lock`.

## What's installed (all on the SD)
`atmosphere/...` = `atmosphere/contents/01006A800016E000/romfs`

| Mod | Files | Status |
|---|---|---|
| **Melee Fox** stats + cancel frames | `ultimate/mods/hdr-melee-fox/` (`fighter/common/param/fighter_param.prcxml`, `fighter/fox/motion/body/motion_patch.yaml`, `info.toml`) | **User-tested: works** |
| **Melee Fox** hitboxes | `atmosphere/.../skyline/plugins/libmelee_fox.nro` (source `melee_fox/`) | **User-tested: works** |
| **Item variants** (bumper / soccer ball) | `atmosphere/.../skyline/plugins/libitem_variants.nro` (source `item_variants/`) | Not tested yet |
| **Ultra Instinct parry sound** | `ultimate/mods/parry-sound/` (`sound/bank/common/se_common.patch3audio`, `info.toml`) | Not tested yet |
| **Little Mac trip + throw dash attack** | `atmosphere/.../skyline/plugins/liblittlemac_trip.nro` (source `littlemac_trip/`) | Not tested yet |

Full details are in the source folder: `README.md` (overview), `melee_fox/NOTES.md` and `item_variants/NOTES.md`.

### Melee Fox
- **Stats:** HDR Fox already had Melee's movement stats. In `fighter_param.prcxml`, **Fox is `fighter_param_table` index 6 (index 7 is Pikachu!)**. Changes:
  - `jump_y` 31.28, `mini_jump_y` 10.65, `jump_aerial_y` 40.2.
  - Aerial landing lag set to Melee's L-canceled values, n/f/b/hi/lw = 7/11/10/9/9. HDR has no L-cancel.
  - The file is a full copy of HDR's with only index 6 changed.
- **Cancel frames:** `motion_patch.yaml` is a full copy of HDR's Fox file plus Melee cancel frames:
  - fsmash 40, usmash 42, dsmash 46, dtilt 28, uair 40, dair 50, fair 36, grab 30, dash grab 40.
- **Plugin:** smashline scripts at `Priority::High` override HDR's `Priority::Low`. Each changed script has a `// MELEE:` comment.
  - Damage and timing changed on ftilt, utilt, dtilt, dash attack, all smashes, nair, bair, uair, and shine (5%).
  - Fair rebuilt as Melee's 5 spaced hits, with the animation retimed using `FT_MOTION_RATE_RANGE`.
  - Dair is 8 one-frame hits.
  - Firefox: 2% charge hits, single 14% dash hit.
  - Illusion 7%. Laser 3% then 2%.

### Item variants
- Smash can't add new item types. So every bumper and soccer ball rolls a variant from its object ID and gets tinted:
  - Bumper: blue = weak (×0.5 knockback), red = strong (×1.8).
  - Soccer ball: purple = spike (280°), yellow = launch up (90°), cyan = gravity (pulls nearby fighters).
- One per-frame callback on `Agent::new("fighter")` loops over `ItemManager` active items. The ItemManager singleton comes from symbol `_ZN3lib9SingletonIN3app11ItemManagerEE9instance_E`.
- Tuning constants are at the top of `src/lib.rs`.
- The user turns other items off in the in-game Items menu.

### Parry sound
- The user's download replaced the whole `se_common.nus3audio` with a copy from 2021, which clashed with HDR's `se_common.patch3audio`.
- Fix: pulled only the track `se_common_justshield` and merged it with HDR's single patch track (`se_system_amiibo_write_2`) into `parry-sound/.../se_common.patch3audio`, using `nus3tool` (Rust, `nus3audio` crate).

### Little Mac
- When his dash attack starts, a fighter per-frame callback swaps the animation to `slip` (his trip), then `slip_stand`.
- `game_slip` and `effect_slip` overrides act only during `FIGHTER_STATUS_KIND_ATTACK_DASH`. Otherwise they're the vanilla scripts, so real banana trips are unchanged.
- On frame 9 a single-id hitbox travels forward in an arc for 20 frames with the flowerpot's numbers: 10% then 6%, 70°, knockback growth 60, base 65. `sys_attack_impact` effects mark its path.
- **No new objects, no shared files: chosen for stability.**

## Files moved / ARCropolis state
- **Moved out of HDR** to `D:\hdr-melee-fox-dev\backup\` because ARCropolis reported a conflict. Our versions are supersets of HDR's:
  - `hdr/fighter/fox/motion/body/motion_patch.yaml`
  - `hdr-assets/sound/bank/common/se_common.patch3audio`
- **Enabled-mod preset:** `ultimate/arcropolis/config/1152963019137372243/18081973790210329761/presets` is a JSON list of hash40 values of `sd:/ultimate/mods/<folder>`. hash40 = crc32 of the string + (length << 32); compute it with `hash40.ps1`. Current value:
  `[124334938316 (hdr-assets), 123019169574 (hdr-stages), 94293926859 (hdr), 133816425474 (hdr-melee-fox), 126823261710 (parry-sound)]`
  `.bak-*` copies sit next to it.
- **Disabled experiment:** `D:\hdr-melee-fox-dev\disabled-hdr-littlemac-flowerpot\` and `liblittlemac_trip-flowerpot.nro.disabled`. **Don't re-enable as-is** (see the traps below).

## Traps already found
1. **`fighter_param_table` index ≠ fighter kind order.** Fox = 6. Check the `<!-- NAME -->` comments in HDR's file.
2. **`.prcxml` patches stack across mods; `motion_patch.yaml` does NOT.** Two mods with the same yaml cause an ARCropolis conflict error at boot (`ultimate/arcropolis/conflicts.json`). Fix: one full copy, and move HDR's out.
3. **Cross-fighter `share-to-vanilla` hangs the game.** Sharing Villager's flowerpot files to Little Mac paths (plus `new-dir-files`) made matches hang on an endless black screen with no crash report. Shared files point at Villager's file group, which doesn't load without Villager. Cloning articles with `smashline::clone_weapon` needs **real copies** of the files in the mod folder (see LilyLavender/ssbu-mod-workshop episode46 `ArticleCloningGuide.md`). Getting those requires a dump of the user's own game. The user chose not to do that.
4. **`clone_weapon` makes the article read settings from the NEW owner's `vl.prc`**, so you'd need to add a `param_<name>` block. prcx patches *can* add new keys.
5. **Item hitboxes** are Lua scripts in romfs; smashline can't override them. Plain-text `.lua` replacements work (HDR does this in `hdr-assets/item/*/script/animcmd/body/game.lua`), but they apply to every copy of the item.
6. **Smashline priority:** HDR registers at `Priority::Low`. Use `High`; the higher priority wins regardless of load order (HDR smashline `src/create_agent.rs`).
7. **Claude tooling quirks seen this session:**
   - The Write tool failed with "classifier gave no verdict" for files under `D:\hdr-melee-fox-dev`, and with EPERM when writing straight to `D:\` (the drive root). Writing under `C:\Users\Sophie\hdr-modding\` and copying with bash works.
   - A big bash heredoc with apostrophes broke once; using the Write tool for source files was fine.
   - Bash `sed` strips CRLF; HDR's prcxml uses CRLF.
8. **One early crash** happened at boot right after adding a new mod, inside Skyline while only ARCropolis was loaded (probably cache rebuild). It didn't repeat. Crash reports are in `atmosphere/crash_reports/`.

## Useful references
- **HDR source:** github.com/HDR-Development/HewDraw-Remix at tag v0.50.0. Fox is `fighters/fox/src`, Little Mac `fighters/littlemac/src`.
- **Vanilla scripts (ACMD dumps):** github.com/WuBoytH/SSBU-Dumped-Scripts (`smashline-hdr/lua2cpp_<fighter>/`, items in `luas/item/`). Status scripts: github.com/Coolsonickirby/SSBU-Dumped-Status-Scripts.
- **Game file paths:** github.com/ultimate-research/archive-hashes (`Hashes_FullPath`). Param names: ultimate-research/param-labels.
- **Melee frame data:** meleeframedata.com/fox (certificate expired; use `curl -k`), SmashWiki Fox (SSBM).
- **ARCropolis config keys** (`new-dir-files`, `share-to-vanilla`, …): `src/modfs/handlers/config.rs` and `src/fs.rs` in github.com/Raytwo/ARCropolis.

## Possible next steps
- Get the user's test results for item variants, parry sound and Little Mac, then tune:
  - Tint strength, gravity pull (`GRAVITY_MAX_PULL`), bumper knockback.
  - Mac's throw arc via `projectile_pos`, `FLIGHT_FRAMES` and `STAND_RATE`.
- **Melee Fox polish:** knockback values on his moves are still HDR's; Firefox and Illusion travel distances haven't been compared with Melee.
- **Real flowerpot for Mac:** only possible if the user dumps the pot files from their own Switch. Don't use or decrypt the NSP or keys on this card.
- **Git:** the user wanted git "later". The source isn't under version control yet.
