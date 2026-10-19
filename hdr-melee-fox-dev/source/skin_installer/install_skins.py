"""Installs a folder of downloaded ARCropolis skins onto the Switch SD card.

Usage: py install_skins.py <skins folder> <SD root> [--hashes Hashes_FullPath.txt] [--apply]

Without --apply it only prints what it would do.

Each skin becomes its own mod folder, ultimate/mods/skin-<name>. Separate folders let ARCropolis
merge the files that are meant to stack (ui_chara_db .prcxml/.prcx and msg_name.xmsbt) instead
of treating them as conflicts.

Mods already on the SD card win every conflict. Conflicts are checked per costume slot, not per
file: if an installed mod already has a model body or portraits for, say, Fox c07, every file a
new skin has for Fox c07 is skipped (model, sounds, effects, portraits). Otherwise you'd get one
skin's model with another skin's voice. Among the new skins, the first one alphabetically wins.

Cleanup done on the way:
- drops macOS/Windows junk, readmes, images, .blend files, and nested copies of the mod;
- drops "stream" folders: they're whole-song replacements (not per skin), and without the
  ARCropolis "stream;" name they did nothing anyway;
- moves se_/vc_ sound banks that were put straight in fighter/ to sound/bank/;
- removes config.json "new-dir-files" entries for files that are neither in the mod nor in the
  game (a typo or a missing file there can stop ARCropolis from loading the mod).

Installed skins are recorded in <SD>/hdr-melee-fox-dev/skins_manifest.json. A re-run updates
those folders and leaves every other mod alone. New folders are turned on in the ARCropolis preset.
"""
import json, os, re, shutil, sys, hashlib, zlib

args = [a for a in sys.argv[1:] if not a.startswith("--")]
APPLY = "--apply" in sys.argv
HASHES = None
if "--hashes" in sys.argv:
    HASHES = sys.argv[sys.argv.index("--hashes") + 1]
    args.remove(HASHES)
SRC, SD = args[0], args[1]
MODS = os.path.join(SD, "ultimate", "mods")
MANIFEST = os.path.join(SD, "hdr-melee-fox-dev", "skins_manifest.json")

# Hand-picked exclusions: (skin folder, path inside it).
EXCLUDE = {
    # A Bayonetta c00 portrait that has nothing to do with this Ivysaur skin.
    ("Bulbasaur Gang", "ui/replace/chara/chara_1/chara_1_bayonetta_00.bntx"),
}
GAME_DIRS = {"fighter", "ui", "sound", "param", "effect", "camera", "item", "standard"}
MOD_FILES = {"config.json", "info.toml", "preview.webp", "preview.png", "preview.jpg"}
JUNK = {".ds_store", "desktop.ini", "thumbs.db"}
# These patch formats merge across mods, so two skins having them is fine.
MERGEABLE = re.compile(r"^(param/.*\.(prcxml|prcx)|ui/message/.*\.xmsbt)$")
UI_ALIAS = {"ice_climber": "popo", "nana": "popo"}


def slug(name):
    return "skin-" + re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")


def slot_of(rel):
    """(fighter, slot, claims) for a per-costume file, else None. claims=True for a model body
    or a portrait: the files that decide whose skin a slot is."""
    p = rel.split("/")
    m = re.match(r"fighter/([a-z0-9_]+)/.*?/c(\d\d)(/|$)", rel)
    if m:
        part = p[3] if len(p) > 3 and p[2] == "model" else p[2]
        return UI_ALIAS.get(m.group(1), m.group(1)), m.group(2), part == "body" and p[2] == "model"
    m = re.match(r"ui/replace(?:_patch)?/chara/chara_\d+/chara_\d+_([a-z0-9_]+?)_(\d\d)\.bntx$", rel)
    if m:
        return UI_ALIAS.get(m.group(1), m.group(1)), m.group(2), True
    m = re.match(r"sound/bank/fighter(?:_voice)?/(?:se|vc)_([a-z0-9_]+?)_c(\d\d)", rel)
    if m:
        return UI_ALIAS.get(m.group(1), m.group(1)), m.group(2), False
    m = re.match(r"effect/fighter/([a-z0-9_]+)/ef_[a-z0-9_]+?_c(\d\d)\.eff$", rel)
    if m:
        return UI_ALIAS.get(m.group(1), m.group(1)), m.group(2), False
    return None


def mod_files(root):
    """Game-relevant files of a mod folder: {relative path: source path}."""
    out = {}
    for dp, dns, fns in os.walk(root):
        dns[:] = [d for d in dns if d != "__MACOSX" and not d.startswith(".")]
        for f in fns:
            if f.lower() in JUNK or f.startswith("._"):
                continue
            src = os.path.join(dp, f)
            out[os.path.relpath(src, root).replace("\\", "/")] = src
    return out


def skin_files(name):
    root = os.path.join(SRC, name)
    out, dropped = {}, []
    for rel, src in mod_files(root).items():
        top = rel.split("/")[0]
        if "/" not in rel:
            if rel in MOD_FILES:
                out[rel] = src
            elif rel.lower() not in JUNK:
                dropped.append(rel)
            continue
        if (name, rel) in EXCLUDE or top not in GAME_DIRS:
            dropped.append(rel)
            continue
        m = re.match(r"fighter/((se|vc)_[a-z0-9_]+\.nus3(audio|bank))$", rel)
        if m:  # sound bank put straight in fighter/
            rel = ("sound/bank/fighter/" if m.group(2) == "se" else "sound/bank/fighter_voice/") + m.group(1)
        out[rel] = src
    return out, dropped


