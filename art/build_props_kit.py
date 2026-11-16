"""Builds the props kit (ticket A03) procedurally in the open .blend.

Run inside Blender (Text Editor, or the MCP with `__file__` set), then save and
export:
    blender art/props_kit.blend --background --python art/build_props_kit.py
rebuilds every prop collection from scratch; art/export.py then writes the
.glb files. It reuses the dungeon kit's helpers (`Piece`, `material`,
`kit_collection`) through build_town_kit.py, so the props share the town's
stone and wood, and adds a few materials of its own.

Same conventions as the other kits: +X east, +Y north (towards smaller grid
y), +Z up, one unit per metre and tile, each prop built at the origin on the
tile centre and inside its tile. Props face south (-Y): the game turns a prop
standing beside a wall to face away from it (src/models.rs `model_yaw`).
Every collection is named for the raws `model` value of its prop.

Lights: the game draws the flame of a prop with a raws `light` (src/lighting.rs)
as a teardrop whose round base (radius 0.09) is centred at the light's height,
so the candle's wick and the lamp's burner sit just under that point and no
flame is modelled here.
"""

import importlib.util
import math
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("build_town_kit", HERE / "build_town_kit.py")
tk = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tk)
dk = tk.dk

# sRGB base colour, roughness, metallic, added to the town palette.
dk.PALETTE.update({
    "cloth": ((0.56, 0.20, 0.17), 0.95, 0.0),
    "linen": ((0.80, 0.74, 0.60), 0.95, 0.0),
    "leather": ((0.70, 0.50, 0.32), 0.9, 0.0),
    "wax": ((0.92, 0.86, 0.68), 0.8, 0.0),
    "glass": ((0.40, 0.78, 0.45), 0.3, 0.0),
    "steel": ((0.60, 0.62, 0.66), 0.5, 0.3),
    "flesh": ((0.55, 0.27, 0.25), 0.8, 0.0),
    "blood": ((0.30, 0.04, 0.04), 0.5, 0.0),
})
GLASS_GLOW = 0.6  # the potions glow faintly, so the alchemist's bench reads green in the dark

CANDLE_FLAME = 0.75  # raws Candle light height
LAMP_FLAME = 2.4  # raws Lamp Post light height
FLAME_BASE = 0.09  # radius of the flame's round base (src/lighting.rs `flame`)


# --- helpers -----------------------------------------------------------------

def placed(p, build, at=(0.0, 0.0, 0.0), yaw=0.0, tilt_x=0.0, tilt_y=0.0, scale=(1.0, 1.0, 1.0)):
    """Runs `build()` (which adds faces at the origin), then scales, tilts
    about X then Y, turns about Z and moves what it added."""
    start = len(p.bm.faces)
    build()
    m = (Matrix.Translation(at) @ Matrix.Rotation(yaw, 4, "Z") @ Matrix.Rotation(tilt_y, 4, "Y")
         @ Matrix.Rotation(tilt_x, 4, "X") @ Matrix.Diagonal((*scale, 1.0)))
    for f in list(p.bm.faces)[start:]:
        for v in f.verts:
            v.co = m @ v.co


def drum(p, cx, cy, z0, z1, r0, r1, sides, mat, shade=1.0):
    """A closed prism or frustum (`prism` plus a bottom)."""
    p.prism(cx, cy, z0, z1, r0, r1, sides, mat, shade, cap=r1 > 0)
    p.face([(cx + r0 * math.cos(2 * math.pi * i / sides), cy + r0 * math.sin(2 * math.pi * i / sides), z0)
            for i in reversed(range(sides))], mat, shade * 0.6)


def post(p, x, y, z0, z1, w, mat="wood", shade=0.85):
    p.box(x - w / 2, x + w / 2, y - w / 2, y + w / 2, z0, z1, mat, shade, skip=["b"] if z0 == 0 else ())


def plank(p, x0, x1, y0, y1, z0, z1, mat="wood"):
    p.box(x0, x1, y0, y1, z0, z1, mat, p.rng.uniform(0.85, 1.1))


def slab_from_outline(p, outline, y0, y1, mat, shade=1.0):
    """A thin upright slab, its front at y0 and back at y1, cut to an (x, z) outline."""
    front = [(x, y0, z) for x, z in outline]
    back = [(x, y1, z) for x, z in reversed(outline)]
    p.face(front, mat, shade)
    p.face(back, mat, shade * 0.7)
    n = len(outline)
    for i in range(n):
        (xa, za), (xb, zb) = outline[i], outline[(i + 1) % n]
        p.face([(xa, y0, za), (xa, y1, za), (xb, y1, zb), (xb, y0, zb)], mat, shade * 0.8)


