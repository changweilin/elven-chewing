#!/usr/bin/env python3
"""Elven glyph source of truth.

Each glyph is defined once as vector pen commands in a 0..100 coordinate box and
rendered two ways so the Windows tray and the web sandbox stay identical:

  * multi-size .ico  -> tip/rc/   (what the Win10/11 language bar / tray shows)
  * inline SVG path  -> printed,  pasted into crates/web_demo/static/index.html

Run:  python design/elven-glyphs/gen.py            # write icons + previews
      python design/elven-glyphs/gen.py --svg      # also print SVG path data
"""
import math
import os
import sys

from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
RC = os.path.join(ROOT, "tip", "rc")

# ---- Elven palette (from design/logo-concepts) -----------------------------
FOREST = (31, 77, 63, 255)      # deep forest green  #1f4d3f  (light-tray stroke)
MITHRIL = (223, 231, 226, 255)  # silver / mithril   #dfe7e2  (dark-tray stroke)
GOLD = (208, 162, 74, 255)      # elven gold         #d0a24a  (dot / accent)

# ============================================================================
# A tiny vector "pen": records path ops in a 0..100 box. The SAME ops drive the
# PIL raster (brush stroke) and the emitted SVG path string.
# ============================================================================
class Pen:
    def __init__(self):
        self.cmds = []

    def M(self, x, y): self.cmds.append(("M", (x, y))); return self
    def L(self, x, y): self.cmds.append(("L", (x, y))); return self
    def Q(self, cx, cy, x, y): self.cmds.append(("Q", (cx, cy, x, y))); return self
    def C(self, a, b, c, d, x, y): self.cmds.append(("C", (a, b, c, d, x, y))); return self
    def Z(self): self.cmds.append(("Z", ())); return self


def _flatten(cmds, n=40):
    """Turn pen ops into a list of subpaths (each a list of (x,y) points)."""
    subs, cur, start = [], (0.0, 0.0), (0.0, 0.0)
    for op, a in cmds:
        if op == "M":
            cur = (a[0], a[1]); start = cur; subs.append([cur])
        elif op == "L":
            subs[-1].append((a[0], a[1])); cur = (a[0], a[1])
        elif op == "Q":
            cx, cy, x, y = a
            for i in range(1, n + 1):
                t = i / n
                mt = 1 - t
                px = mt * mt * cur[0] + 2 * mt * t * cx + t * t * x
                py = mt * mt * cur[1] + 2 * mt * t * cy + t * t * y
                subs[-1].append((px, py))
            cur = (x, y)
        elif op == "C":
            c1x, c1y, c2x, c2y, x, y = a
            for i in range(1, n + 1):
                t = i / n
                mt = 1 - t
                px = (mt**3) * cur[0] + 3 * mt * mt * t * c1x + 3 * mt * t * t * c2x + t**3 * x
                py = (mt**3) * cur[1] + 3 * mt * mt * t * c1y + 3 * mt * t * t * c2y + t**3 * y
                subs[-1].append((px, py))
            cur = (x, y)
        elif op == "Z":
            subs[-1].append(start); cur = start
    return subs


def svg_path(pen):
    out = []
    for op, a in pen.cmds:
        if op == "M": out.append(f"M{a[0]:g} {a[1]:g}")
        elif op == "L": out.append(f"L{a[0]:g} {a[1]:g}")
        elif op == "Q": out.append(f"Q{a[0]:g} {a[1]:g} {a[2]:g} {a[3]:g}")
        elif op == "C": out.append(f"C{a[0]:g} {a[1]:g} {a[2]:g} {a[3]:g} {a[4]:g} {a[5]:g}")
        elif op == "Z": out.append("Z")
    return "".join(out)


# ============================================================================
# Glyph library  (coords in 0..100; stroke width is in the same units)
# Each entry: list of (Pen, "stroke"|"fill", width) layers, optional accents.
# ============================================================================
SW = 13  # default stroke width


def g_chi():
    """中文模式 — the primary tengwa: an upright stem, a full rounded lower bow
    on the right, and a leaf flourish curling left from the crown (echoes the
    quill mark in the brand logo). The crown curls left so the corner
    update-dot never collides with it."""
    stem = Pen().M(42, 16).C(42, 40, 42, 62, 42, 84)
    bow = Pen().M(42, 50).C(80, 48, 82, 90, 42, 86)
    crown = Pen().M(42, 20).C(28, 10, 20, 22, 34, 30)
    return [(stem, SW), (bow, SW), (crown, SW)]


def g_eng():
    """英數模式 — a woven tengwa: a stem with two bows on alternating sides, a
    clearly different (S-woven) silhouette from 中文 and not a Latin letter."""
    stem = Pen().M(50, 16).C(50, 40, 50, 62, 50, 84)
    upper = Pen().M(50, 30).C(78, 28, 78, 54, 50, 50)
    lower = Pen().M(50, 56).C(22, 54, 22, 82, 50, 80)
    return [(stem, SW), (upper, SW), (lower, SW)]


