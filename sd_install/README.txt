Project TUFF - pending SD card changes

Everything in this folder is laid out exactly like the SD card. To install:
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
    360-degree camera spin). If the spin misbehaves, create an empty file SD:	uff_no_spin.txt.
  - ultimate\mods\parry-sound\...\se_common.patch3audio: final-hit sound = slowed bumper KUNG
    (keeps HDR's amiibo sound and the parry sound).
