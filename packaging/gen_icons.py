#!/usr/bin/env python3
"""Generate app icons (PNG/ICO/.iconset) from the same design as the in-app icon. Stdlib only."""
import math, os, struct, zlib, subprocess, sys

def render(s):
    c = s / 2.0
    ro, ri = s * 0.30, s * 0.19
    hx, hy = c - ro * 0.45, c - ro * 0.45
    lo, hi, r = s * 22 / 256, s - s * 22 / 256, s * 46 / 256
    def inside(x, y):
        if x < lo or x > hi or y < lo or y > hi: return False
        nx = min(max(x, lo + r), hi - r); ny = min(max(y, lo + r), hi - r)
        return math.hypot(x - nx, y - ny) <= r
    buf = bytearray(s * s * 4)
    for y in range(s):
        for x in range(s):
            i = (y * s + x) * 4
            if not inside(x + .5, y + .5):
                continue
            d = math.hypot(x + .5 - c, y + .5 - c)
            col = (0x1E, 0x6F, 0xB8)
            if ri <= d < ro: col = (0xF0, 0xF4, 0xF8)
            elif d < ri: col = (0x10, 0x1A, 0x28)
            if math.hypot(x + .5 - hx, y + .5 - hy) < s * 0.04: col = (0xFF, 0xFF, 0xFF)
            buf[i:i+4] = bytes((*col, 0xFF))
    return bytes(buf)

def png(path, s, rgba=None):
    rgba = rgba or render(s)
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    raw = bytearray()
    for y in range(s):
        raw.append(0); raw += rgba[y*s*4:(y+1)*s*4]
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", s, s, 8, 6, 0, 0, 0)) \
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")
    open(path, "wb").write(data)
    return data

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
assets = os.path.join(root, "assets")
png(os.path.join(assets, "icon.png"), 512)
png(os.path.join(assets, "icon_256.png"), 256)
# Windows .ico wrapping a 256 PNG (Vista+ supports PNG-in-ICO)
p256 = png(os.path.join(assets, "_tmp256.png"), 256)
with open(os.path.join(assets, "icon.ico"), "wb") as f:
    f.write(struct.pack("<HHH", 0, 1, 1))
    f.write(struct.pack("<BBBBHHII", 0, 0, 0, 0, 1, 32, len(p256), 22))
    f.write(p256)
os.remove(os.path.join(assets, "_tmp256.png"))
# macOS .icns via iconutil from an .iconset
iconset = os.path.join(assets, "AppIcon.iconset"); os.makedirs(iconset, exist_ok=True)
for sz in (16, 32, 64, 128, 256, 512):
    png(os.path.join(iconset, f"icon_{sz}x{sz}.png"), sz)
    png(os.path.join(iconset, f"icon_{sz//2}x{sz//2}@2x.png"), sz)
if sys.platform == "darwin":
    subprocess.run(["iconutil", "-c", "icns", iconset, "-o", os.path.join(assets, "icon.icns")], check=True)
print("icons generated in", assets)
