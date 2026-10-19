"""Turns HDR's green menu theme red.

Usage: py recolor_ui.py <hdr-assets mod folder> <vanilla ui dir> [--apply]

<vanilla ui dir> holds the game's own ui/layout/**/layout.arc files (pulled from your dump with
arc_extract). Only green that HDR added is changed: every color and texture is compared with the
game's original, and anything that's the same as vanilla is left alone. That keeps the game's own
meaningful greens (player 4, the green team, controller lights, Pokemon types, gauges...).

What gets recolored, in each HDR layout.arc:
- layout colors (material, vertex, text) and animated colors in the .bflyt/.bflan files;
- textures that HDR changed and that are at least MIN_GREEN green.
Green means hue 75-170 degrees with some saturation; that band is squeezed onto reds (340-20
degrees), keeping brightness and saturation, with a soft edge so anti-aliasing doesn't band.
Other colors stay.

Without --apply it only reports. With --apply the originals are copied to
<hdr-assets>/../../../hdr-melee-fox-dev/backup/recolor/ first (once).
"""
import sys, os, glob, struct, shutil, subprocess, tempfile, colorsys, re
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lytcolors import sarc_files, bflyt_colors, bflan_colors, is_green, to_red, red_hue, toward

TOOLS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "..", "ClaudeTools")
LAYOUT_TEX = os.path.join(TOOLS, "layout_tex", "target", "release", "layout_tex.exe")
TEX = os.path.join(TOOLS, "ultimate_tex", "ultimate_tex_cli.exe")
FFMPEG = os.path.join(TOOLS, "ffmpeg", "ffmpeg.exe")

MIN_GREEN = 0.05
# In-match HUD layouts whose colors mean something in gameplay.
SKIP_LAYOUTS = {"info_melee", "info_1on1"}
# Names whose green is a player/team color or otherwise meaningful, or not HDR's theme.
SKIP_NAME = re.compile(r"_4p|color4|player_color|team_color|indicator|title_front_hdr|Inkling", re.I)
# BNTX format -> ultimate_tex format name
FORMATS = {"0x2006": "BC7RgbaUnormSrgb", "0x2001": "BC7RgbaUnorm", "0x1c06": "BC3RgbaUnormSrgb",
           "0x1c01": "BC3RgbaUnorm", "0x1a06": "BC1RgbaUnormSrgb", "0x1a01": "BC1RgbaUnorm",
           "0x0b06": "R8G8B8A8UnormSrgb", "0x0b01": "R8G8B8A8Unorm"}

args = [a for a in sys.argv[1:] if not a.startswith("--")]
APPLY = "--apply" in sys.argv
HDR, VAN = args[0], args[1]
tmp = tempfile.mkdtemp(prefix="recolor_")


def static_map(d):
    """{(file, where, n): offset} for every layout color."""
    m, cnt = {}, {}
    for name, s, e in sarc_files(d):
        if name.endswith(".bflyt"):
            for r in bflyt_colors(d, s):
                k = (name, r.where)
                cnt[k] = cnt.get(k, 0) + 1
                m[(name, r.where, cnt[k])] = r.off
    return m


def read_key(d, vo, curve):
    return struct.unpack_from("<f", d, vo)[0] if curve == 2 else struct.unpack_from("<H", d, vo)[0]


def anim_map(d):
    """{(file, where): {frame: [(channel, offset, curve)]}}"""
    m = {}
    for name, s, e in sarc_files(d):
        if name.endswith(".bflan"):
            for ac in bflan_colors(d, s):
                frames = {}
                for ch, lst in ac.tracks.items():
                    for fr, vo, curve in lst:
                        frames.setdefault(fr, []).append((ch, vo, curve))
                m[(name, ac.where)] = frames
    return m


def anim_values(d, frames):
    out = {}
    for fr, keys in frames.items():
        rgb = [None] * 3
        for ch, vo, curve in keys:
            rgb[ch] = read_key(d, vo, curve)
        out[fr] = rgb
    return out


def recolor_colors(d, v, report):
    """Recolors HDR's green layout colors in d (bytearray) in place. v = vanilla arc bytes."""
    hs, vs = static_map(d), static_map(v)
    n = 0
    for k, off in hs.items():
        c = bytes(d[off:off + 4])
        if not is_green(*c[:3]) or SKIP_NAME.search(k[0] + k[1]):
            continue
        if k in vs and bytes(v[vs[k]:vs[k] + 4]) == c:
            continue  # vanilla's own green
        r, g, b = to_red(*c[:3])
        d[off:off + 3] = bytes((r, g, b))
        report.append(f"  color {k[0].split('/')[-1]} {k[1]}: #{c[:3].hex()} -> #{bytes((r, g, b)).hex()}")
        n += 1
    ha, va = anim_map(d), anim_map(v)
    for k, frames in ha.items():
        if SKIP_NAME.search(k[0] + k[1]):
            continue
        vals = anim_values(d, frames)
        if not any(None not in rgb and is_green(*rgb) for rgb in vals.values()):
            continue
        if k in va and anim_values(v, va[k]) == vals:
            continue
        for fr, keys in frames.items():
            rgb = vals[fr]
            if None in rgb or not is_green(*rgb):
                continue
            new = to_red(*[max(0, min(255, round(x))) for x in rgb])
            for ch, vo, curve in keys:
                if curve == 2:
                    struct.pack_into("<f", d, vo, float(new[ch]))
                else:
                    struct.pack_into("<H", d, vo, new[ch])
        report.append(f"  anim  {k[0].split('/')[-1]} {k[1]}")
        n += 1
    return n