# --- traps and floor clutter -------------------------------------------------

def build_bear_trap():
    """An open trap lying flat: two toothed half-rings either side of the pan,
    a spring bar under them and a short chain to a stake."""
    p = dk.Piece("bear_trap")
    r, segs = 0.22, 5
    p.box(-0.36, 0.36, -0.03, 0.03, 0.0, 0.03, "iron", 0.8, skip=["b"])
    p.box(-0.07, 0.07, -0.07, 0.07, 0.0, 0.045, "iron", 1.1, skip=["b"])
    for side in (-1, 1):
        for k in range(segs):
            a0 = math.pi * k / segs - math.pi / 2
            a1 = math.pi * (k + 1) / segs - math.pi / 2
            pts = [(side * r * math.cos(a), r * math.sin(a)) for a in (a0, a1)]
            inner = [(side * (r - 0.035) * math.cos(a), (r - 0.035) * math.sin(a)) for a in (a0, a1)]
            ring = [(*pts[0], 0.0), (*pts[1], 0.0), (*inner[1], 0.0), (*inner[0], 0.0)]
            top = [(x, y, 0.035) for x, y, _ in ring]
            p.face(top, "iron", 0.95)
            for i in range(4):
                j = (i + 1) % 4
                p.face([ring[i], ring[j], top[j], top[i]], "iron", 0.7)
            am = (a0 + a1) / 2
            p.prism(side * (r - 0.017) * math.cos(am), (r - 0.017) * math.sin(am), 0.035, 0.1, 0.022, 0.0, 3, "iron", 1.15, cap=False)
    for k in range(3):
        x = 0.37 + k * 0.03
        p.box(x - 0.02, x + 0.02, -0.07 - k * 0.05, -0.05 - k * 0.05, 0.0, 0.02, "iron", 0.75, skip=["b"])
    post(p, 0.43, -0.25, 0.0, 0.08, 0.05, "wood", 0.8)
    return p


def build_dead_thing():
    """A gutted beast on its back, legs in the air, ribs showing, in a pool of blood."""
    p = dk.Piece("dead_thing")
    p.prism(0.0, 0.0, 0.0, 0.006, 0.4, 0.4, 9, "blood", 1.0)
    # The body, along x: three frustum sections on their side.
    sections = [(-0.25, 0.10), (-0.12, 0.15), (0.08, 0.16), (0.24, 0.12)]
    for (xa, ra), (xb, rb) in zip(sections, sections[1:]):
        placed(p, lambda ra=ra, rb=rb, xa=xa, xb=xb: p.prism(0, 0, xa, xb, ra, rb, 7, "flesh", p.rng.uniform(0.85, 1.0), cap=False),
               at=(0, 0, 0.17), tilt_y=math.pi / 2)
    placed(p, lambda: drum(p, 0, 0, 0.0, 0.03, 0.12, 0.09, 7, "flesh", 0.8), at=(0.24, 0, 0.17), tilt_y=math.pi / 2)
    # Head and snout at the west end, lolling.
    p.box(-0.42, -0.24, -0.07, 0.08, 0.04, 0.2, "flesh", 0.9)
    p.box(-0.52, -0.42, -0.05, 0.05, 0.04, 0.12, "flesh", 0.8)
    for y in (-0.06, 0.06):
        p.box(-0.33, -0.29, y - 0.02, y + 0.02, 0.2, 0.26, "flesh", 0.7)
    # Ribs over the opened belly.
    for x in (-0.1, -0.02, 0.06):
        for side in (-1, 1):
            placed(p, lambda: p.box(-0.012, 0.012, -0.012, 0.012, 0.0, 0.13, "linen", 1.05),
                   at=(x, side * 0.02, 0.27), tilt_x=side * 1.0)
    p.box(-0.16, 0.12, -0.08, 0.08, 0.27, 0.29, "blood", 0.9)
    # Stiff legs splayed up.
    for x, y in ((-0.14, -0.08), (-0.14, 0.08), (0.17, -0.07), (0.17, 0.07)):
        placed(p, lambda: p.box(-0.025, 0.025, -0.025, 0.025, 0.0, 0.26, "flesh", 0.75),
               at=(x, y, 0.2), tilt_x=-0.45 * (1 if y > 0 else -1), tilt_y=0.2 * (1 if x > 0 else -1))
    return p


# --- pub ---------------------------------------------------------------------

