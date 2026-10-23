import math, struct, sys, zlib

SIZE, SS = 512, 4
BG = (0x0f, 0x10, 0x12)
MARK = (0xe5, 0xa0, 0x0d)
# The play mark, as in the favicon: a triangle with rounded corners, a little
# right of centre so it looks centred. Corners rounded by shrinking the
# triangle and growing it back by R.
R = 26.0
H = 300.0
W = H * 14 / 16.8
cx, cy = SIZE / 2 + W * 0.08, SIZE / 2
A = (cx - W / 2, cy - H / 2); B = (cx - W / 2, cy + H / 2); C = (cx + W / 2, cy)

def inset(a, b, c, d):
    # Move each edge inward by d: the triangle's incentre shrink.
    la = math.dist(b, c); lb = math.dist(a, c); lc = math.dist(a, b)
    p = la + lb + lc
    ix = (la * a[0] + lb * b[0] + lc * c[0]) / p; iy = (la * a[1] + lb * b[1] + lc * c[1]) / p
    area = abs((b[0]-a[0])*(c[1]-a[1]) - (c[0]-a[0])*(b[1]-a[1])) / 2
    r = 2 * area / p
    k = (r - d) / r
    return [(ix + (v[0]-ix) * k, iy + (v[1]-iy) * k) for v in (a, b, c)]

T = inset(A, B, C, R)

def seg_dist(p, a, b):
    ax, ay = a; bx, by = b; px, py = p
    t = max(0, min(1, ((px-ax)*(bx-ax) + (py-ay)*(by-ay)) / ((bx-ax)**2 + (by-ay)**2)))
    return math.hypot(px - (ax + t*(bx-ax)), py - (ay + t*(by-ay)))

def inside_tri(p, a, b, c):
    def s(p1, p2, p3): return (p1[0]-p3[0])*(p2[1]-p3[1]) - (p2[0]-p3[0])*(p1[1]-p3[1])
    d1, d2, d3 = s(p, a, b), s(p, b, c), s(p, c, a)
    return not ((d1 < 0 or d2 < 0 or d3 < 0) and (d1 > 0 or d2 > 0 or d3 > 0))

def covered(x, y):
    p = (x, y)
    if inside_tri(p, *T): return True
    return min(seg_dist(p, T[i], T[(i+1) % 3]) for i in range(3)) <= R

rows = []
for y in range(SIZE):
    row = bytearray([0])
    for x in range(SIZE):
        hits = sum(covered(x + (i + .5) / SS, y + (j + .5) / SS) for i in range(SS) for j in range(SS))
        a = hits / (SS * SS)
        row += bytes(round(BG[k] + (MARK[k] - BG[k]) * a) for k in range(3))
    rows.append(bytes(row))

def chunk(kind, data): return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xffffffff)
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(b"".join(rows), 9)) + chunk(b"IEND", b"")
open(sys.argv[1], "wb").write(png)