def g_simp():
    """簡體輸出 — the 中文 letter carrying a tehta (a free-floating diacritic
    curl) high above, the authentic Tengwar way to mark a variant."""
    layers = g_chi()
    tehta = Pen().M(40, 10).C(50, 1, 62, 4, 58, 14)  # curl centred-high, clear of dot
    return layers + [(tehta, SW - 4)]


def g_full():
    """全形 — a closed elven ring with a sun dot (a 'full' luminary)."""
    ring = Pen().M(50, 18).C(68, 18, 82, 32, 82, 50) \
                .C(82, 68, 68, 82, 50, 82) \
                .C(32, 82, 18, 68, 18, 50) \
                .C(18, 32, 32, 18, 50, 18).Z()
    dot = Pen().M(50, 44).C(53, 44, 56, 47, 56, 50).C(56, 53, 53, 56, 50, 56) \
               .C(47, 56, 44, 53, 44, 50).C(44, 47, 47, 44, 50, 44).Z()
    return [(ring, SW - 2), (dot, SW + 2)]


def g_half():
    """半形 — the same ring but only the leading half drawn (a waxing moon)."""
    arc = Pen().M(50, 18).C(32, 18, 18, 32, 18, 50).C(18, 68, 32, 82, 50, 82)
    bar = Pen().M(50, 18).L(50, 82)
    return [(arc, SW - 2), (bar, SW - 4)]


def g_config():
    """設定 — the eight-pointed Elven star (star of Eärendil motif)."""
    layers = []
    cx, cy, R, r = 50, 50, 40, 14
    pts = []
    for i in range(16):
        ang = math.pi / 2 + i * math.pi / 8  # start pointing up
        rad = R if i % 2 == 0 else r
        pts.append((cx + rad * math.cos(ang), cy - rad * math.sin(ang)))
    star = Pen().M(*pts[0])
    for p in pts[1:]:
        star.L(*p)
    star.Z()
    return [("FILL", star)]


GLYPHS = {
    "chi": g_chi,
    "eng": g_eng,
    "simp": g_simp,
    "full": g_full,
    "half": g_half,
    "config": g_config,
}


# ============================================================================
# Raster rendering (supersampled brush) + .ico writers
# ============================================================================
def _stroke(draw, pen, width, color, scale, off):
    for sp in _flatten(pen.cmds):
        pts = [(x * scale + off, y * scale + off) for x, y in sp]
        w = max(1, int(round(width * scale)))
        if len(pts) >= 2:
            draw.line(pts, fill=color, width=w, joint="curve")
        r = w / 2
        for x, y in (pts[0], pts[-1]):
            draw.ellipse([x - r, y - r, x + r, y + r], fill=color)


def _fill(draw, pen, color, scale, off):
    for sp in _flatten(pen.cmds):
        pts = [(x * scale + off, y * scale + off) for x, y in sp]
        if len(pts) >= 3:
            draw.polygon(pts, fill=color)


def render(name, size, color, dot=False, ss=4):
    """Render one glyph to an RGBA image of (size,size)."""
    S = size * ss
    pad = S * 0.10
    scale = (S - 2 * pad) / 100.0
    off = pad
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    for layer in GLYPHS[name]():
        if layer[0] == "FILL":
            _fill(d, layer[1], color, scale, off)
        else:
            pen, w = layer
            _stroke(d, pen, w, color, scale, off)
    if dot:
        # update-available luminary in the upper-right corner
        rr = S * 0.16
        cx, cy = S - rr - pad * 0.1, rr + pad * 0.1
        d.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], fill=GOLD)
    return img.resize((size, size), Image.LANCZOS)


def write_ico(path, color, dot=False, sizes=(16, 32, 48, 256), name=None):
    name = name or os.path.splitext(os.path.basename(path))[0]
    base = name.replace("_dark", "").replace("_dot", "")
    # Render a single crisp master and let Pillow emit exactly the requested
    # sizes (passing `sizes` + `append_images` together is buggy in this Pillow).
    master = render(base, max(max(sizes), 128), color, dot=dot)
    master.save(path, format="ICO", sizes=[(s, s) for s in sizes])
    print("wrote", os.path.relpath(path, ROOT))


def _rounded_mask(S, radius):
    m = Image.new("L", (S, S), 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, S - 1, S - 1], radius=radius, fill=255)
    return m