def build_keg():
    """A barrel on its side in a cradle, the tap towards the east."""
    p = dk.Piece("keg")
    length, r_end, r_mid = 0.72, 0.26, 0.31
    rings = [(-length / 2, r_end), (-0.12, r_mid), (0.12, r_mid), (length / 2, r_end)]

    def barrel():
        for (za, ra), (zb, rb) in zip(rings, rings[1:]):
            p.prism(0, 0, za, zb, ra, rb, 10, "wood", p.rng.uniform(0.9, 1.05), cap=False)
        for z, r in ((-length / 2, r_end), (length / 2, r_end)):
            p.face([(r * 0.98 * math.cos(2 * math.pi * i / 10), r * 0.98 * math.sin(2 * math.pi * i / 10), z) for i in range(10)], "wood", 0.7)
        for z in (-0.27, 0.27):
            r = r_end + (r_mid - r_end) * (length / 2 - abs(z)) / (length / 2 - 0.12) + 0.012
            p.prism(0, 0, z - 0.025, z + 0.025, r, r, 10, "iron", 0.9, cap=False)

    axis = 0.45
    placed(p, barrel, at=(0, 0, axis), tilt_y=math.pi / 2)
    p.box(length / 2, length / 2 + 0.07, -0.025, 0.025, axis - 0.16, axis - 0.11, "iron", 1.0)
    p.box(length / 2 + 0.04, length / 2 + 0.07, -0.02, 0.02, axis - 0.2, axis - 0.16, "iron", 0.9)
    for x in (-0.22, 0.22):
        p.box(x - 0.06, x + 0.06, -0.32, 0.32, 0.0, 0.17, "wood", 0.75, skip=["b"])
    return p


def build_table():
    p = dk.Piece("table")
    top = 0.74
    for y0, y1 in ((-0.38, -0.13), (-0.13, 0.12), (0.12, 0.38)):
        plank(p, -0.44, 0.44, y0 + 0.004, y1 - 0.004, top - 0.06, top)
    for x in (-0.36, 0.36):
        for y in (-0.3, 0.3):
            post(p, x, y, 0.0, top - 0.06, 0.07, "wood", 0.7)
    p.box(-0.36, 0.36, -0.02, 0.02, 0.18, 0.24, "wood", 0.65)
    drum(p, 0.18, 0.06, top, top + 0.12, 0.05, 0.055, 6, "wood", 0.6)
    drum(p, -0.15, -0.1, top, top + 0.02, 0.11, 0.12, 8, "stone", 1.05)
    return p


def build_chair():
    """A seat with a slatted back on its north side, facing south."""
    p = dk.Piece("chair")
    seat = 0.46
    plank(p, -0.21, 0.21, -0.2, 0.2, seat - 0.05, seat)
    for x in (-0.17, 0.17):
        post(p, x, -0.16, 0.0, seat - 0.05, 0.05, "wood", 0.7)
        post(p, x, 0.17, 0.0, 1.0, 0.055, "wood", 0.8)
    for z in (0.68, 0.86):
        p.box(-0.15, 0.15, 0.15, 0.19, z, z + 0.09, "wood", 0.9)
    return p


def build_candle():
    """A tall iron candlestick on three feet. The candle's top sits just under
    the flame's base, so the game's flame burns on the wick."""
    p = dk.Piece("candle")
    pan = 0.5
    wax_top = CANDLE_FLAME - FLAME_BASE
    for k in range(3):
        a = 2 * math.pi * k / 3 + math.pi / 2
        placed(p, lambda: p.box(0.0, 0.17, -0.02, 0.02, 0.0, 0.04, "iron", 0.85, skip=["b"]), yaw=a)
    p.prism(0, 0, 0.0, 0.1, 0.05, 0.03, 6, "iron", 0.9)
    p.prism(0, 0, 0.1, pan, 0.022, 0.022, 6, "iron", 0.8, cap=False)
    drum(p, 0, 0, pan, pan + 0.03, 0.08, 0.1, 8, "iron", 1.0)
    p.prism(0, 0, pan + 0.03, wax_top, 0.04, 0.038, 8, "wax", 1.0)
    p.box(0.03, 0.05, -0.04, -0.02, pan + 0.05, wax_top - 0.02, "wax", 0.9)
    p.box(-0.005, 0.005, -0.005, 0.005, wax_top, wax_top + 0.03, "iron", 0.4, skip=["b"])
    return p


# --- smithy ------------------------------------------------------------------

