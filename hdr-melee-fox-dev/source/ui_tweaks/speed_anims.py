"""Speeds up animations inside a Smash Ultimate UI layout.arc, in place (same file size).

Usage: py speed_anims.py <layout.arc> <speed> <anim name>... [--apply]
  speed 2.0 = twice as fast. Anim names are the BFLAN names without "anim/" and ".bflan".
  Without --apply it only prints what would change. With --apply the original is copied to
  <layout.arc>.before-speed (once) and the file is rewritten.

Example (the pre-match VS screen: the 3-second "VS" intro and the loading-done transition):
  py speed_anims.py D:/ultimate/mods/hdr-assets/ui/layout/system/matchup/matchup/layout.arc 2 ^
     matchup_anime_seq_03_00 matchup_anime_seq_02_00 matchup_anime_seq_02_01 --apply

Each BFLAN's frame count, every keyframe time and every Hermite slope are scaled, so the
animation looks the same, just shorter. The pat1 tag's timeline end is moved to match.
"""
import os, shutil, struct, sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "recolor_ui"))
from lytcolors import sarc_files  # noqa: E402


def speed_bflan(d, base, speed):
    magic, bom, hlen, ver, fsize, nsec, _ = struct.unpack_from("<4sHHIIHH", d, base)
    assert magic == b"FLAN"
    o = base + hlen
    keys = 0
    for _ in range(nsec):
        mag, size = struct.unpack_from("<4sI", d, o)
        if mag == b"pat1":
            start, end = struct.unpack_from("<HH", d, o + 0x18)
            if end > start:
                struct.pack_into("<H", d, o + 0x1A, start + round((end - start) / speed))
        elif mag == b"pai1":
            frames = struct.unpack_from("<H", d, o + 8)[0]
            struct.pack_into("<H", d, o + 8, round(frames / speed))
            ntex, nentries, entry_tbl = struct.unpack_from("<HHI", d, o + 12)
            for e in range(nentries):
                eo = o + struct.unpack_from("<I", d, o + entry_tbl + 4 * e)[0]
                for t in range(d[eo + 0x1C]):
                    to = eo + struct.unpack_from("<I", d, eo + 0x20 + 4 * t)[0]
                    for g in range(d[to + 4]):
                        go = to + struct.unpack_from("<I", d, to + 8 + 4 * g)[0]
                        idx, target, curve, _, nkeys, _, koff = struct.unpack_from("<BBBBHHI", d, go)
                        stride = 12 if curve == 2 else 8
                        for k in range(nkeys):
                            ko = go + koff + stride * k
                            f = struct.unpack_from("<f", d, ko)[0]
                            struct.pack_into("<f", d, ko, f / speed)
                            if curve == 2:  # Hermite: frame, value, slope (per frame)
                                s = struct.unpack_from("<f", d, ko + 8)[0]
                                struct.pack_into("<f", d, ko + 8, s * speed)
                            keys += 1
            print(f"  {frames} -> {round(frames / speed)} frames, {keys} keyframes")
        o += size


def main():
    args = [a for a in sys.argv[1:] if a != "--apply"]
    apply = "--apply" in sys.argv
    path, speed, names = args[0], float(args[1]), args[2:]
    d = bytearray(open(path, "rb").read())
    files = {n: (s, e) for n, s, e in sarc_files(d)}
    for n in names:
        key = f"anim/{n}.bflan"
        if key not in files:
            sys.exit(f"no {key} in {path}")
        print(n)
        speed_bflan(d, files[key][0], speed)
    if apply:
        bak = path + ".before-speed"
        if not os.path.exists(bak):
            shutil.copy2(path, bak)
        open(path, "wb").write(d)
        print(f"wrote {path} (original: {bak})")
    else:
        print("dry run; add --apply to write")


if __name__ == "__main__":
    main()
