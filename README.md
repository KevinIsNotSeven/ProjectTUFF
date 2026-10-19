# Project TUFF

A set of tweaks for **Super Smash Bros. Ultimate 13.0.4** layered on top of **[HDR](https://github.com/HDR-Development/HewDraw-Remix) v0.50.0**. It runs on a modded Switch with Atmosphere, Skyline and ARCropolis.

## What it does

| Feature | Where it lives |
|---|---|
| **Bumper variants.** Each bumper is randomly normal, weak (sponge) or strong ("tuff", 1.5× knockback), with its own texture and sound. Soccer balls are randomly normal, spike (launches down), launch (straight up) or gravity (strongly pulls fighters in, even off the ground), each with its own texture. | `hdr-melee-fox-dev/source/item_variants` (plugin + ARCropolis mod in `mod/`) |
| **K. Rool back to his base Ultimate moveset**, and Dedede's down tilt back to base Ultimate. | `hdr-melee-fox-dev/source/tuff_tweaks` |
| **Kirby** starts each stock with a random copy ability, and has Jigglypuff's air speed. | `tuff_tweaks` (copy ability); air speed is in `fighter_param.prcxml` on the SD card |
| **Bigger final hit.** The game-ending KO lasts 3 seconds, strobes red/blue, plays the bumper's KUNG slowed down, and the camera spins 360° around the hit (tune it, or turn the spin off, in `SD:\tuff_final_hit.txt`). | `tuff_tweaks` (`final_hit`); sound in `sd_install/ultimate/mods/parry-sound` |
| **Auto-tech toggle.** Hold shield and press a taunt: a ding and sparkle mean on, a thud means off. Each player has their own toggle. | `tuff_tweaks` |
| **Little Mac dash attack.** He trips and throws a hitbox forward, Villager-style, with his hoodie as the visible projectile. | `hdr-melee-fox-dev/source/littlemac_trip` |
| **Melee Fox.** Melee stats, cancel frames and hitboxes. | `hdr-melee-fox-dev/source/melee_fox` (hitboxes); the stats and cancel frames are in the `hdr-melee-fox` mod on the SD card |
| **Custom logos** replacing HDR's on the title screen, matchup screen and launcher. | Artwork in `custom logo/` |
| **Red menu theme.** HDR's green menus, Ready/GAME! text, loading spinner and launcher are red. Only green HDR added is changed; the game's own greens (player 4, green team, gauges) stay. | `hdr-melee-fox-dev/source/recolor_ui` |

More detail is in each folder's `NOTES.md` and in [hdr-melee-fox-dev/source/README.md](hdr-melee-fox-dev/source/README.md). [CLAUDE_HANDOFF.md](CLAUDE_HANDOFF.md) has the full development history.

## Installing
1. Put the Switch SD card in the PC.
2. Double-click **`Install to SD.bat`**.

The installer copies everything in `sd_install/`, which is laid out exactly like the SD card. It finds the SD card on its own and backs up any file it replaces to `SD:\hdr-melee-fox-dev\backup\installer-<date>\`. It also turns on the mods listed in `sd_install/enable_mods.txt` in the ARCropolis preset. Running it again is safe.

### Updating from the Switch (no PC needed)
Open **Project TUFF Updater** in the Homebrew Menu. It downloads the latest files in `sd_install/` from this GitHub repo, so you don't have to take the SD card out. It only downloads what changed, checks every download, and backs up anything it replaces to `SD:\hdr-melee-fox-dev\backup\updater-<date>\`. It also turns on new mods in ARCropolis. Close Smash and open it again afterwards to load the changes. The updater updates itself the same way. Its source is in `hdr-melee-fox-dev/source/updater` (C, libnx + curl). To build it, copy it to a folder without spaces and run `make` with devkitPro.

### Skins
Put downloaded skins (unzipped, one folder per skin) in `TuffCharacterSkins` at the root of the SD card. Then double-click **`Install Skins.bat`**. It shows what it will do and asks before installing.

Each skin becomes its own mod, `ultimate/mods/skin-<name>`. Skins already on the card keep their costume slots: a new skin for a slot that's already taken is skipped as a whole. That way you never end up with one skin's model and another skin's voice. The script also cleans out readmes and macOS junk, and removes config entries that point at missing files, since those can stop ARCropolis from loading a mod. It also **shrinks oversized textures**: model textures are capped at 2048 px, and the big select-screen portraits (`chara_3`) at 3 megapixels. HD skins over those limits run the game out of memory, and it crashes when the costume is picked, when its portrait comes up, or on the results screen. To shrink a mod that's already installed, run `py install_skins.py --shrink <mod folder>`. The rules are explained at the top of [install_skins.py](hdr-melee-fox-dev/source/skin_installer/install_skins.py).

## Folder layout
```
Install to SD.bat           runs sd_install/install.ps1
Install Skins.bat           installs SD:\TuffCharacterSkins (runs skin_installer/install_skins.py)
sd_install/                 files waiting to go onto the SD card (SD card layout)
hdr-melee-fox-dev/source/   source for every plugin and tool
  item_variants/              bumper / soccer ball variants (plugin + mod files)
  tuff_tweaks/                K. Rool & Dedede revert, Kirby copy, auto-tech
  littlemac_trip/             Little Mac dash attack
  melee_fox/                  Melee Fox hitboxes
  arc_extract/                pulls files out of a data.arc dump (also reads split dumps)
  layout_tex/                 swaps textures inside layout.arc (BNTX) files
  bumper_model/               builds the bumper model with extra texture variants
  nus3tool/                   patches and extracts tracks in .nus3audio / .patch3audio banks
  skin_installer/             installs a folder of skins with slot-conflict handling
  updater/                    the Switch updater app (Homebrew Menu)
  recolor_ui/                 turns HDR's green menu theme red (layouts + launcher)
bumper-texture/             bumper textures and sounds (custom/) + the original
soccer-texture/             soccer ball textures: one 2:1 world-map PNG per variant (run convert-to-game.bat)
custom logo/                logo artwork
```

## Building the plugins
The plugins are Rust and use [cargo-skyline](https://github.com/jam1garner/cargo-skyline) 3.5.0. They depend on HDR's fork of smashline, and their dependency versions are pinned in each `Cargo.lock` to match HDR v0.50.0.

```bash
cd hdr-melee-fox-dev/source/tuff_tweaks
cargo skyline build --release
```

The `.nro` file goes in `atmosphere/contents/01006A800016E000/romfs/skyline/plugins/` (copy it into `sd_install/` and run the installer).

Some things to know:
- Without a terminal, `cargo skyline build` can hang on a prompt. Install the toolchain first with `cargo skyline update-std`. On Windows you also need git long paths turned on.
- `tuff_tweaks/src/vanilla_acmd.rs` is generated by running `py gen_vanilla_acmd.py <dump tree json> <HDR fighters dir> src/vanilla_acmd.rs`. It contains vanilla move scripts taken from [WuBoytH/SSBU-Dumped-Scripts](https://github.com/WuBoytH/SSBU-Dumped-Scripts). The script's docstring explains it.
- **Status scripts can't be reverted the normal way.** smashline has no priority for status scripts, so the last plugin to register one wins, and HDR's plugin loads last. `tuff_tweaks` writes the game's own status functions back into the status table instead.

## Not included
- **Character skins** are made by other people, so they aren't in this repo. That covers the skins installed by `Install to SD.bat` (Melee Fox v2, 64 Fox, Fox 85622, Funky Kong, Groovy Lucas) and everything in `TuffCharacterSkins`. Download them from their original pages. The installers skip any skins that are missing.
- **Some files exist only on the SD card for now.** These are the `hdr-melee-fox` mod, the edited `fighter_param.prcxml`, the logo-patched `layout.arc`, `.eff` and launcher files, and the K. Rool file removals. They're described in [CLAUDE_HANDOFF.md](CLAUDE_HANDOFF.md) and the NOTES files.

## Contributing
Open an issue or a pull request. Test on a real Switch before you merge: most of this can only be checked in game.