def build_anvil():
    """An anvil with its horn to the east, on a tree stump, a hammer on its face."""
    p = dk.Piece("anvil")
    stump = 0.42
    p.prism(0, 0, 0.0, stump, 0.23, 0.2, 8, "wood", 0.85)
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        placed(p, lambda: p.box(0.0, 0.1, -0.035, 0.035, 0.0, 0.08, "wood", 0.7, skip=["b"]), at=(0.15 * math.cos(a), 0.15 * math.sin(a), 0), yaw=a)
    p.box(-0.15, 0.15, -0.11, 0.11, stump, stump + 0.06, "iron", 0.8)
    p.box(-0.08, 0.08, -0.06, 0.06, stump + 0.06, stump + 0.17, "iron", 0.75, skip=["b"])
    face0, face1 = stump + 0.17, stump + 0.28
    p.box(-0.22, 0.14, -0.09, 0.09, face0, face1, "iron", 1.0)
    placed(p, lambda: p.prism(0, 0, 0.0, 0.2, 0.075, 0.01, 6, "iron", 1.0), at=(0.14, 0, face0 + 0.065), tilt_y=math.pi / 2)
    # The hammer, lying across the face.
    p.box(-0.16, -0.06, -0.03, 0.03, face1, face1 + 0.06, "iron", 1.15)
    p.box(-0.12, -0.1, -0.29, -0.03, face1 + 0.015, face1 + 0.045, "wood", 0.9)
    return p


def build_water_trough():
    """A plank trough with iron bands, full of water."""
    p = dk.Piece("water_trough")
    x, y, h, t = 0.44, 0.26, 0.48, 0.05
    p.box(-x, x, -y, -y + t, 0.04, h, "wood", 0.95)
    p.box(-x, x, y - t, y, 0.04, h, "wood", 0.85)
    p.box(-x, -x + t, -y + t, y - t, 0.04, h, "wood", 0.8, skip=["n", "s"])
    p.box(x - t, x, -y + t, y - t, 0.04, h, "wood", 0.9, skip=["n", "s"])
    p.box(-x + t, x - t, -y + t, y - t, 0.04, h - 0.07, "shallows", 1.0, skip=["n", "s", "e", "w"])
    for bx in (-0.3, 0.3):
        p.box(bx - 0.025, bx + 0.025, -y - 0.012, y + 0.012, 0.04, h + 0.01, "iron", 0.85, skip=["b"])
    for bx in (-0.36, 0.36):
        p.box(bx - 0.05, bx + 0.05, -y + 0.02, y - 0.02, 0.0, 0.04, "wood", 0.6, skip=["b"])
    return p


def sword(p):
    p.box(-0.045, 0.045, -0.012, 0.012, 0.3, 1.15, "steel", 1.0)
    p.prism(0, 0, 1.15, 1.25, 0.045, 0.0, 4, "steel", 1.0, cap=False)
    p.box(-0.12, 0.12, -0.025, 0.025, 0.25, 0.3, "iron", 0.8)
    p.box(-0.018, 0.018, -0.018, 0.018, 0.08, 0.26, "leather", 0.7)
    p.box(-0.03, 0.03, -0.03, 0.03, 0.04, 0.08, "iron", 0.8)


def spear(p):
    p.prism(0, 0, 0.0, 1.38, 0.025, 0.025, 5, "wood", 0.9, cap=False)
    p.box(-0.04, 0.04, -0.04, 0.04, 1.3, 1.36, "iron", 0.8)
    p.prism(0, 0, 1.36, 1.6, 0.065, 0.0, 4, "steel", 1.0, cap=False)


def axe(p):
    p.prism(0, 0, 0.0, 1.0, 0.022, 0.022, 5, "wood", 0.95)
    p.box(0.0, 0.14, -0.018, 0.018, 0.82, 0.96, "iron", 1.1)
    p.box(0.12, 0.22, -0.02, 0.02, 0.7, 1.08, "steel", 1.0)


def build_weapon_rack():
    """An upright rack against its back rail: a spear, two swords and an axe
    leaning in it. Full cover, so it stands about head height."""
    p = dk.Piece("weapon_rack")
    for x in (-0.42, 0.42):
        p.box(x - 0.04, x + 0.04, 0.06, 0.16, 0.0, 1.62, "wood", 0.8, skip=["b"])
        p.box(x - 0.045, x + 0.045, -0.3, 0.3, 0.0, 0.07, "wood", 0.65, skip=["b"])
    p.box(-0.38, 0.38, 0.06, 0.16, 1.48, 1.56, "wood", 0.9)
    p.box(-0.38, 0.38, -0.24, -0.16, 0.06, 0.14, "wood", 0.75)
    p.box(-0.38, 0.38, 0.06, 0.16, 0.3, 0.36, "wood", 0.75)
    lean = -0.14  # tops lean back against the top rail
    for x, make, turn in ((-0.27, spear, 0.0), (-0.09, sword, 0.0), (0.09, sword, 0.0), (0.27, axe, math.pi)):
        placed(p, lambda make=make: make(p), at=(x, -0.18, 0.06), tilt_x=lean, yaw=turn)
    return p


