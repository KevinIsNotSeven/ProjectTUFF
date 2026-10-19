"""Installs a folder of downloaded ARCropolis skins onto the Switch SD card.

Usage: py install_skins.py <skins folder> <SD root> [--hashes Hashes_FullPath.txt] [--apply]
       py install_skins.py --shrink <mod folder>     (shrink oversized textures in place)

Without --apply it only prints what it would do.

Each skin becomes its own mod folder, ultimate/mods/skin-<name>. Separate folders let ARCropolis
merge the files that are meant to stack (ui_chara_db .prcxml/.prcx and msg_name.xmsbt) instead
of treating them as conflicts.

Mods already on the SD card win every conflict, then skins this script installed on an earlier
run (so adding skins never bumps one you already have), then new skins alphabetically. Conflicts
are checked per costume slot, not per file: if an installed mod already has a model body or
portraits for, say, Fox c07, every file a new skin has for Fox c07 is skipped (model, sounds,
effects, portraits). Otherwise you'd get one skin's model with another skin's voice. A skin that
loses a slot also loses its name patches (msg_name.xmsbt, ui_chara_db), which would otherwise
rename the winner. RELOCATE below moves a skin to another slot by hand instead.
Folders in the skins folder whose names start with "_" (e.g. "_ModZips") are ignored.

Cleanup done on the way:
- drops macOS/Windows junk, readmes, images, .blend files, and nested copies of the mod;
- drops files that replace something for the whole game rather than one costume: music
  ("stream"/"stream;"), movies ("prebuilt;"), full ui_chara_db.prc / soundlabelinfo.sli copies,
  the announcer bank and UI layouts. Full copies come from whatever game version the author had
  and break other characters, HDR and other mods (the .prcxml/.xmsbt patch versions are kept);
- moves se_/vc_ sound banks that were put straight in fighter/ to sound/bank/;
- removes config.json "new-dir-files" entries for files that are neither in the mod nor in the
  game (a typo or a missing file there can stop ARCropolis from loading the mod);
- fixes portraits put in ui/replace(_patch)/chara_N/ instead of ui/replace(_patch)/chara/chara_N/;
- SHRINKS OVERSIZED TEXTURES. The game has fixed memory budgets, and HD skins that blow past them
  crash it (an out-of-memory abort) when the file loads: picking the costume, scrolling to its
  portrait, or the results screen. Model/effect textures are capped at 2048 px (the game's own
  largest), and the big select-screen portraits (chara_3) at 3 megapixels (normal ones are 1.5-2.6). Needs
  ClaudeTools/ultimate_tex and ffmpeg (paths below).

Installed skins are recorded in <SD>/hdr-melee-fox-dev/skins_manifest.json. A re-run updates
those folders and leaves every other mod alone. New folders are turned on in the ARCropolis preset.
"""
import json, os, re, shutil, sys, hashlib, zlib, struct, subprocess, tempfile, math

TOOLS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "..", "ClaudeTools")
TEX = os.path.join(TOOLS, "ultimate_tex", "ultimate_tex_cli.exe")
FFMPEG = os.path.join(TOOLS, "ffmpeg", "ffmpeg.exe")
MAX_SIDE = 2048
MAX_PORTRAIT_PIXELS = 3_000_000
# DXGI format -> ultimate_tex format name, for re-encoding in the original format.
DXGI = {98: "BC7RgbaUnorm", 99: "BC7RgbaUnormSrgb", 71: "BC1RgbaUnorm", 72: "BC1RgbaUnormSrgb",
        77: "BC3RgbaUnorm", 78: "BC3RgbaUnormSrgb", 80: "BC4RUnorm", 83: "BC5RgUnorm",
        28: "R8G8B8A8Unorm", 29: "R8G8B8A8UnormSrgb"}
_tmp = tempfile.mkdtemp(prefix="skin_shrink_")


