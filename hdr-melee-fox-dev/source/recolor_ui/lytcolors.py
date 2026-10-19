"""Finds (and optionally recolors) colors inside Smash Ultimate UI layout.arc files.

Library used by recolor_ui.py. Handles SARC archives with BFLYT (v8/v9) layouts and BFLAN
animations:
  BFLYT mat1  material black/white colors (RGBA8)
  BFLYT pic1  4 vertex colors
  BFLYT txt1  font top/bottom colors and shadow top/bottom colors
  BFLYT wnd1  window content vertex colors
  BFLAN pai1  FLMC (material color) and FLVC (vertex color) keyframes
Everything is edited in place (same sizes), so offsets never move.
"""
import struct, colorsys


def sarc_files(d):
    """[(name, start, end)] of a SARC (uncompressed)."""
    assert d[:4] == b"SARC", "not a SARC"
    hdr_len, bom, size, data_start = struct.unpack_from("<HHII", d, 4)
    sfat = hdr_len
    assert d[sfat:sfat + 4] == b"SFAT"
    _, count, _ = struct.unpack_from("<HHI", d, sfat + 4)
    nodes = sfat + 12
    sfnt = nodes + 16 * count
    names_start = sfnt + 8
    out = []
    for i in range(count):
        h, attr, s, e = struct.unpack_from("<IIII", d, nodes + 16 * i)
        name = ""
        if attr & 0x01000000:
            no = names_start + (attr & 0xFFFF) * 4
            name = d[no:d.index(b"\0", no)].decode()
        out.append((name, data_start + s, data_start + e))
    return out


class ColorRef:
    """One RGBA8 color at a byte offset (in the whole arc)."""
    def __init__(self, off, where):
        self.off, self.where = off, where


def bflyt_colors(d, base):
    """ColorRefs for every RGBA8 color in a BFLYT located at d[base:]."""
    refs = []
    magic, bom, hlen, ver, fsize, nsec, _ = struct.unpack_from("<4sHHIIHH", d, base)
    assert magic == b"FLYT"
    major = ver >> 24
    o = base + hlen
    mats = []
    for _ in range(nsec):
        mag, size = struct.unpack_from("<4sI", d, o)
        if mag == b"mat1":
            n = struct.unpack_from("<H", d, o + 8)[0]
            for k in range(n):
                m = o + struct.unpack_from("<I", d, o + 12 + 4 * k)[0]
                name = d[m:m + 0x1C].split(b"\0")[0].decode(errors="replace")
                mats.append(name)
                if major >= 8:
                    ctype = d[m + 0x20:m + 0x24]
                    if ctype != b"\x00\x02\x04\x08" and ctype[:2] != b"\x00\x02":
                        # unknown color encoding: leave this material alone
                        continue
                    c0 = m + 0x24
                else:
                    c0 = m + 0x1C
                refs.append(ColorRef(c0, f"mat {name} black"))
                refs.append(ColorRef(c0 + 4, f"mat {name} white"))
        elif mag in (b"pic1", b"txt1", b"wnd1"):
            pane = d[o + 0x10:o + 0x28].split(b"\0")[0].decode(errors="replace")
            p = o + 0x54  # end of the pane header
            if mag == b"pic1":
                for i, c in enumerate(("TL", "TR", "BL", "BR")):
                    refs.append(ColorRef(p + 4 * i, f"pic {pane} vtx {c}"))
            elif mag == b"txt1":
                refs.append(ColorRef(p + 20, f"txt {pane} top"))
                refs.append(ColorRef(p + 24, f"txt {pane} bottom"))
                refs.append(ColorRef(p + 64, f"txt {pane} shadow top"))
                refs.append(ColorRef(p + 68, f"txt {pane} shadow bottom"))
            else:
                content = o + struct.unpack_from("<I", d, p + 0x14)[0]
                for i, c in enumerate(("TL", "TR", "BL", "BR")):
                    refs.append(ColorRef(content + 4 * i, f"wnd {pane} vtx {c}"))
        o += size
    return refs


class AnimColor:
    """A color animated by up to 3 float tracks (R, G, B): lists of key value offsets per frame."""
    def __init__(self, where):
        self.where = where
        self.tracks = {}  # channel 0..2 -> [(frame, value_offset)]


def bflan_colors(d, base):
    """AnimColors for FLMC/FLVC color tracks in a BFLAN located at d[base:]."""
    out = []
    magic, bom, hlen, ver, fsize, nsec, _ = struct.unpack_from("<4sHHIIHH", d, base)
    assert magic == b"FLAN"
    o = base + hlen
    for _ in range(nsec):
        mag, size = struct.unpack_from("<4sI", d, o)
        if mag == b"pai1":
            frame_size, flags, _, ntex, nentries, entry_tbl = struct.unpack_from("<HBBHHI", d, o + 8)
            for e in range(nentries):
                eo = o + struct.unpack_from("<I", d, o + entry_tbl + 4 * e)[0]
                name = d[eo:eo + 0x1C].split(b"\0")[0].decode(errors="replace")
                ntags, is_mat = d[eo + 0x1C], d[eo + 0x1D]
                for t in range(ntags):
                    to = eo + struct.unpack_from("<I", d, eo + 0x20 + 4 * t)[0]
                    tag = d[to:to + 4]
                    if tag not in (b"FLMC", b"FLVC"):
                        continue
                    ngroups = d[to + 4]
                    colors = {}
                    for g in range(ngroups):
                        go = to + struct.unpack_from("<I", d, to + 8 + 4 * g)[0]
                        idx, target, curve, _, nkeys, _, keys = struct.unpack_from("<BBBBHHI", d, go)
                        if curve not in (1, 2):
                            continue
                        stride = 12 if curve == 2 else 8
                        if tag == b"FLMC":
                            color_id, ch = divmod(target, 4)      # 0 black, 1 white (RGBA each)
                            label = f"{name} mat {['black', 'white'][color_id] if color_id < 2 else color_id}"
                        else:
                            if target >= 16:
                                continue                          # pane alpha
                            color_id, ch = divmod(target, 4)
                            label = f"{name} vtx {['TL', 'TR', 'BL', 'BR'][color_id]}"
                        if ch == 3:
                            continue                              # alpha
                        ac = colors.setdefault(label, AnimColor(f"anim {label}"))
                        lst = []
                        for k in range(nkeys):
                            ko = go + keys + stride * k
                            frame = struct.unpack_from("<f", d, ko)[0]
                            lst.append((frame, ko + 4, curve))
                        ac.tracks[ch] = lst
                    out.extend(colors.values())
        o += size
    return out


def is_green(r, g, b, min_s=0.25):
    """Green-ish: hue 75-170 degrees with some saturation (min_s) and brightness."""
    h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
    return 75 <= h * 360 <= 170 and s > min_s and v > 0.12


def red_hue(h_deg):
    """Where a green hue goes: the green band 75-170 is squeezed onto reds 340-20 degrees."""
    return ((h_deg - 122.5) * 40 / 95) % 360


def toward(h_deg, target, w):
    """Hue moved fraction w of the way to target, along the shorter way round."""
    delta = (target - h_deg + 180) % 360 - 180
    return (h_deg + delta * w) % 360


def to_red(r, g, b, min_s=0.25):
    """A green color turned red, keeping saturation and brightness; other colors unchanged."""
    h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
    if not (75 <= h * 360 <= 170 and s > min_s and v > 0.12):
        return r, g, b
    nr, ng, nb = colorsys.hsv_to_rgb(red_hue(h * 360) / 360, s, v)
    return round(nr * 255), round(ng * 255), round(nb * 255)