def build_armour_stand():
    """A post on crossed feet carrying a breastplate, pauldrons and a helm."""
    p = dk.Piece("armour_stand")
    for yaw in (0.0, math.pi / 2):
        placed(p, lambda: p.box(-0.3, 0.3, -0.04, 0.04, 0.0, 0.07, "wood", 0.7, skip=["b"]), yaw=yaw)
    post(p, 0, 0, 0.07, 1.5, 0.07, "wood", 0.8)
    p.box(-0.3, 0.3, -0.035, 0.035, 1.3, 1.36, "wood", 0.85)
    placed(p, lambda: drum(p, 0, 0, 0.0, 0.18, 0.2, 0.17, 8, "iron", 0.75), at=(0, 0, 0.72), scale=(1.0, 0.7, 1.0))
    placed(p, lambda: drum(p, 0, 0, 0.0, 0.42, 0.17, 0.25, 8, "steel", 1.0), at=(0, 0, 0.9), scale=(1.0, 0.65, 1.0))
    p.box(-0.02, 0.02, -0.17, -0.14, 0.95, 1.3, "steel", 0.8)
    for side in (-1, 1):
        drum(p, side * 0.25, 0, 1.24, 1.38, 0.1, 0.05, 6, "steel", 0.9)
    drum(p, 0, 0, 1.36, 1.42, 0.06, 0.06, 6, "iron", 0.7)
    drum(p, 0, 0, 1.42, 1.68, 0.13, 0.1, 8, "steel", 1.0)
    p.box(-0.08, 0.08, -0.135, -0.1, 1.54, 1.57, "iron", 0.3)
    p.box(-0.015, 0.015, -0.14, -0.1, 1.42, 1.56, "steel", 0.85)
    return p


# --- clothier ----------------------------------------------------------------

def build_loom():
    """A floor loom: warp threads running from the cloth roll at the front up
    to the back beam, half woven, a heddle bar across them."""
    p = dk.Piece("loom")
    front, back = -0.3, 0.26
    low, high = 0.72, 1.12
    for x in (-0.42, 0.42):
        p.box(x - 0.04, x + 0.04, -0.38, 0.38, 0.0, 0.07, "wood", 0.65, skip=["b"])
        post(p, x, front, 0.07, low + 0.06, 0.07, "wood", 0.8)
        post(p, x, back, 0.07, high + 0.08, 0.07, "wood", 0.8)
        p.box(x - 0.035, x + 0.035, front, back, 0.32, 0.38, "wood", 0.7)
    p.box(-0.42, 0.42, front - 0.05, front + 0.05, low, low + 0.07, "wood", 0.95)
    p.box(-0.42, 0.42, back - 0.05, back + 0.05, high, high + 0.08, "wood", 0.9)
    placed(p, lambda: drum(p, 0, 0, -0.38, 0.38, 0.075, 0.075, 8, "cloth", 0.95), at=(0, front + 0.02, 0.52), tilt_y=math.pi / 2)
    # Threads and cloth on the slope from the front beam to the back beam.
    run = Vector((0.0, back - front, high - low))
    slope = math.atan2(run.z, run.y)
    length = run.length

    def sheet():
        p.box(-0.36, 0.36, 0.0, length * 0.38, -0.012, 0.012, "cloth", 1.0)
        for k in range(9):
            x = -0.32 + k * 0.08
            p.box(x - 0.007, x + 0.007, length * 0.38, length, -0.006, 0.006, "linen", 1.1)
        p.box(-0.38, 0.38, length * 0.55, length * 0.55 + 0.05, -0.03, 0.03, "wood", 1.0)

    placed(p, sheet, at=(0, front, low + 0.07), tilt_x=slope)
    return p


