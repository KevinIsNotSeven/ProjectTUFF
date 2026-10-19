"""Turns the HDR launcher's green theme red, by patching the CSS colors inside hdr-launcher.nro.

Usage: py recolor_launcher.py <hdr-launcher.nro in> <out>

The launcher's web page is stored as plain text inside the .nro, so every green color (CSS
rgb(...) and #rrggbb forms) and the "lightgreen" color keyword are swapped in place for the same
number of bytes. Text like stage names ("Green Greens") is left alone.
"""
import re, sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lytcolors import is_green, to_red

# The launcher's pale mint hover border is only ~22% saturated, so count paler greens here.
MIN_S = 0.15

src, out = sys.argv[1], sys.argv[2]
d = bytearray(open(src, "rb").read())
n = 0

def rgb_sub(m):
    global n
    r, g, b = int(m[1]), int(m[2]), int(m[3])
    if not is_green(r, g, b, MIN_S):
        return m[0]
    nr, ng, nb = to_red(r, g, b, MIN_S)
    for sep in (b", ", b",", ):
        s = b"rgb(" + sep.join(str(x).encode() for x in (nr, ng, nb)) + b")"
        if len(s) <= len(m[0]):
            n += 1
            return s[:-1] + b" " * (len(m[0]) - len(s)) + b")"
    return m[0]

def hex_sub(m):
    global n
    h = m[1].decode()
    r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
    if not is_green(r, g, b, MIN_S):
        return m[0]
    n += 1
    return b"#" + ("%02x%02x%02x" % to_red(r, g, b, MIN_S)).encode()

d = re.sub(rb"rgb\((\d{1,3}), ?(\d{1,3}), ?(\d{1,3})\)", rgb_sub, bytes(d))
# CSS hex colors: '#' + 6 hex digits, ended by ; } ) , space or quote (not part of a longer word)
d = re.sub(rb"#([0-9a-fA-F]{6})(?=[;}\),\s\"'!])", hex_sub, d)
k = len(re.findall(rb"(['\"])lightgreen\1", d))
d = re.sub(rb"(['\"])lightgreen\1", rb"\1lightcoral\1", d)
assert len(d) == os.path.getsize(src)
open(out, "wb").write(d)
print(f"{n} green colors and {k} 'lightgreen' keywords changed -> {out}")