def hash40(s):
    return zlib.crc32(s.encode()) + (len(s) << 32)


def same(a, b):
    if os.path.getsize(a) != os.path.getsize(b):
        return False
    with open(a, "rb") as fa, open(b, "rb") as fb:
        return hashlib.md5(fa.read()).digest() == hashlib.md5(fb.read()).digest()


manifest = json.load(open(MANIFEST)) if os.path.exists(MANIFEST) else {}
managed = set(manifest.values())

# What's already installed (mods this script didn't make).
installed_files, installed_claims = {}, {}
for mod in sorted(os.listdir(MODS)):
    if mod in managed or not os.path.isdir(os.path.join(MODS, mod)):
        continue
    for rel in mod_files(os.path.join(MODS, mod)):
        installed_files.setdefault(rel.lower(), mod)
        s = slot_of(rel.lower())
        if s and s[2]:
            installed_claims.setdefault(s[:2], mod)

vanilla = None
if HASHES:
    vanilla = set(l.strip().lower() for l in open(HASHES, encoding="utf-8", errors="ignore"))

taken_files, taken_claims = dict(installed_files), dict(installed_claims)
plan, report = {}, []
for name in sorted(os.listdir(SRC), key=str.lower):
    if not os.path.isdir(os.path.join(SRC, name)):
        continue
    files, dropped = skin_files(name)
    notes = [f"dropped (not game files / excluded): {d}" for d in dropped]
    # whole slots that someone else already owns
    claims = {slot_of(r.lower())[:2] for r in files if slot_of(r.lower()) and slot_of(r.lower())[2]}
    lost = {c: taken_claims[c] for c in claims if c in taken_claims}
    for (ch, n), owner in sorted(lost.items()):
        notes.append(f"SKIPPED {ch} c{n}: already used by {owner}")
    keep = {}
    for rel, src in files.items():
        s = slot_of(rel.lower())
        if s and s[:2] in lost:
            continue
        owner = taken_files.get(rel.lower())
        if owner and not MERGEABLE.match(rel.lower()) and rel not in MOD_FILES:
            notes.append(f"skipped file {rel}: already in {owner}")
            continue
        keep[rel] = src
    if not any(r not in MOD_FILES for r in keep):
        report.append((name, None, notes + ["NOT INSTALLED: nothing left after conflicts"]))
        continue
    folder = manifest.get(name, slug(name))
    for rel in keep:
        if rel not in MOD_FILES:
            taken_files.setdefault(rel.lower(), folder)
    for c in claims:
        if c not in lost:
            taken_claims.setdefault(c, folder)
    plan[name] = (folder, keep)
    report.append((name, folder, notes))


def fix_config(src, keep):
    """config.json with new-dir-files entries pointing at nothing removed."""
    cfg = json.load(open(src, encoding="utf-8-sig"))
    have = {r.lower() for r in keep}
    removed = []
    ndf = cfg.get("new-dir-files")
    if isinstance(ndf, dict):
        for d, lst in list(ndf.items()):
            ok = [f for f in lst if f.lower() in have or (vanilla is not None and f.lower() in vanilla)]
            removed += [f for f in lst if f not in ok]
            if ok:
                ndf[d] = ok
            else:
                del ndf[d]
    return cfg, removed


for name, folder, notes in report:
    print(f"== {name} -> {folder or '-'}")
    if folder:
        keep = plan[name][1]
        if "config.json" in keep:
            if vanilla is None:
                notes.append("config.json kept as-is (pass --hashes to check its entries)")
            else:
                _, removed = fix_config(keep["config.json"], keep)
                for f in removed:
                    notes.append(f"config.json: removed entry for missing file {f}")
        print(f"   {len(keep)} files")
    for n in notes:
        print("   " + n)

if not APPLY:
    print("\n(dry run: pass --apply to install)")
    sys.exit(0)

copied = 0
for name, (folder, keep) in plan.items():
    dest_root = os.path.join(MODS, folder)
    # a re-run replaces the folder's contents with exactly the planned files
    if os.path.isdir(dest_root):
        for rel in list(mod_files(dest_root)):
            if rel not in keep:
                os.remove(os.path.join(dest_root, rel))
    for rel, src in keep.items():
        dest = os.path.join(dest_root, rel)
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        if rel == "config.json" and vanilla is not None:
            cfg, _ = fix_config(src, keep)
            text = json.dumps(cfg, indent=4)
            if not os.path.exists(dest) or open(dest, encoding="utf-8").read() != text:
                open(dest, "w", encoding="utf-8").write(text)
                copied += 1
            continue
        if os.path.exists(dest) and same(src, dest):
            continue
        shutil.copyfile(src, dest)
        copied += 1
    manifest[name] = folder
os.makedirs(os.path.dirname(MANIFEST), exist_ok=True)
json.dump(manifest, open(MANIFEST, "w"), indent=2)
print(f"\ncopied {copied} files into {len(plan)} skin folders")

# turn them on in every ARCropolis preset
cfg_root = os.path.join(SD, "ultimate", "arcropolis", "config")
for dp, _, fns in os.walk(cfg_root):
    if "presets" not in fns:
        continue
    p = os.path.join(dp, "presets")
    ids = json.load(open(p))
    new = [hash40(f"sd:/ultimate/mods/{f}") for f, _ in plan.values()]
    add = [h for h in new if h not in ids]
    if add:
        shutil.copyfile(p, p + ".bak-skins")
        json.dump(ids + add, open(p, "w"), separators=(",", ":"))
    print(f"preset {p}: turned on {len(add)} mods")