def build_hide_rack():
    """A tall frame with a hide laced inside it."""
    p = dk.Piece("hide_rack")
    for x in (-0.42, 0.42):
        p.box(x - 0.04, x + 0.04, -0.04, 0.04, 0.0, 1.72, "wood", 0.8, skip=["b"])
        p.box(x - 0.045, x + 0.045, -0.28, 0.28, 0.0, 0.07, "wood", 0.65, skip=["b"])
    p.box(-0.46, 0.46, -0.035, 0.035, 1.58, 1.66, "wood", 0.9)
    p.box(-0.38, 0.38, -0.035, 0.035, 0.24, 0.3, "wood", 0.75)
    hide = [(-0.25, 0.4), (-0.08, 0.36), (0.1, 0.4), (0.27, 0.46), (0.3, 0.8), (0.33, 1.1), (0.26, 1.46),
            (0.08, 1.5), (-0.1, 1.45), (-0.28, 1.48), (-0.31, 1.12), (-0.3, 0.75)]
    slab_from_outline(p, hide, -0.015, 0.015, "leather", 1.0)
    for x, z, ex, ez in ((-0.3, 0.75, -0.38, 0.75), (-0.31, 1.12, -0.38, 1.12), (0.3, 0.8, 0.38, 0.8),
                         (0.33, 1.1, 0.38, 1.1), (0.08, 1.5, 0.08, 1.58), (-0.1, 0.36, -0.1, 0.3)):
        p.box(min(x, ex) - 0.008, max(x, ex) + 0.008, -0.008, 0.008, min(z, ez) - 0.008, max(z, ez) + 0.008, "linen", 0.9)
    return p


def build_cabinet():
    """A tall two-door cupboard on a plinth, under a cornice."""
    p = dk.Piece("cabinet")
    p.box(-0.4, 0.4, -0.2, 0.22, 0.0, 0.08, "wood", 0.5, skip=["b"])
    p.box(-0.42, 0.42, -0.22, 0.24, 0.08, 1.78, "wood", 0.75)
    p.box(-0.45, 0.45, -0.25, 0.27, 1.78, 1.86, "wood", 0.9)
    for x0, x1 in ((-0.38, -0.012), (0.012, 0.38)):
        p.box(x0, x1, -0.24, -0.22, 0.14, 1.72, "wood", p.rng.uniform(0.95, 1.1), skip=["n"])
    for x in (-0.06, 0.06):
        p.box(x - 0.015, x + 0.015, -0.27, -0.24, 0.88, 1.0, "iron", 1.0, skip=["n"])
    for z in (0.4, 1.45):
        for x0, x1 in ((-0.38, -0.3), (0.3, 0.38)):
            p.box(x0, x1, -0.25, -0.24, z, z + 0.05, "iron", 0.8, skip=["n"])
    return p


# --- alchemist ---------------------------------------------------------------

def build_chemistry_set():
    """A bench of glassware: a flask over a burner, a tall bottle, a retort and a rack of tubes."""
    p = dk.Piece("chemistry_set")
    top = 0.64
    plank(p, -0.44, 0.44, -0.3, 0.3, top - 0.05, top)
    for x in (-0.38, 0.38):
        for y in (-0.24, 0.24):
            post(p, x, y, 0.0, top - 0.05, 0.06, "wood", 0.7)
    p.box(-0.38, 0.38, -0.24, 0.24, 0.16, 0.2, "wood", 0.6)
    # Burner and flask on a tripod.
    drum(p, -0.22, -0.05, top, top + 0.06, 0.05, 0.04, 6, "iron", 0.9)
    for k in range(3):
        a = 2 * math.pi * k / 3
        p.box(-0.22 + 0.07 * math.cos(a) - 0.01, -0.22 + 0.07 * math.cos(a) + 0.01,
              -0.05 + 0.07 * math.sin(a) - 0.01, -0.05 + 0.07 * math.sin(a) + 0.01, top, top + 0.12, "iron", 0.7, skip=["b"])
    drum(p, -0.22, -0.05, top + 0.12, top + 0.24, 0.1, 0.07, 8, "glass", 1.0)
    p.prism(-0.22, -0.05, top + 0.24, top + 0.36, 0.03, 0.025, 6, "glass", 1.1)
    # A tall bottle at the back.
    p.prism(0.05, 0.16, top, top + 0.24, 0.075, 0.075, 8, "glass", 0.85)
    p.prism(0.05, 0.16, top + 0.24, top + 0.32, 0.075, 0.025, 8, "glass", 0.85, cap=False)
    drum(p, 0.05, 0.16, top + 0.32, top + 0.37, 0.027, 0.027, 6, "leather", 0.8)
    # The retort: a bulb with a spout reaching down to a small dish.
    drum(p, 0.18, -0.08, top, top + 0.17, 0.09, 0.06, 8, "glass", 1.05)
    placed(p, lambda: p.box(-0.012, 0.012, -0.012, 0.012, 0.0, 0.22, "glass", 1.0), at=(0.2, -0.08, top + 0.15), tilt_y=2.0)
    drum(p, 0.38, -0.08, top, top + 0.03, 0.05, 0.06, 6, "stone", 1.0)
    # A rack of three tubes.
    p.box(-0.08, 0.1, 0.12, 0.2, top, top + 0.05, "wood", 0.9)
    for k in range(3):
        p.prism(-0.04 + k * 0.06, 0.16, top + 0.02, top + 0.16, 0.017, 0.017, 5, "glass", 1.1 - 0.1 * k)
    return p