def textures(arc):
    out = {}
    for l in subprocess.run([LAYOUT_TEX, "list", arc], capture_output=True, text=True).stdout.splitlines():
        p = l.split()
        out[p[0]] = {"fmt": p[1], "dims": p[2], "mips": int(p[4])}
    return out


def extract(arc, name, dds):
    subprocess.run([LAYOUT_TEX, "extract", arc, name, dds], capture_output=True, check=True)
    return open(dds, "rb").read()


def soft(x, lo0, lo1, hi1, hi0):
    """1 inside [lo1, hi1], fading to 0 at lo0 / hi0."""
    if x <= lo0 or x >= hi0:
        return 0.0
    if x < lo1:
        return (x - lo0) / (lo1 - lo0)
    if x > hi1:
        return (hi0 - x) / (hi0 - hi1)
    return 1.0


def recolor_pixels(raw):
    """Rotates green pixels toward red in an RGBA byte buffer. Returns (new buffer, green fraction)."""
    out = bytearray(raw)
    n = g = 0
    cache = {}
    for i in range(0, len(raw), 4):
        R, G, B, A = raw[i:i + 4]
        if A == 0:
            continue
        n += 1
        key = (R, G, B)
        res = cache.get(key)
        if res is None:
            h, s, v = colorsys.rgb_to_hsv(R / 255, G / 255, B / 255)
            w = soft(h * 360, 65, 75, 170, 180) * soft(s, 0.15, 0.25, 2, 3) * soft(v, 0.01, 0.04, 2, 3)
            if w > 0:
                h2 = toward(h * 360, red_hue(min(max(h * 360, 75), 170)), w) / 360
                nr, ng, nb = colorsys.hsv_to_rgb(h2, s, v)
                res = (round(nr * 255), round(ng * 255), round(nb * 255), w >= 0.5)
            else:
                res = (R, G, B, False)
            cache[key] = res
        out[i:i + 3] = bytes(res[:3])
        g += res[3]
    return bytes(out), g / max(n, 1)


def recolor_textures(arc_in, arc_out, varc, report):
    """Writes arc_out with HDR's green textures recolored. Returns how many were changed."""
    shutil.copyfile(arc_in, arc_out)
    hl, vl = textures(arc_in), textures(varc)
    n = 0
    for name, t in hl.items():
        fmt = FORMATS.get(t["fmt"])
        if not fmt or SKIP_NAME.search(name):
            continue
        dds = os.path.join(tmp, "h.dds")
        hd = extract(arc_out, name, dds)
        if name in vl and extract(varc, name, os.path.join(tmp, "v.dds")) == hd:
            continue  # vanilla texture
        png, raw = os.path.join(tmp, "h.png"), os.path.join(tmp, "h.raw")
        subprocess.run([TEX, dds, png], capture_output=True, check=True)
        w, h = map(int, t["dims"].split("x"))
        subprocess.run([FFMPEG, "-loglevel", "error", "-y", "-i", png, "-f", "rawvideo", "-pix_fmt", "rgba", raw], check=True)
        new, frac = recolor_pixels(open(raw, "rb").read())
        if frac < MIN_GREEN:
            continue
        open(raw, "wb").write(new)
        png2, dds2 = os.path.join(tmp, "n.png"), os.path.join(tmp, "n.dds")
        subprocess.run([FFMPEG, "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", f"{w}x{h}", "-i", raw, png2], check=True)
        cmd = [TEX, "-f", fmt] + (["--no-mipmaps"] if t["mips"] == 1 else []) + [png2, dds2]
        subprocess.run(cmd, capture_output=True, check=True)
        r = subprocess.run([LAYOUT_TEX, "replace", arc_out, name, dds2, arc_out], capture_output=True, text=True)
        if r.returncode:
            report.append(f"  texture {name}: FAILED ({r.stderr.strip()[:120]})")
            continue
        report.append(f"  texture {name} ({t['dims']}, {round(frac * 100)}% green)")
        n += 1
    return n


layout_root = os.path.join(HDR, "ui", "layout")
backup_root = os.path.normpath(os.path.join(HDR, "..", "..", "..", "hdr-melee-fox-dev", "backup", "recolor"))
for arc in sorted(glob.glob(os.path.join(layout_root, "*", "*", "*", "layout.arc"))):
    rel = os.path.relpath(arc, HDR).replace("\\", "/")
    lname = rel.split("/")[-2]
    if lname in SKIP_LAYOUTS:
        continue
    varc = os.path.join(VAN, rel)
    if not os.path.exists(varc):
        print(f"{lname}: no vanilla copy, skipped")
        continue
    report = []
    # If we already recolored this file, start again from the backed-up HDR original.
    src = os.path.join(backup_root, rel) if os.path.exists(os.path.join(backup_root, rel)) else arc
    work = os.path.join(tmp, "work.arc")
    ntex = recolor_textures(src, work, varc, report)
    d = bytearray(open(work, "rb").read())
    ncol = recolor_colors(d, open(varc, "rb").read(), report)
    print(f"{lname}: {ncol} colors, {ntex} textures")
    for r in report:
        print(r)
    if APPLY and (ncol or ntex):
        b = os.path.join(backup_root, rel)
        if not os.path.exists(b):
            os.makedirs(os.path.dirname(b), exist_ok=True)
            shutil.copyfile(arc, b)
        open(arc, "wb").write(d)
if not APPLY:
    print("\n(dry run: pass --apply to write)")
