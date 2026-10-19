# HDR mods: overview

Everything here is built for **HDR v0.50.0** (the version on the SD card).

| Mod | On the SD card | Turn off | Details |
|---|---|---|---|
| Melee Fox (stats + cancel frames) | `ultimate/mods/hdr-melee-fox/` | ARCropolis mod menu | [melee_fox/NOTES.md](melee_fox/NOTES.md) |
| Melee Fox (hitboxes) | `atmosphere/.../skyline/plugins/libmelee_fox.nro` | delete the file | [melee_fox/NOTES.md](melee_fox/NOTES.md) |
| Bumper / soccer ball variants | `atmosphere/.../skyline/plugins/libitem_variants.nro` | delete the file | [item_variants/NOTES.md](item_variants/NOTES.md) |
| Ultra Instinct parry sound | `ultimate/mods/parry-sound/` | ARCropolis mod menu | below |
| Little Mac trip + throw dash attack | `atmosphere/.../skyline/plugins/liblittlemac_trip.nro` | delete the file | below |

`atmosphere/...` = `atmosphere/contents/01006A800016E000/romfs`

## Files moved out of HDR
Two HDR files were moved to `sd:/hdr-melee-fox-dev/backup/` because our mods ship full replacements of them. Without the move, ARCropolis reports a conflict and only uses one of the two copies.

| HDR file | Replaced by | If you disable our mod |
|---|---|---|
| `hdr/fighter/fox/motion/body/motion_patch.yaml` | `hdr-melee-fox/...` (HDR's entries + Melee changes) | move HDR's copy back |
| `hdr-assets/sound/bank/common/se_common.patch3audio` | `parry-sound/...` (HDR's amiibo sound + parry sound) | move HDR's copy back |

## Parry sound
The original download replaced the whole `se_common.nus3audio`, the bank with every shared sound effect, using a copy from 2021. HDR only patches one sound in that bank (`se_system_amiibo_write_2`) with a `.patch3audio`, so the full replacement fought with HDR. That's most likely why it never worked right.

The fix: take only the parry track (`se_common_justshield`) out of the download and merge it with HDR's patch into `ultimate/mods/parry-sound/sound/bank/common/se_common.patch3audio`. Every other sound stays as it is in the current game. The mod is turned on in the ARCropolis preset.

## Little Mac trip + throw dash attack
Little Mac's dash attack is now Villager's in spirit: he trips, and on frame 9 (when Villager throws his flowerpot) a thrown hitbox leaves him and arcs forward. Source is in `littlemac_trip/src/lib.rs`.

- **Built for stability:** it creates no new objects, uses no shared files and doesn't clone anything. It's a hitbox on Mac that travels away from him, with built-in impact effects showing where it is.
- **Damage:** the flowerpot's numbers. 10% for the first 6 frames of flight, then 6%, at 70° (knockback growth 60, base 65). It travels for 20 frames, out to about 60 units in front of Mac. It can only hit each opponent once.
- **Animation:** Mac's own trip animation (`slip`), then his get-up animation (`slip_stand`) at 1.5× speed.
- **Limits:**
  - Because it's attached to Mac, it vanishes if Mac gets hit out of the move.
  - It can't be reflected or absorbed.
  - There's no visible pot or hoodie model.
- **Real trips** (bananas, etc.) don't throw anything.
- **Tuning:** constants at the top of `lib.rs`: `STAND_RATE`, `THROW_FRAME`, `FLIGHT_FRAMES`, `STRONG_FRAMES`, and `projectile_pos` for the arc.

### What we tried first (and why it's off)
Cloning Villager's real flowerpot onto Mac made matches hang on a black screen. The pot's files were shared from Villager's folder, and those only load when Villager is in the match. Real copies of the files would fix that, but they have to be dumped from your own Switch. The disabled attempt is kept in `sd:/hdr-melee-fox-dev/` (`disabled-hdr-littlemac-flowerpot/` and `liblittlemac_trip-flowerpot.nro.disabled`). Don't move it back as-is.