# --- house -------------------------------------------------------------------

def build_bed():
    """A short bed, the headboard north: pillow, red blanket turned down."""
    p = dk.Piece("bed")
    for x in (-0.4, 0.4):
        post(p, x, 0.42, 0.0, 0.86, 0.07, "wood", 0.8)
        post(p, x, -0.44, 0.0, 0.52, 0.07, "wood", 0.8)
        p.box(x - 0.025, x + 0.025, -0.42, 0.4, 0.14, 0.32, "wood", 0.7)
    p.box(-0.37, 0.37, 0.4, 0.44, 0.3, 0.8, "wood", 0.9)
    p.box(-0.37, 0.37, -0.46, -0.42, 0.3, 0.48, "wood", 0.9)
    p.box(-0.375, 0.375, -0.42, 0.4, 0.26, 0.42, "linen", 0.85, skip=["b"])
    p.box(-0.39, 0.39, -0.42, 0.08, 0.3, 0.46, "cloth", 1.0, skip=["b"])
    p.box(-0.39, 0.39, 0.08, 0.16, 0.3, 0.47, "linen", 1.1, skip=["b"])
    p.box(-0.26, 0.26, 0.2, 0.38, 0.42, 0.52, "linen", 1.15, skip=["b"])
    return p


# --- street ------------------------------------------------------------------

def build_market_stall():
    """A counter of goods under a striped awning on four posts, the counter at the front."""
    p = dk.Piece("market_stall")
    counter = 0.95
    p.box(-0.42, 0.42, -0.38, -0.12, 0.0, counter - 0.05, "wood", 0.8, skip=["b"])
    for k in range(3):
        x = -0.28 + k * 0.28
        p.box(x - 0.005, x + 0.005, -0.385, -0.375, 0.05, counter - 0.1, "wood", 0.5)
    plank(p, -0.45, 0.45, -0.42, -0.1, counter - 0.05, counter)
    for x in (-0.43, 0.43):
        post(p, x, -0.4, 0.0, 1.95, 0.06, "wood", 0.8)
        post(p, x, 0.42, 0.0, 2.15, 0.06, "wood", 0.8)
    # Goods: apples in a basket, a bolt of cloth, a jar.
    drum(p, -0.24, -0.25, counter, counter + 0.1, 0.11, 0.13, 8, "wood", 0.7)
    for dx, dy in ((-0.05, 0.0), (0.05, 0.02), (0.0, -0.06), (0.0, 0.06), (-0.02, 0.0)):
        p.prism(-0.24 + dx, -0.25 + dy, counter + 0.08, counter + 0.15 + (0.03 if dx == -0.02 else 0), 0.045, 0.03, 6, "cloth", 1.1)
    placed(p, lambda: drum(p, 0, 0, -0.14, 0.14, 0.055, 0.055, 6, "linen", 1.0), at=(0.08, -0.26, counter + 0.055), tilt_y=math.pi / 2)
    drum(p, 0.3, -0.25, counter, counter + 0.16, 0.06, 0.045, 6, "stone", 1.0)
    # A crate of stock behind the counter.
    p.box(-0.1, 0.25, 0.05, 0.35, 0.0, 0.32, "wood", 0.9, skip=["b"])
    # The awning slopes from the back posts down to the front, in stripes.
    front, back = (-0.46, 1.9), (0.47, 2.18)
    run = Vector((0.0, back[0] - front[0], back[1] - front[1]))
    slope = math.atan2(run.z, run.y)

    def awning():
        for k in range(6):
            x0 = -0.48 + k * 0.16
            p.box(x0, x0 + 0.16, 0.0, run.length, 0.0, 0.03, "cloth" if k % 2 == 0 else "linen", 1.0)
        for k in range(6):
            x0 = -0.48 + k * 0.16
            p.box(x0, x0 + 0.16, -0.03, 0.0, -0.14, 0.03, "cloth" if k % 2 == 0 else "linen", 0.85)

    placed(p, awning, at=(0, front[0], front[1]), tilt_x=slope)
    return p


