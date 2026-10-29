# Melee Fox for HDR: notes

This makes HDR's Fox play like Melee Fox. It's built for **HDR v0.50.0** (the version on the SD card) and is meant to stay on that version.

Melee data comes from meleeframedata.com/fox and SmashWiki's Fox (SSBM) page. HDR data comes from HewDraw-Remix at tag v0.50.0. The game's original scripts come from WuBoytH/SSBU-Dumped-Scripts.

## What's where

| Piece | Location | Turn off by |
|---|---|---|
| Stats (jumps, landing lag) | `sd:/ultimate/mods/hdr-melee-fox/fighter/common/param/fighter_param.prcxml` | disabling "HDR Melee Fox" in ARCropolis |
| Cancel frames (when moves can be interrupted) | `sd:/ultimate/mods/hdr-melee-fox/fighter/fox/motion/body/motion_patch.yaml` | same as above |
| Hitboxes (damage, timing) | `sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libmelee_fox.nro` | deleting that file |
| Plugin source | `C:\Users\Sophie\hdr-modding\melee_fox` | – |

- Both mod-folder files are **full copies of HDR's own files with only Fox changed**.
  - `fighter_param.prcxml` stacks fine with HDR's.
  - `motion_patch.yaml` did not stack: ARCropolis showed a conflict error at boot. To fix it, HDR's copy (`sd:/ultimate/mods/hdr/fighter/fox/motion/body/motion_patch.yaml`) was moved to `sd:/hdr-melee-fox-dev/backup/`. Ours has all of HDR's entries, so nothing is lost while this mod is on. If you ever disable it, put HDR's copy back.
- In `fighter_param`, Fox is **index 6**. Index 7 is Pikachu.
- The plugin registers its scripts at `Priority::High`. HDR uses `Priority::Low`, and smashline always keeps the higher one.

## Stats
HDR Fox already had Melee's numbers for weight, walk, dash, run, traction, jumpsquat, air speed, air accel, gravity, fall speed and fast fall.

| Stat | Melee | HDR | Now |
|---|---|---|---|
| Full hop (`jump_y`) | 31.28 | 35 (vanilla) | 31.28 |
| Short hop (`mini_jump_y`) | 10.65 | 12 | 10.65 |
| Double jump (`jump_aerial_y`) | 40.2 | 37 (vanilla) | 40.2 |
| Landing lag nair/fair/bair/uair/dair | 7/11/10/9/9 (L-canceled) | vanilla/15/9/10/10 | 7/11/10/9/9 |

HDR has no L-cancel, so I used Melee's L-canceled numbers.

## Cancel frames (`motion_patch.yaml`, in animation frames)
| Move | HDR | Now | Notes |
|---|---|---|---|
| Fsmash | 51 | 40 | Melee: 39 total frames |
| Usmash | – | 42 | Melee: 41 total. Comes out close to 41 because of HDR's animation speed changes |
| Dsmash | – | 46 | Melee: can act on frame 46 |
| Dtilt | 27 | 28 | Melee: can act on frame 28 |
| Uair | 36 | 40 | Melee: 39 total (Melee uair is slower) |
| Dair | 51 | 50 | Melee: 49 total |
| Fair | – | 36 | = game frame 53 after the fair retiming |
| Standing / dash grab | 36 / 44 | 30 / 40 | Melee: 29 / 39 total |

## Hitboxes (plugin)
| Move | Melee Fox (now) | HDR before |
|---|---|---|
| Ftilt (all 3) | 9% | 8% |
| Utilt | foot 12% / leg 9%, frames 5-11 | 8/6%, frames 5-10 |
| Dtilt | 10%, frames 7-9 | 9%, frames 7-8 |
| Dash attack | 7/5%, late hit to frame 17 | 6/4%, to frame 15 |
| Fsmash | strong 12-16, weak 17-22 (15/12%) | 13-14 / 15-16 |
| Usmash | 18/13%, weak hit to frame 17 | 16/12%, to frame 15 |
| Dsmash | frames 6-10 | 6-9 |
| Nair | 12/9% | 11/8% |
| Fair | 5 hits: 7/5/6/4/3% on frames 6, 16, 24, 33, 43 (2 frames each). Animation retimed to match, no float, no landing hit | 4×1.8% + 5.8% packed into frames 7-28, rises during the move |
| Bair | 15% strong hit | 13% |
| Uair | 5% on frames 8-9, 11% on frames 11-14, knockback growth 110 (nerfed 2026-10-28 from Melee's 13% / 125) | 3% on 9-10, 10% on 12-13 |
| Dair | 8 one-frame hits on frames 5, 8, 11 … 26. 3% for hits 1-4, 2% for hits 5-8 | 7 two-frame hits ×2% |
| Shine | 5% | 3% |
| Firefox | charge 7×2%, dash is a single 14% hit | 7×1.8%, then 16% → 10% |
| Illusion | 7% ground and air | vanilla 8% / 5% |
| Laser | 3% for the first 9 frames, then 2%, no flinch | vanilla 3 → 2 → 1.4% |
| Jab, throws | unchanged, already Melee-like | – |

### Guesses to check in testing
- **Fair:** hits 1-4 still use HDR's knockback values, so the higher damage pushes harder at high percent. Melee fair doesn't link reliably either. If it feels off, change knockback rather than damage.
- **Dair:** sources disagree. One says 8 hits, another says 7. I don't know exactly where Melee switches from 3% to 2%.
- **Utilt:** SmashWiki says 12% on grounded opponents and 9% otherwise. I simplified that to foot 12% / leg 9%.
- **`jump_initial_y`:** HDR sets it to 12.5125 and there's no documentation for it. If jump heights look wrong, look here first.

## Test checklist (training mode)
1. The game boots and Fox loads. If it crashes, delete `libmelee_fox.nro` and try again.
2. Nair does 12%, shine 5%, usmash 18% on the strong hit. If you still see HDR's numbers (11%, 3%, 16%), the plugin's overrides didn't take.
3. Short hop is noticeably lower than full hop.
4. Fair: 5 visible kicks with clear gaps between them, and Fox doesn't rise.
5. Dair: rapid small hits, and no hit on landing.
6. Firefox: 14% when the dash connects.
7. Lasers: 3% up close, no flinch.

## Building
```bash
cd ~/hdr-modding/melee_fox
cargo skyline build --release
cp target/aarch64-skyline-switch/release/libmelee_fox.nro /d/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/
```
Library versions are pinned in `Cargo.lock` to match HDR v0.50.0 (May 2026).

Every edited script has a `// MELEE:` comment saying what changed from HDR.
