"""Puts a picture in place of the hyphen on the 1-on-1 stock score popup ("3 - 2").

Usage: py score_picture.py <picture.png> <HDR info_1on1 layout.arc> <out layout.arc> [size]
  size = on-screen size of the square picture (default 200; the digits are 200x256).

HDR's file is ultimate/mods/hdr-assets/ui/layout/info/info_1on1/info_1on1/layout.arc.
The hyphen texture slot is 134x70, but its data space (18432 bytes) also fits a 128x128 BC7
image, so the texture header is changed to 128x128 and the hyphen pane is made square.
The picture gets rounded corners. Needs ffmpeg, ultimate_tex_cli and layout_tex from ClaudeTools.
"""
import os, struct, subprocess, sys, tempfile

TOOLS = os.path.join(os.path.dirname(os.path.abspath(__file__)), *[".."] * 5, "ClaudeTools")
FFMPEG = os.path.join(TOOLS, "ffmpeg", "ffmpeg.exe")
TEX = os.path.join(TOOLS, "ultimate_tex", "ultimate_tex_cli.exe")
LAYOUT = os.path.join(TOOLS, "layout_tex", "target", "release", "layout_tex.exe")
TEXTURE = "info_1on1_localize_hyphen^u"


def patch_headers(d, size):
    b = d.find(b"BNTX")
    i = d.find(b"BRTI", b)
    done = False
    while i >= 0:
        no = b + struct.unpack_from("<Q", d, i + 0x60)[0]
        name = d[no + 2:no + 2 + struct.unpack_from("<H", d, no)[0]].decode()
        if name == TEXTURE:
            if struct.unpack_from("<II", d, i + 0x24) == (134, 70):
                struct.pack_into("<II", d, i + 0x24, 128, 128)
            assert struct.unpack_from("<II", d, i + 0x24) == (128, 128)
            done = True
        i = d.find(b"BRTI", i + 4)
    assert done, "hyphen texture not found"
    f = d.find(b"FLYT")
    off = struct.unpack_from("<H", d, f + 6)[0]
    n = struct.unpack_from("<H", d, f + 0x10)[0]
    p = f + off
    for _ in range(n):
        mag, sz = struct.unpack_from("<4sI", d, p)
        if mag == b"pic1" and d[p + 0xC:p + 0x24].split(b"\0")[0] == b"hyphen":
            struct.pack_into("<2f", d, p + 0x4C, size, size)
            return
        p += sz
    raise SystemExit("hyphen pane not found")


def main():
    pic, arc_in, arc_out = sys.argv[1:4]
    size = float(sys.argv[4]) if len(sys.argv) > 4 else 200.0
    tmp = tempfile.mkdtemp()
    png, dds, stage = (os.path.join(tmp, n) for n in ("pic.png", "pic.dds", "stage.arc"))
    corner = "255*clip(28.5-hypot(max(0,abs(X-63.5)-35.5),max(0,abs(Y-63.5)-35.5)),0,1)"
    subprocess.run([FFMPEG, "-v", "error", "-y", "-i", pic, "-vf",
                    f"scale=128:128:flags=lanczos,format=rgba,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='{corner}'",
                    png], check=True)
    subprocess.run([TEX, png, dds, "-f", "BC7RgbaUnormSrgb", "--no-mipmaps"], check=True)
    d = bytearray(open(arc_in, "rb").read())
    patch_headers(d, size)
    open(stage, "wb").write(d)
    subprocess.run([LAYOUT, "replace", stage, TEXTURE, dds, arc_out], check=True)


if __name__ == "__main__":
    main()