def tex_info(path):
    """(width, height, mip count, dxgi format) of a .nutexb/.bntx, or None if unreadable."""
    dds = os.path.join(_tmp, "info.dds")
    if subprocess.run([TEX, path, dds], capture_output=True).returncode:
        return None
    d = open(dds, "rb").read(148)
    h, w = struct.unpack_from("<II", d, 12)
    mips = max(1, struct.unpack_from("<I", d, 28)[0])
    fmt = struct.unpack_from("<I", d, 128)[0] if d[84:88] == b"DX10" else None
    return w, h, mips, fmt


def shrink_target(rel, info):
    """New (width, height) if this texture is too big for the game, else None."""
    w, h = info[0], info[1]
    scale = min(1.0, MAX_SIDE / max(w, h))
    if re.search(r"/chara_3_[^/]+\.bntx$", rel):
        scale = min(scale, math.sqrt(MAX_PORTRAIT_PIXELS / (w * h)))
    if scale >= 1.0:
        return None
    return max(4, int(w * scale) // 4 * 4), max(4, int(h * scale) // 4 * 4)


def shrink(src, dest, rel):
    """Writes a shrunk copy of src to dest. Returns (old, new) sizes, or None if src is fine."""
    if not rel.endswith((".nutexb", ".bntx")) or os.path.getsize(src) < 2_000_000:
        return None
    info = tex_info(src)
    if not info:
        return None
    size = shrink_target(rel, info)
    if not size:
        return None
    png, small = os.path.join(_tmp, "a.png"), os.path.join(_tmp, "b.png")
    out = os.path.join(_tmp, "out" + os.path.splitext(rel)[1])
    subprocess.run([TEX, src, png], check=True, capture_output=True)
    subprocess.run([FFMPEG, "-loglevel", "error", "-y", "-i", png, "-vf",
                    f"scale={size[0]}:{size[1]}:flags=lanczos", "-pix_fmt", "rgba", small], check=True)
    cmd = [TEX, "-f", DXGI.get(info[3], "BC7RgbaUnorm")]
    if info[2] == 1:
        cmd.append("--no-mipmaps")
    subprocess.run(cmd + [small, out], check=True, capture_output=True)
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    shutil.copyfile(out, dest)
    return (info[0], info[1]), size


def md5(path):
    with open(path, "rb") as f:
        return hashlib.md5(f.read()).hexdigest()


if sys.argv[1:2] == ["--shrink"]:
    root = sys.argv[2]
    for dp, _, fns in os.walk(root):
        for f in fns:
            p = os.path.join(dp, f)
            rel = os.path.relpath(p, root).replace("\\", "/")
            r = shrink(p, p, rel)
            if r:
                print(f"shrunk {rel}: {r[0][0]}x{r[0][1]} -> {r[1][0]}x{r[1][1]}")
    sys.exit(0)

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
# Hand-picked slot moves: skin folder -> {(fighter, from slot): to slot}.
RELOCATE = {
    # Same slot as Kool-Aid Man; every K. Rool costume uses the same model, so c05 is safe.
    "Hot Dog Vendor over K. Rool": {("krool", "04"): "05"},
}
GAME_DIRS = {"fighter", "ui", "sound", "param", "effect", "camera", "item", "standard"}
# Whole-game replacements (see the docstring).
GLOBAL = re.compile(r"^(ui/param/.*\.prc$|sound/param/|sound/bank/narration/|ui/layout/)")
MOD_FILES = {"config.json", "info.toml", "preview.webp", "preview.png", "preview.jpg"}
JUNK = {".ds_store", "desktop.ini", "thumbs.db"}
# These patch formats merge across mods, so two skins having them is fine.
MERGEABLE = re.compile(r"^((ui/)?param/.*\.(prcxml|prcx)|ui/message/.*\.xmsbt)$")
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


def relocate(rel, moves):
    """rel with the costume slot changed per moves {(fighter, from): to}."""
    for (ch, a), b in moves.items():
        rel = re.sub(rf"^(fighter/{ch}/.+/)c{a}(/)", rf"\g<1>c{b}\2", rel)
        rel = re.sub(rf"^(fighter/{ch}/)c{a}$", rf"\g<1>c{b}", rel)  # config.json dir keys
        rel = re.sub(rf"^(ui/replace(?:_patch)?/chara/chara_\d+/chara_\d+_{ch}_){a}(\.bntx)$", rf"\g<1>{b}\2", rel)
        rel = re.sub(rf"^(sound/bank/fighter(?:_voice)?/(?:se|vc)_{ch}_)c{a}", rf"\g<1>c{b}", rel)
        rel = re.sub(rf"^(effect/fighter/{ch}/ef_{ch}_)c{a}(\.eff)$", rf"\g<1>c{b}\2", rel)
    return rel


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
        if (name, rel) in EXCLUDE or top not in GAME_DIRS or GLOBAL.match(rel.lower()):
            dropped.append(rel)
            continue
        rel = re.sub(r"^ui/(replace|replace_patch)/(chara_\d+/)", r"ui/\1/chara/\2", rel)
        m = re.match(r"fighter/((se|vc)_[a-z0-9_]+\.nus3(audio|bank))$", rel)
        if m:  # sound bank put straight in fighter/
            rel = ("sound/bank/fighter/" if m.group(2) == "se" else "sound/bank/fighter_voice/") + m.group(1)
        out[relocate(rel, RELOCATE.get(name, {}))] = src
    return out, dropped


def hash40(s):
    return zlib.crc32(s.encode()) + (len(s) << 32)


def same(a, b):
    if os.path.getsize(a) != os.path.getsize(b):
        return False
    with open(a, "rb") as fa, open(b, "rb") as fb:
        return hashlib.md5(fa.read()).digest() == hashlib.md5(fb.read()).digest()


manifest = json.load(open(MANIFEST)) if os.path.exists(MANIFEST) else {}
if "skins" not in manifest:  # first version of the manifest was just {skin: folder}
    manifest = {"skins": manifest, "shrunk": {}}
shrunk = manifest["shrunk"]  # "<folder>/<rel>" -> md5 of the source it was shrunk from
managed = set(manifest["skins"].values())

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
known = list(manifest["skins"])
names = [n for n in os.listdir(SRC) if os.path.isdir(os.path.join(SRC, n)) and not n.startswith("_")]
names.sort(key=lambda n: (known.index(n) if n in known else len(known), n.lower()))
for name in names:
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
        if lost and MERGEABLE.match(rel.lower()):
            notes.append(f"skipped {rel}: it would rename the slot's other skin")
            continue
        owner = taken_files.get(rel.lower())
        if owner and not MERGEABLE.match(rel.lower()) and rel not in MOD_FILES:
            notes.append(f"skipped file {rel}: already in {owner}")
            continue
        keep[rel] = src
    if not any(r not in MOD_FILES for r in keep):
        report.append((name, None, notes + ["NOT INSTALLED: nothing left after conflicts"]))
        continue
    folder = manifest["skins"].get(name, slug(name))
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
    moves = RELOCATE.get(next((n for n, (f, k) in plan.items() if k is keep), None), {})
    if moves and isinstance(cfg.get("new-dir-files"), dict):
        cfg["new-dir-files"] = {relocate(d, moves): [relocate(f, moves) for f in lst]
                                for d, lst in cfg["new-dir-files"].items()}
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
        for rel, src in keep.items():
            if rel.endswith((".nutexb", ".bntx")) and os.path.getsize(src) >= 2_000_000:
                info = tex_info(src)
                size = info and shrink_target(rel, info)
                if size:
                    notes.append(f"will shrink {rel}: {info[0]}x{info[1]} -> {size[0]}x{size[1]}")
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
        key = f"{folder}/{rel}"
        if os.path.exists(dest) and key in shrunk and shrunk[key] == md5(src):
            continue  # already shrunk from this exact source
        r = shrink(src, dest, rel)
        if r:
            shrunk[key] = md5(src)
            print(f"shrunk {folder}/{rel}: {r[0][0]}x{r[0][1]} -> {r[1][0]}x{r[1][1]}")
            copied += 1
            continue
        shrunk.pop(key, None)
        if os.path.exists(dest) and same(src, dest):
            continue
        shutil.copyfile(src, dest)
        copied += 1
    manifest["skins"][name] = folder
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
