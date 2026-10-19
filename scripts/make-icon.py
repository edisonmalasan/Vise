"""Write the application icon as a PNG.

The desktop build wants a PNG for the window and the Linux package, and an ICO
for the Windows installer. The icon is drawn here rather than shipped as a
binary so both formats come from one description and stay in step.

Run: python scripts/make-icon.py
"""

import os
import struct
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ICONS = os.path.join(ROOT, "apps", "vise-desktop", "src-tauri", "icons")

# The mark: a rounded square in the application colour with a pair of chevrons
# that read as a transcode, on a transparent ground.
FOREGROUND = (0x3B, 0x82, 0xF6)
BACKGROUND = (0x0F, 0x17, 0x26)
ACCENT = (0xF8, 0xFA, 0xFC)

SIZES = [32, 64, 128, 256, 512]


def rounded_square(x, y, size, radius):
    """Whether a point falls inside a rounded square."""
    left = top = radius
    right = bottom = size - radius
    if not (left <= x < right or top <= y < bottom):
        # In a corner region, check against the circle at that corner.
        cx = left if x < left else right - 1
        cy = top if y < top else bottom - 1
        return (x - cx) ** 2 + (y - cy) ** 2 <= radius**2
    return True


def pixel(x, y, size):
    """The colour of one pixel, as (r, g, b, a)."""
    unit = size / 32.0
    if not rounded_square(x, y, size, 5 * unit):
        return (0, 0, 0, 0)
    # A pair of chevrons pointing right, which read as a transcode.
    stroke = 2.2 * unit
    for origin in (5.0, 15.0):
        left = origin * unit
        peak = left + 7 * unit
        top = 8 * unit
        bottom = 24 * unit
        middle = (top + bottom) / 2
        if not (top <= y <= bottom):
            continue
        if y <= middle:
            edge = left + (y - top) / (middle - top) * (peak - left)
        else:
            edge = peak - (y - middle) / (bottom - middle) * (peak - left)
        if abs(x - edge) <= stroke:
            return ACCENT + (255,)
    return BACKGROUND + (255,)


def png_bytes(size):
    """A PNG of the icon at the given size."""
    raw = bytearray()
    for y in range(size):
        raw.append(0)  # no filter on this row
        for x in range(size):
            raw.extend(pixel(x, y, size))

    def chunk(tag, payload):
        out = struct.pack(">I", len(payload)) + tag + payload
        return out + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def ico_bytes(images):
    """An ICO holding the PNG-encoded images."""
    count = len(images)
    header = struct.pack("<HHH", 0, 1, count)
    offset = 6 + 16 * count
    entries = bytearray()
    payloads = bytearray()
    for size, data in images:
        entries += struct.pack(
            "<BBBBHHII",
            0 if size >= 256 else size,
            0 if size >= 256 else size,
            0,
            0,
            1,
            32,
            len(data),
            offset,
        )
        payloads += data
        offset += len(data)
    return bytes(header) + bytes(entries) + bytes(payloads)


def main():
    os.makedirs(ICONS, exist_ok=True)
    images = []
    for size in SIZES:
        data = png_bytes(size)
        images.append((size, data))
        path = os.path.join(ICONS, f"{size}x{size}.png")
        with open(path, "wb") as handle:
            handle.write(data)
        print(f"wrote {os.path.relpath(path, ROOT)}")

    # The window and the Linux package use the 512-pixel image; the smaller ones
    # are what a desktop environment picks for its own menu.
    with open(os.path.join(ICONS, "icon.png"), "wb") as handle:
        handle.write(dict(images)[512])
    print("wrote apps/vise-desktop/src-tauri/icons/icon.png")

    ico = ico_bytes([(size, data) for size, data in images if size <= 256])
    with open(os.path.join(ICONS, "icon.ico"), "wb") as handle:
        handle.write(ico)
    print("wrote apps/vise-desktop/src-tauri/icons/icon.ico")


if __name__ == "__main__":
    main()
