"""Generate apps/tahan-desktop/icons/icon.ico (PNG-in-ICO, 256x256) without external deps."""
import struct, zlib, os

W = H = 256

# Simple flat design: dark navy rounded-ish square bg + teal ascending chart line + book spine
BG = (15, 23, 42, 255)          # navy
FG = (56, 189, 248, 255)        # teal
BG2 = (30, 41, 59, 255)

px = [[BG for _ in range(W)] for _ in range(H)]

def put(x, y, c):
    if 0 <= x < W and 0 <= y < H:
        px[y][x] = c

# rounded corners cut
r = 40
for y in range(H):
    for x in range(W):
        if (x < r and y < r and (r-x)**2 + (r-y)**2 > r*r) or \
           (x >= W-r and y < r and (x-(W-r-1))**2 + (r-y)**2 > r*r) or \
           (x < r and y >= H-r and (r-x)**2 + (y-(H-r-1))**2 > r*r) or \
           (x >= W-r and y >= H-r and (x-(W-r-1))**2 + (y-(H-r-1))**2 > r*r):
            px[y][x] = (0, 0, 0, 0)

# ascending chart polyline (thick)
points = [(50, 190), (100, 150), (130, 165), (205, 85)]
for i in range(len(points)-1):
    x0, y0 = points[i]; x1, y1 = points[i+1]
    steps = max(abs(x1-x0), abs(y1-y0))
    for s in range(steps+1):
        x = x0 + (x1-x0)*s//steps
        y = y0 + (y1-y0)*s//steps
        for dx in range(-5, 6):
            for dy in range(-5, 6):
                if dx*dx + dy*dy <= 25:
                    put(x+dx, y+dy, FG)

# baseline under chart
for x in range(45, 210):
    for dy in range(3):
        put(x, 205+dy, BG2)

# encode PNG
raw = b"".join(b"\x00" + b"".join(bytes(p) for p in row) for row in px)
def chunk(tag, data):
    c = struct.pack(">I", len(data)) + tag + data
    return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(raw, 9))
png += chunk(b"IEND", b"")

# wrap in ICO (PNG-in-ICO)
ico = struct.pack("<HHH", 0, 1, 1)
ico += struct.pack("<BBBBHHII", 0, 0, 0, 0, 1, 32, len(png), 22)
ico += png

os.makedirs("apps/tahan-desktop/icons", exist_ok=True)
with open("apps/tahan-desktop/icons/icon.ico", "wb") as f:
    f.write(ico)
print("icon.ico written:", len(ico), "bytes")