def build_lamp_post():
    """An iron post on a stone foot carrying an open lantern. The game's flame
    burns inside the cage at LAMP_FLAME, its base just over the burner."""
    p = dk.Piece("lamp_post")
    floor = LAMP_FLAME - FLAME_BASE - 0.03  # the burner plate under the flame
    p.prism(0, 0, 0.0, 0.22, 0.18, 0.14, 8, "stone", 0.9)
    p.prism(0, 0, 0.22, floor - 0.08, 0.045, 0.04, 6, "iron", 0.8, cap=False)
    drum(p, 0, 0, floor - 0.08, floor, 0.04, 0.12, 6, "iron", 0.75)
    drum(p, 0, 0, floor, floor + 0.02, 0.13, 0.13, 6, "iron", 0.95)
    drum(p, 0, 0, floor + 0.02, floor + 0.05, 0.035, 0.03, 6, "iron", 0.5)
    roof = LAMP_FLAME + 0.35
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        post(p, 0.11 * math.cos(a), 0.11 * math.sin(a), floor + 0.02, roof, 0.022, "iron", 0.9)
    p.prism(0, 0, roof, roof + 0.13, 0.16, 0.04, 4, "iron", 1.0)
    p.face([(0.16 * math.cos(a), 0.16 * math.sin(a), roof) for a in (0, 3 * math.pi / 2, math.pi, math.pi / 2)], "iron", 0.5)
    drum(p, 0, 0, roof + 0.13, roof + 0.2, 0.03, 0.0, 4, "iron", 0.9)
    return p


# --- build -------------------------------------------------------------------

BUILDERS = [build_bear_trap, build_keg, build_table, build_chair, build_candle, build_anvil,
            build_water_trough, build_weapon_rack, build_armour_stand, build_chemistry_set,
            build_dead_thing, build_cabinet, build_bed, build_loom, build_hide_rack,
            build_market_stall, build_lamp_post]


def glow_glass():
    mat = bpy.data.materials.get("glass")
    if not mat:
        return
    bsdf = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Emission Color"].default_value = (*[dk.linear(c) for c in dk.PALETTE["glass"][0]], 1.0)
    bsdf.inputs["Emission Strength"].default_value = GLASS_GLOW


def build_all():
    pieces = [build() for build in BUILDERS]
    for p in pieces:
        p.finish(dk.kit_collection(p.name))
    glow_glass()
    lay_out_preview([p.name for p in pieces])


def lay_out_preview(names):
    """Every prop in a small room, three rows of six, with the game's camera. All in
    `_preview`, which export skips; the props stay at the origin, hidden."""
    for name in ("_cam", "_sun", "_floor"):
        if name in bpy.data.objects:
            bpy.data.objects.remove(bpy.data.objects[name])
    preview = dk.kit_collection("_preview")
    layers = bpy.context.view_layer.layer_collection.children
    for i, name in enumerate(names):
        layers[name].hide_viewport = True
        empty = bpy.data.objects.new(f"_{name}", None)
        empty.instance_type = "COLLECTION"
        empty.instance_collection = bpy.data.collections[name]
        empty.location = ((i % 6) * 1.5, -(i // 6) * 1.5, 0.0)
        preview.objects.link(empty)
    # A room the camera looks into: a plank floor with walls on the north and west sides.
    floor = dk.Piece("_floor")
    floor.box(-0.75, 8.25, -3.75, 0.75, -0.05, 0.0, "wood", 0.6)
    floor.box(-0.75, 8.25, 0.75, 0.95, -0.05, dk.H, "plaster", 0.8)
    floor.box(-0.95, -0.75, -3.75, 0.95, -0.05, dk.H, "plaster", 0.75)
    floor.finish(preview)

    cam = bpy.data.objects.new("_cam", bpy.data.cameras.get("_cam") or bpy.data.cameras.new("_cam"))
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = 10
    yaw, pitch = math.radians(45), math.radians(35)
    look = Vector((math.sin(yaw) * math.cos(pitch), -math.cos(yaw) * math.cos(pitch), math.sin(pitch)))
    cam.location = Vector((3.75, -1.5, 0.6)) + look * 30
    cam.rotation_euler = (-look).to_track_quat("-Z", "Y").to_euler()
    preview.objects.link(cam)
    bpy.context.scene.camera = cam
    sun = bpy.data.objects.new("_sun", bpy.data.lights.get("_sun") or bpy.data.lights.new("_sun", "SUN"))
    sun.data.energy = 3
    sun.data.color = (1.0, 0.85, 0.7)
    sun.rotation_euler = (math.radians(50), 0, math.radians(30))
    preview.objects.link(sun)


if __name__ == "__main__":
    build_all()
