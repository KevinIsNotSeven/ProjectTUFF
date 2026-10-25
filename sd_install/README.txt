Project TUFF - pending SD card changes

Everything in this folder is laid out exactly like the SD card. Two ways to install it:
  - On the Switch: open "Project TUFF Updater" in the Homebrew Menu (downloads this folder from
    GitHub; no PC needed). Then close and reopen Smash.
  - On a PC:
  1. Put the Switch SD card in the PC.
  2. Double-click "Install to SD.bat" in the Project TUFF folder.

The installer:
  - finds the SD card by itself (the drive with atmosphere\ and ultimate\mods\),
  - copies these files over, backing up any file it replaces to
    SD:\hdr-melee-fox-dev\backup\installer-<date>\,
  - turns on the mod folders listed in enable_mods.txt in the ARCropolis preset.
Running it again is safe: files that are already up to date are skipped.

Currently pending:
  - atmosphere\...\plugins\libtuff_tweaks.nro: fixes the K. Rool / Dedede crash (vanilla
    scripts are now compiled in), fixes K. Rool's up B T-pose and neutral B suction (HDR's
    versions were still winning), Kirby random copy, and auto-tech (toggle: hold shield +
    taunt; writes a debug log to SD:\tuff_tweaks_log.txt).
  - ultimate\mods\skins-fox: Fox c00-c03 Melee Fox v2, c04-c05 64 Fox, c06-c07 Fox (85622).
  - ultimate\mods\skin-funky-kong: Donkey Kong c05.
  - ultimate\mods\skin-groovy-lucas: Lucas c07.
  - atmosphere\...\plugins\libtuff_tweaks.nro also has the bigger final hit (3 s, red/blue strobe,
    camera orbit). Tune it in SD:\tuff_final_hit.txt (spin = 0 turns the orbit off).
  - ultimate\mods\parry-sound\...\se_common.patch3audio: final-hit sound = slowed bumper KUNG
    (keeps HDR's amiibo sound and the parry sound).
  - tuff_final_hit.txt: final-hit settings (goes to the SD root). Edit it on the SD card; the updater
    only replaces it when a new version is published here.
  - atmosphere\...\plugins\libtuff_tweaks.nro also has the Kirby down air spike and the Joker fun
    pack (no Arsene landing lag, faster/longer Arsene, "1 MORE!" smash-attack cancels, full-auto gun and down tilt: hold the button).
  - ultimate\mods\hdr-assets\ui\layout\info\info_1on1: SpongeBob instead of the hyphen on the
    1-on-1 stock popup. Replaces HDR's copy of that file (the old one is backed up).
  - libtuff_tweaks.nro also has Project M style Ivysaur (Synthesis / Solar Beam neutral B, seed
    down B, Pokemon switch on down taunt, better tether).