def render_app_icon(size, ss=4):
    """Brand mark for the web favicon / app tile: a forest-green rounded tile
    with the 中文 tengwa in mithril and a gold sparkle (matches design/logo)."""
    S = size * ss
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    # vertical forest-green gradient background
    top, bot = (34, 81, 63), (18, 56, 44)
    grad = Image.new("RGBA", (S, S))
    gd = grad.load()
    for y in range(S):
        t = y / (S - 1)
        gd_row = tuple(int(top[i] + (bot[i] - top[i]) * t) for i in range(3)) + (255,)
        for x in range(S):
            gd[x, y] = gd_row
    img.paste(grad, (0, 0), _rounded_mask(S, int(S * 0.22)))
    d = ImageDraw.Draw(img)
    # thin gold inner keyline
    inset = int(S * 0.045)
    d.rounded_rectangle([inset, inset, S - 1 - inset, S - 1 - inset],
                        radius=int(S * 0.18), outline=GOLD, width=max(1, int(S * 0.012)))
    # 中文 tengwa, mithril, centred and scaled to ~62%
    glyph_box = int(S * 0.60)
    gimg = render("chi", glyph_box, MITHRIL, ss=1)
    gx = (S - glyph_box) // 2 + int(S * 0.02)
    gy = (S - glyph_box) // 2 + int(S * 0.04)
    img.alpha_composite(gimg, (gx, gy))
    # gold four-point sparkle, upper-right
    sx, sy, r = int(S * 0.74), int(S * 0.27), int(S * 0.085)
    d.polygon([(sx, sy - r), (sx + r * 0.28, sy - r * 0.28),
               (sx + r, sy), (sx + r * 0.28, sy + r * 0.28),
               (sx, sy + r), (sx - r * 0.28, sy + r * 0.28),
               (sx - r, sy), (sx - r * 0.28, sy - r * 0.28)], fill=GOLD)
    return img.resize((size, size), Image.LANCZOS)


def write_favicons():
    static = os.path.join(ROOT, "crates", "web_demo", "static")
    render_app_icon(32).save(os.path.join(static, "favicon-32.png"))
    render_app_icon(180).save(os.path.join(static, "apple-touch-icon.png"))
    master = render_app_icon(256)
    master.save(os.path.join(static, "favicon.ico"), sizes=[(16, 16), (32, 32), (48, 48)])
    print("wrote web favicons -> crates/web_demo/static/")


def preview():
    """Contact sheet of every glyph (light & dark) on a checkerboard."""
    names = list(GLYPHS)
    cell, pad, cols = 72, 12, len(names)
    rows = 2
    W = cols * (cell + pad) + pad
    H = rows * (cell + pad + 16) + pad
    sheet = Image.new("RGBA", (W, H), (205, 205, 205, 255))
    for y in range(0, H, 8):
        for x in range(0, W, 8):
            if (x // 8 + y // 8) % 2 == 0:
                sheet.paste((175, 175, 175, 255), (x, y, min(x + 8, W), min(y + 8, H)))
    d = ImageDraw.Draw(sheet)
    for ci, n in enumerate(names):
        for ri, (col, lbl) in enumerate([(FOREST, n), (MITHRIL, n + " dark")]):
            x = pad + ci * (cell + pad)
            y = pad + ri * (cell + pad + 16)
            bg = (245, 245, 245, 255) if ri == 0 else (40, 48, 44, 255)
            sheet.paste(bg, (x, y, x + cell, y + cell))
            sheet.alpha_composite(render(n, cell, col, dot=(n == "chi")), (x, y))
            d.text((x, y + cell + 1), lbl, fill=(10, 10, 10, 255))
    out = os.path.join(HERE, "_preview.png")
    sheet.save(out)
    print("preview ->", os.path.relpath(out, ROOT))


def write_all_icos():
    # base glyphs: 中 / 英 / 簡 (multi-size) light + dark + dot variants
    for nm, col in [("chi", FOREST), ("eng", FOREST), ("simp", FOREST)]:
        write_ico(os.path.join(RC, f"{nm}.ico"), col, name=nm)
    for nm in ("chi", "eng", "simp"):
        write_ico(os.path.join(RC, f"{nm}_dark.ico"), MITHRIL, name=nm)
    for nm in ("chi", "eng", "simp"):
        write_ico(os.path.join(RC, f"{nm}_dot.ico"), FOREST, dot=True, name=nm)
        write_ico(os.path.join(RC, f"{nm}_dark_dot.ico"), MITHRIL, dot=True, name=nm)
    # single-size indicators
    write_ico(os.path.join(RC, "full.ico"), FOREST, sizes=(16,), name="full")
    write_ico(os.path.join(RC, "half.ico"), FOREST, sizes=(16,), name="half")
    write_ico(os.path.join(RC, "config.ico"), FOREST, sizes=(16,), name="config")


if __name__ == "__main__":
    if "--svg" in sys.argv:
        for n, fn in GLYPHS.items():
            print(f"\n/* {n} */")
            for layer in fn():
                if layer[0] == "FILL":
                    print(f'  fill  : {svg_path(layer[1])}')
                else:
                    pen, w = layer
                    print(f'  w={w}  : {svg_path(pen)}')
    else:
        preview()
        if "--ico" in sys.argv:
            write_all_icos()
        if "--favicon" in sys.argv:
            # large preview so I can eyeball the brand mark
            render_app_icon(256).save(os.path.join(HERE, "_favicon.png"))
            write_favicons()
