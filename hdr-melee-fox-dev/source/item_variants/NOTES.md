# Item variants: notes

Spicier bumpers and soccer balls.

**On the SD card:** `sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libitem_variants.nro`. Delete that file to turn the variants off.

## How it works
Smash can't add new item types, so this mod gives the existing items random variants instead. Whenever a bumper or soccer ball spawns, it rolls one (all variants have equal odds) and shows that variant's own texture so you can tell which is which. The roll is based on the item's ID, so an item keeps its variant for its whole life.

| Item | Variant | Look | Effect |
|---|---|---|---|
| Bumper | normal | **NormalBumper** texture | unchanged, custom bounce sound |
| Bumper | weak | **SpongeBumper** texture | knockback ×0.5, WeakBumper sound |
| Bumper | strong | **TuffBumper** texture (troll face) | knockback ×1.5, Bumper sound |
| Soccer ball | normal | **NormalSoccer** texture | unchanged |
| Soccer ball | spike | **SpikeSoccer** texture (purple) | launches you down (280°) |
| Soccer ball | launch | **LaunchSoccer** texture (yellow) | launches you straight up (90°) |
| Soccer ball | gravity | **GravitySoccer** texture (cyan) | pulls nearby fighters toward it while it's out (radius 40, stronger up close; grounded fighters slide but aren't lifted) |

Variants are rolled randomly (system-clock xorshift) the first time the plugin sees an item, and remembered per object id; entries unseen for 1 s expire because ids restart every match.

**2026-10-18 update (3 bumper skins):** the model now has `normal_model`, `weak_model`, `tuff_model` (`bumper_model build <stock> <out> weak=bumper_weak_col tuff=bumper_tuff_col`; `bumper_col.nutexb` itself is replaced with NormalBumper). Sounds: stock `se_item_bumper_repel` is replaced with 0.5 s of silence; the plugin detects it starting (rising edge, per item) and plays `se_item_deathscythe_swing_l` (normal), `_swing_m` (weak) or `_reap` (strong). Sounds were boosted ~7 dB with a limiter (ffmpeg `volume=...,alimiter=limit=0.95`), about 4–6 dB louder on average than the stock bounce. A debug log still writes to `sd:/item_variants_log.txt` (`DEBUG_LOG`, capped at 300 lines).


**2026-10-19 update (soccer ball textures):** the soccer ball used to be tinted in code; it now works like the bumper. The model has `normal_`/`spike_`/`launch_`/`gravity_gamemodel` (`bumper_model build <stock> <out> spike=soccerball_spike_col launch=soccerball_launch_col gravity=soccerball_gravity_col`; mesh names are lowercased). The textures are edited in `Project TUFF\soccer-texture` (256x128 PNGs, BC7) and converted with its `convert-to-game.bat`. The starting PNGs are the stock texture multiplied by the old tint colors.

To play with only these, turn every other item off in the game's **Items** menu. Nothing else needs to be removed.

## Tuff bumper (texture + sound)
Added 2026-10-18. Needs both the plugin and the ARCropolis mod `sd:/ultimate/mods/hdr-item-variants/` (copy in `mod/`, enabled in the preset as hash 151701989782).
- **Model:** the game can't swap one item's texture at runtime, so the bumper model now has two copies of its mesh: `normal_model` (stock material) and `tuff_model` (material `ShaderfxShader2_tuff`, texture `bumper_tuff_col`). The plugin shows one per bumper with `ModelModule::set_mesh_visibility`. Built by `../bumper_model` from the stock files (`bumper_model build <stock c00> <out> bumper_tuff_col`). `model.nuanmb`'s material tracks are copied to the new material; its single "always visible" track was dropped.
- **Texture:** `bumper_tuff_col.nutexb` is a new file, registered under `new-dir-files` for the arc folder `item/bumper/model/body/c00`. Convert with `ultimate_tex_cli -f BC1RgbaUnormSrgb TuffBumper.png bumper_tuff_col.nutexb` (same format as stock `bumper_col`).
- **Sound:** the game can't load new sound names, so the custom sound replaces `se_item_deathscythe_reap` (Death's Scythe item, kept off) in `sound/bank/item/se_item.patch3audio`. Mono 48 kHz IDSP (`mod/bumper_tuff.idsp`, from VGAudioCli). Built with `nus3tool patch <stock se_item.nus3audio> <out> se_item_deathscythe_reap <idsp>`. When a strong bumper's stock `se_item_bumper_repel` is playing, the plugin stops it and plays the custom one.
- **Stock files** came from the user's own Switch dump (nxdt_rw_poc, 13.0.4 update), read with `../arc_extract` straight from the split parts.
- **Toolchain on this PC:** everything lives in `C:\Users\kevin\OneDrive\Desktop\GamesD\ClaudeTools` (Rust with RUSTUP_HOME/CARGO_HOME there, cargo-skyline, ultimate_tex, VGAudio, ffmpeg). `cargo skyline build` hangs on an interactive prompt without a terminal; the toolchain was installed with `cargo skyline update-std` plus `GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.longpaths GIT_CONFIG_VALUE_0=true`.

## Tuning
All the numbers are constants at the top of `src/lib.rs`: knockback multipliers, angles, gravity radius and strength, colors, and how many variants each item rolls. Change them, then rebuild:
```bash
cd ~/hdr-modding/item_variants
cargo skyline build --release
cp target/aarch64-skyline-switch/release/libitem_variants.nro /d/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/
```

## Untested / might need adjusting
- **Tint strength:** the color-blend tint might look too faint or too strong on these models.
- **First hit:** the spike and launch angles get re-applied every frame. If the game resets a ball's hitbox on the exact frame it's kicked, that one hit might use the normal angle.
- **Bumper strength:** the multiplier should scale both the hit when it's thrown and the bounce off a placed bumper. I haven't confirmed the second one.
- **Gravity:** a pull of 0.6 per frame is a guess. If it's too strong to escape, lower `GRAVITY_MAX_PULL`.
