"""Builds the characters kit (ticket A04) procedurally in the open .blend.

Run inside Blender (Text Editor, or the MCP with `__file__` set), then save and
export:
    blender art/characters_kit.blend --background --python art/build_characters_kit.py
rebuilds every character collection from scratch; art/export.py then writes
the .glb files. It reuses the props script (and through it the town and
dungeon helpers and their wood, iron, steel, leather and glass).

Same conventions as the other kits: +X east, +Y north, +Z up, one unit per
metre, each character built at the origin standing on the floor and facing
south (-Y). The character's left hand is on the +X side, the side the game's
camera sees.

Rig-ready (A09): every character is separate objects named
`<model>_<part>` (head, torso, arm_l, arm_r, leg_l, leg_r, and held items),
each with its origin at its joint: the neck, the hips, a shoulder, a hip
joint, and a held item's grip in the hand. The pose is a symmetric neutral
stance, arms hanging a little out from the sides.

Colour: most surfaces (cloth, skin, hair) use the white `dye` material tinted by the vertex
colour (glTF COLOR_0, multiplied into the base colour as in the other kits).
A material written "dye.red" is the `dye` material with the DYES["red"] tint.
The heroes' accent colours match their HUD colours in src/party.rs `HEROES`
(yellow, aqua, lime, fuchsia), a little muted.

`humanoid` and the head, hat and item helpers are meant to be reused for the
monsters (A05).
"""

import importlib.util
import math
from pathlib import Path

import bpy
from mathutils import Vector

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("build_props_kit", HERE / "build_props_kit.py")
pk = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pk)
dk = pk.dk
placed, drum = pk.placed, pk.drum

# A white base the vertex colour tints (cloth, skin, hair), and a glowing violet for the enchanter's orb.
dk.PALETTE.update({
    "dye": ((1.0, 1.0, 1.0), 0.95, 0.0),
    "arcane": ((0.62, 0.40, 0.95), 0.4, 0.0),
})
GLOWS = {"glass": 0.8, "arcane": 2.0}

# sRGB tints (bmesh colours are sRGB).
DYES = {
    "fair": (0.82, 0.62, 0.50), "tan": (0.72, 0.50, 0.36), "brown_skin": (0.52, 0.35, 0.24),
    "hair_brown": (0.36, 0.22, 0.12), "hair_black": (0.13, 0.11, 0.10), "hair_red": (0.66, 0.28, 0.10),
    "hair_grey": (0.70, 0.69, 0.66), "hair_blond": (0.88, 0.72, 0.40),
    # The heroes' identity colours (src/party.rs HEROES), muted a little.
    "yellow": (0.88, 0.66, 0.0), "aqua": (0.10, 0.70, 0.74), "lime": (0.42, 0.78, 0.16), "fuchsia": (0.80, 0.18, 0.68),
    "white": (0.82, 0.80, 0.74), "cream": (0.76, 0.70, 0.55), "brown": (0.42, 0.28, 0.17),
    "dark": (0.17, 0.16, 0.18), "grey": (0.46, 0.46, 0.50), "red": (0.66, 0.16, 0.13),
    "blue": (0.22, 0.32, 0.62), "green": (0.26, 0.44, 0.22), "purple": (0.42, 0.22, 0.58),
    "ochre": (0.78, 0.60, 0.26), "pink": (0.86, 0.46, 0.58), "gold": (0.92, 0.72, 0.24),
    "olive": (0.42, 0.42, 0.20), "eye": (0.05, 0.04, 0.04),
}

# The body, in metres before a character's scale.
HIP = 0.62  # hip joints and the torso's origin
SHOULDER = 1.1  # shoulder joints
SHOULDER_X = 0.27
NECK = 1.14  # the head's origin
HEAD = 0.36  # head height: big, for reading at a distance
HEAD_R = 0.18
LEG_X = 0.1
ARM_LEN = 0.44
ARM_TILT = 0.14  # arms hang this far out from the sides (radians)
DEPTH = 0.62  # torso front-to-back over width


class Part(dk.Piece):
    """A `Piece` with its object origin at `pivot`, and dyed materials."""

    def __init__(self, name, pivot):
        super().__init__(name)
        self.pivot = Vector(pivot)

    def face(self, verts, mat, shade=1.0):
        base, _, dye = mat.partition(".")
        f = super().face(verts, base, shade)
        if dye:
            r, g, b = DYES[dye]
            for loop in f.loops:
                loop[self.col] = (r * shade, g * shade, b * shade, 1.0)
        return f

    def finish(self, collection, scale=1.0):
        for v in self.bm.verts:
            v.co = (v.co - self.pivot) * scale
        obj = super().finish(collection)
        obj.location = self.pivot * scale
        return obj


def octo(p, z0, z1, r0, r1, mat, shade=1.0, at=(0.0, 0.0), depth=1.0, cap=True, bottom=False):
    """An eight-sided frustum with flats facing the four directions, squashed front to back."""
    def build():
        if bottom:
            drum(p, 0, 0, z0, z1, r0, r1, 8, mat, shade)
        else:
            p.prism(0, 0, z0, z1, r0, r1, 8, mat, shade, cap=cap)
    placed(p, build, at=(at[0], at[1], 0.0), yaw=math.pi / 8, scale=(1.0, depth, 1.0))


class Figure:
    """A character's parts: the body joints, plus any held items."""

    def __init__(self, name, scale=1.0):
        self.name = name
        self.scale = scale
        self.parts = {}
        self.part("torso", (0, 0, HIP))
        self.part("head", (0, 0, NECK))
        for side, key in ((1, "l"), (-1, "r")):
            self.part(f"arm_{key}", (side * SHOULDER_X, 0, SHOULDER))
            self.part(f"leg_{key}", (side * LEG_X, 0, HIP))

    def part(self, key, pivot):
        p = Part(f"{self.name}_{key}", pivot)
        self.parts[key] = p
        return p

    def __getitem__(self, key):
        return self.parts[key]

    @staticmethod
    def hand(side):
        """Where a hand's grip is (side 1 left, -1 right)."""
        return Vector((side * (SHOULDER_X + math.sin(ARM_TILT) * ARM_LEN), -0.02, SHOULDER - math.cos(ARM_TILT) * ARM_LEN))

    def item(self, key, side, build, offset=(0.0, 0.0, 0.0), **turn):
        """A held item, built round its grip at the origin, placed in a hand."""
        grip = self.hand(side) + Vector(offset)
        p = self.part(key, grip)
        placed(p, lambda: build(p), at=grip, **turn)
        return p

    def finish(self):
        col = dk.kit_collection(self.name)
        for p in self.parts.values():
            p.finish(col, self.scale)
        return col


# --- body --------------------------------------------------------------------

def humanoid(name, skin, top, legs, boots, sleeves=None, belly=0.0, scale=1.0, bare_arms=False):
    """The shared body: legs and boots, a torso widening to the shoulders, a neck,
    a head with eyes and nose, and arms with big hands."""
    f = Figure(name, scale)
    sleeves = sleeves or top
    for side, key in ((1, "l"), (-1, "r")):
        leg, x = f[f"leg_{key}"], side * LEG_X
        leg.box(x - 0.075, x + 0.075, -0.08, 0.08, 0.15, HIP + 0.04, legs)
        leg.box(x - 0.085, x + 0.085, -0.16, 0.09, 0.0, 0.16, boots, 0.8)
    t = f["torso"]
    octo(t, HIP - 0.08, HIP + 0.14, 0.2, 0.19 + belly, top, 0.9, depth=DEPTH + belly, bottom=True)
    octo(t, HIP + 0.14, SHOULDER, 0.19 + belly, 0.26, top, 1.0, depth=DEPTH + belly * 0.5, cap=False)
    octo(t, SHOULDER, SHOULDER + 0.06, 0.26, 0.15, top, 1.05, depth=DEPTH)
    for side, key in ((1, "l"), (-1, "r")):
        arm = f[f"arm_{key}"]
        sleeve = skin if bare_arms else sleeves

        def build(arm=arm, sleeve=sleeve):
            drum(arm, 0, 0, -ARM_LEN + 0.06, 0.04, 0.062, 0.078, 6, sleeve, 0.95)
            arm.box(-0.07, 0.07, -0.075, 0.075, -ARM_LEN - 0.07, -ARM_LEN + 0.07, skin, 1.0)
        placed(arm, build, at=(side * SHOULDER_X, 0, SHOULDER), tilt_y=-side * ARM_TILT)
    head(f, skin)
    return f


def head(f, skin):
    h = f["head"]
    h.prism(0, 0, NECK - 0.04, NECK + 0.06, 0.075, 0.075, 6, skin, 0.85, cap=False)
    octo(h, NECK + 0.04, NECK + 0.04 + HEAD, HEAD_R, HEAD_R * 0.92, skin, 1.0, depth=0.95, bottom=True)
    front = -HEAD_R * 0.924 * 0.95
    eye = NECK + 0.04 + HEAD * 0.52
    for side in (-1, 1):
        h.box(side * 0.065 - 0.024, side * 0.065 + 0.024, front - 0.012, front + 0.01, eye - 0.035, eye + 0.03, "dye.eye", 1.0, skip=["n"])
    h.box(-0.025, 0.025, front - 0.05, front + 0.01, eye - 0.1, eye - 0.03, skin, 0.9, skip=["n"])


def head_top():
    return NECK + 0.04 + HEAD


def hair(f, colour, back=True, fringe=True):
    """A cap of hair over the crown, down the back of the head."""
    h, top = f["head"], head_top()
    octo(h, top - 0.14, top + 0.04, HEAD_R + 0.015, HEAD_R * 0.7, colour, 1.0, depth=1.0)
    if back:
        h.box(-HEAD_R * 0.85, HEAD_R * 0.85, HEAD_R * 0.55, HEAD_R + 0.02, NECK + 0.08, top - 0.12, colour, 0.85)
    if fringe:
        h.box(-HEAD_R * 0.8, HEAD_R * 0.8, -HEAD_R - 0.01, -HEAD_R + 0.06, top - 0.1, top - 0.02, colour, 1.05)


def beard(f, colour, long=0.0):
    h, front = f["head"], -HEAD_R * 0.88
    chin = NECK + 0.04
    h.box(-0.13, 0.13, front - 0.04, front + 0.08, chin - long - 0.04, chin + 0.12, colour, 0.95)
    h.box(-0.09, 0.09, front - 0.05, front, chin + 0.1, chin + 0.15, colour, 1.0)  # moustache


def apron(f, colour, top=0.98, bottom=0.3, width=0.2):
    """A flat panel hanging down the front of the torso."""
    t = f["torso"]
    front = -0.26 * DEPTH * 0.924 - 0.01
    t.box(-width, width, front - 0.025, front, bottom, top, colour, 1.0)


def back_panel(f, colour, top=SHOULDER, bottom=0.3, width=0.24):
    t = f["torso"]
    back = 0.26 * DEPTH * 0.924
    t.box(-width, width, back, back + 0.03, bottom, top, colour, 0.9)


def mantle(f, colour, r=0.31):
    """A short cape over the shoulders, the part of a character seen best from above."""
    octo(f["torso"], SHOULDER - 0.16, SHOULDER + 0.08, r, 0.15, colour, 1.0, depth=0.72, bottom=True)


def belt(f, colour="leather", z=HIP + 0.1, r=0.205):
    octo(f["torso"], z - 0.04, z + 0.04, r, r, colour, 0.8, depth=DEPTH + 0.03, cap=False)


def skirt(f, colour, bottom=0.06, r0=0.32, top=HIP + 0.2):
    """A robe or dress from the waist down, flaring to the floor."""
    octo(f["torso"], bottom, top, r0, 0.21, colour, 0.95, depth=0.8, bottom=True)


# --- hats --------------------------------------------------------------------

def pointed_hat(f, colour, band=None):
    h, top = f["head"], head_top()
    octo(h, top - 0.08, top - 0.04, 0.33, 0.33, colour, 0.9, bottom=True)
    if band:
        octo(h, top - 0.04, top + 0.04, 0.19, 0.18, band, 1.0, cap=False)
    placed(h, lambda: h.prism(0, 0, 0.0, 0.5, 0.18, 0.0, 8, colour, 1.0, cap=False), at=(0, 0.02, top - 0.04), tilt_x=0.3)


def helm(f, plume):
    h, top = f["head"], head_top()
    octo(h, top - 0.2, top - 0.02, HEAD_R + 0.025, HEAD_R + 0.02, "steel", 1.0, cap=False)
    octo(h, top - 0.02, top + 0.08, HEAD_R + 0.02, 0.08, "steel", 1.1)
    h.box(-0.02, 0.02, -HEAD_R - 0.05, -HEAD_R + 0.02, top - 0.24, top - 0.02, "steel", 0.9)
    h.box(-0.035, 0.035, -0.16, 0.2, top + 0.04, top + 0.2, plume, 1.0)
    h.box(-0.035, 0.035, 0.12, 0.24, top - 0.06, top + 0.12, plume, 0.85)


def hood(f, colour):
    """A hood round the head, open at the front so the face shows, peaked at the back."""
    h, top = f["head"], head_top()
    r, z0, z1 = HEAD_R + 0.04, NECK - 0.02, top + 0.06
    ring = [(r * math.cos(math.pi / 8 + k * math.pi / 4), r * math.sin(math.pi / 8 + k * math.pi / 4)) for k in range(8)]
    for k in range(8):
        (xa, ya), (xb, yb) = ring[k], ring[(k + 1) % 8]
        if (ya + yb) / 2 < -r * 0.5:
            continue  # the open front
        h.face([(xa, ya, z0), (xb, yb, z0), (xb, yb, z1), (xa, ya, z1)], colour, 0.95)
    h.face([(x * 0.6, y * 0.6, z1 + 0.06) for x, y in ring], colour, 1.0)
    for k in range(8):
        (xa, ya), (xb, yb) = ring[k], ring[(k + 1) % 8]
        h.face([(xa, ya, z1), (xb, yb, z1), (xb * 0.6, yb * 0.6, z1 + 0.06), (xa * 0.6, ya * 0.6, z1 + 0.06)], colour, 1.0)
    h.box(-0.12, 0.12, -r * 0.92, -r * 0.6, top - 0.02, z1 + 0.04, colour, 1.05)  # the brow
    placed(h, lambda: h.prism(0, 0, 0.0, 0.22, 0.09, 0.0, 4, colour, 0.9, cap=False), at=(0, r * 0.7, z1 - 0.04), tilt_x=-1.9)


def brimmed_hat(f, colour, band, brim=0.3, crown=0.14, feather=None):
    h, top = f["head"], head_top()
    octo(h, top - 0.07, top - 0.04, brim, brim, colour, 0.9, bottom=True)
    octo(h, top - 0.04, top + crown, HEAD_R * 0.95, HEAD_R * 0.8, colour, 1.0)
    octo(h, top - 0.04, top + 0.02, HEAD_R * 0.97, HEAD_R * 0.96, band, 1.0, cap=False)
    if feather:
        placed(h, lambda: h.box(-0.025, 0.025, -0.06, 0.06, 0.0, 0.32, feather, 1.1), at=(HEAD_R * 0.8, 0.05, top), tilt_y=-0.6, tilt_x=-0.3)


def straw_hat(f, colour):
    h, top = f["head"], head_top()
    octo(h, top - 0.08, top + 0.16, 0.38, 0.03, colour, 1.0, bottom=True)


def cap(f, colour):
    h, top = f["head"], head_top()
    octo(h, top - 0.1, top + 0.05, HEAD_R + 0.02, HEAD_R * 0.75, colour, 1.0)
    h.box(-0.12, 0.12, -HEAD_R - 0.12, -HEAD_R + 0.02, top - 0.1, top - 0.07, colour, 0.8)


def scarf(f, colour):
    """A headscarf tied at the back."""
    h, top = f["head"], head_top()
    octo(h, top - 0.16, top + 0.05, HEAD_R + 0.02, HEAD_R * 0.6, colour, 1.0)
    h.box(-0.05, 0.05, HEAD_R, HEAD_R + 0.08, top - 0.22, top - 0.1, colour, 0.85)


def bun(f, colour):
    hair(f, colour)
    h, top = f["head"], head_top()
    octo(h, top - 0.02, top + 0.12, 0.09, 0.06, colour, 0.95, at=(0, 0.1))


def goggles(f):
    """Brass-rimmed goggles pushed up on the forehead."""
    h, top = f["head"], head_top()
    octo(h, top - 0.13, top - 0.08, HEAD_R + 0.012, HEAD_R + 0.012, "leather", 0.7, cap=False)
    for side in (-1, 1):
        placed(h, lambda: drum(h, 0, 0, 0.0, 0.06, 0.06, 0.055, 8, "dye.gold", 1.0), at=(side * 0.07, -HEAD_R * 0.9, top - 0.1), tilt_x=math.pi / 2)
        placed(h, lambda: h.prism(0, 0, 0.06, 0.065, 0.045, 0.045, 8, "glass", 1.0), at=(side * 0.07, -HEAD_R * 0.9, top - 0.1), tilt_x=math.pi / 2)


# --- worn and held things -----------------------------------------------------

def pauldrons(f, mat="steel"):
    for side, key in ((1, "l"), (-1, "r")):
        arm = f[f"arm_{key}"]
        octo(arm, SHOULDER - 0.1, SHOULDER + 0.06, 0.12, 0.08, mat, 1.05, at=(side * (SHOULDER_X + 0.01), 0), bottom=True)


def sword(p):
    p.box(-0.018, 0.018, -0.018, 0.018, -0.1, 0.06, "leather", 0.7)
    p.box(-0.12, 0.12, -0.025, 0.025, 0.06, 0.1, "iron", 0.9)
    p.box(-0.04, 0.04, -0.01, 0.01, 0.1, 0.78, "steel", 1.15)
    p.prism(0, 0, 0.78, 0.88, 0.04, 0.0, 4, "steel", 1.15, cap=False)


def shield(p, face):
    """A round shield on the arm, its face out to the side and front."""
    def build():
        drum(p, 0, 0, 0.0, 0.04, 0.28, 0.28, 8, "steel", 0.8)
        p.prism(0, 0, 0.04, 0.05, 0.25, 0.25, 8, face, 1.0)
        drum(p, 0, 0, 0.05, 0.09, 0.07, 0.04, 6, "steel", 1.1)
    placed(p, build, yaw=-0.5, tilt_x=math.pi / 2)


def bow(p):
    """A longbow held upright, its belly forward."""
    pts = [(0.0, -0.55), (-0.08, -0.36), (-0.12, 0.0), (-0.08, 0.36), (0.0, 0.55)]
    pts = [(y, z) for y, z in pts]
    for (ya, za), (yb, zb) in zip(pts, pts[1:]):
        mid_y, mid_z = (ya + yb) / 2, (za + zb) / 2
        length = math.hypot(yb - ya, zb - za)
        angle = math.atan2(-(yb - ya), zb - za)
        placed(p, lambda length=length: p.box(-0.02, 0.02, -0.02, 0.02, -length / 2, length / 2, "wood", 1.0), at=(0, mid_y, mid_z), tilt_x=angle)
    p.box(-0.006, 0.006, -0.006, 0.006, -0.55, 0.55, "linen", 1.0)
    p.box(-0.03, 0.03, -0.14, -0.08, -0.06, 0.06, "leather", 0.8)


def quiver(f):
    t = f["torso"]
    back = 0.26 * DEPTH * 0.924 + 0.05

    def build():
        t.prism(0, 0, 0.0, 0.5, 0.07, 0.08, 6, "leather", 0.9)
        for k, (dx, dy) in enumerate(((-0.03, 0.0), (0.03, 0.02), (0.0, -0.03))):
            t.box(dx - 0.006, dx + 0.006, dy - 0.006, dy + 0.006, 0.5, 0.62, "wood", 1.0)
            t.box(dx - 0.004, dx + 0.004, dy - 0.03, dy + 0.03, 0.56, 0.66, "dye.white", 1.1)
    placed(t, build, at=(-0.06, back + 0.04, 0.65), tilt_y=0.4)


def staff(p, top_mat="glass"):
    p.prism(0, 0, -0.66, 0.95, 0.025, 0.025, 6, "wood", 0.9)
    p.prism(0, 0, 0.95, 1.05, 0.025, 0.06, 4, "wood", 0.8, cap=False)
    drum(p, 0, 0, 1.03, 1.17, 0.07, 0.07, 6, top_mat, 1.0)
    p.prism(0, 0, 1.17, 1.21, 0.07, 0.0, 6, top_mat, 1.0, cap=False)


def mace(p):
    p.box(-0.02, 0.02, -0.02, 0.02, -0.1, 0.45, "wood", 0.9)
    drum(p, 0, 0, 0.42, 0.6, 0.07, 0.07, 6, "steel", 1.05)
    for k in range(4):
        placed(p, lambda: p.box(-0.1, 0.1, -0.012, 0.012, 0.43, 0.59, "steel", 1.15), yaw=k * math.pi / 4)
    p.prism(0, 0, 0.6, 0.66, 0.05, 0.0, 6, "steel", 1.1, cap=False)


def hammer(p):
    p.box(-0.022, 0.022, -0.022, 0.022, -0.1, 0.5, "wood", 0.9)
    p.box(-0.06, 0.06, -0.14, 0.12, 0.44, 0.58, "iron", 1.2)


def mug(p):
    drum(p, 0, 0, 0.0, 0.16, 0.06, 0.065, 6, "wood", 1.0)
    p.prism(0, 0, 0.16, 0.2, 0.068, 0.05, 6, "dye.white", 1.1)
    p.box(0.065, 0.1, -0.015, 0.015, 0.03, 0.13, "wood", 0.85)


def book(p):
    p.box(-0.04, 0.04, -0.14, 0.04, -0.04, 0.2, "dye.red", 0.8)
    p.box(-0.032, 0.045, -0.13, 0.03, -0.03, 0.19, "dye.cream", 1.1)


def cloth_bolt(p, colour):
    """A bolt of cloth carried upright against the hip: a roll round a board."""
    drum(p, 0, 0, -0.32, 0.36, 0.1, 0.1, 8, colour, 1.0)
    p.box(-0.02, 0.02, -0.13, 0.13, 0.36, 0.42, "wood", 0.9)


def flask(p):
    drum(p, 0, 0, -0.02, 0.12, 0.09, 0.07, 8, "glass", 1.0)
    p.prism(0, 0, 0.12, 0.22, 0.03, 0.025, 6, "glass", 1.1)
    drum(p, 0, 0, 0.22, 0.26, 0.03, 0.03, 6, "leather", 0.8)


def basket(p):
    drum(p, 0, 0, -0.24, -0.04, 0.12, 0.16, 8, "wood", 0.85)
    p.prism(0, 0, -0.06, -0.03, 0.14, 0.14, 8, "dye.red", 1.0)
    p.box(-0.015, 0.015, -0.13, 0.13, -0.04, 0.06, "wood", 1.0, skip=["b"])


def pitchfork(p):
    p.prism(0, 0, -0.55, 1.05, 0.022, 0.022, 5, "wood", 0.9)
    p.box(-0.13, 0.13, -0.02, 0.02, 1.02, 1.06, "iron", 0.9)
    for x in (-0.12, 0.0, 0.12):
        p.prism(x, 0, 1.06, 1.32, 0.012, 0.004, 4, "iron", 1.0, cap=False)


def orb(p):
    drum(p, 0, 0, 0.0, 0.05, 0.07, 0.09, 6, "dye.gold", 0.9)
    drum(p, 0, 0, 0.05, 0.2, 0.11, 0.11, 8, "arcane", 1.0)
    p.prism(0, 0, 0.2, 0.25, 0.11, 0.05, 8, "arcane", 1.0)


def symbol(f, colour, z=0.86):
    """A sun cross on the front of a tabard."""
    t = f["torso"]
    front = -0.26 * DEPTH * 0.924 - 0.04
    t.box(-0.025, 0.025, front - 0.01, front, z - 0.12, z + 0.12, colour, 1.15)
    t.box(-0.09, 0.09, front - 0.01, front, z + 0.01, z + 0.06, colour, 1.15)


def stole(f, colour):
    """A priest's stole: two strips down the front from the neck."""
    t = f["torso"]
    front = -0.26 * DEPTH * 0.924 - 0.01
    for x in (-0.08, 0.08):
        t.box(x - 0.035, x + 0.035, front - 0.02, front, 0.16, SHOULDER + 0.04, colour, 1.05)


# --- the heroes -----------------------------------------------------------------

def build_hero_fighter():
    """Plate, a yellow tabard and plume, a sword and a yellow shield."""
    f = humanoid("hero_fighter", "dye.fair", "iron", "iron", "iron", sleeves="iron", scale=1.03)
    apron(f, "dye.yellow", top=SHOULDER, bottom=0.3, width=0.21)
    back_panel(f, "dye.yellow", bottom=0.3, width=0.21)
    belt(f)
    pauldrons(f)
    helm(f, "dye.yellow")
    f.item("sword", -1, sword, tilt_x=0.5)
    f.item("shield", 1, lambda p: shield(p, "dye.yellow"), offset=(0.06, -0.08, 0.12))
    return f


def build_hero_ranger():
    """An aqua hood and cloak over leather, a bow and a quiver of arrows."""
    f = humanoid("hero_ranger", "dye.tan", "leather", "dye.brown", "leather", sleeves="dye.green")
    back_panel(f, "dye.aqua", bottom=0.22, width=0.25)
    mantle(f, "dye.aqua")
    belt(f, "dye.brown")
    hair(f, "dye.hair_red", fringe=True)
    hood(f, "dye.aqua")
    quiver(f)
    f.item("bow", 1, bow, offset=(0.02, -0.05, 0.0))
    return f


def build_hero_mage():
    """A long lime robe, a pointed hat, a white beard and a glowing staff."""
    f = humanoid("hero_mage", "dye.fair", "dye.lime", "dye.lime", "leather", sleeves="dye.lime")
    skirt(f, "dye.lime")
    belt(f, "dye.gold", r=0.215)
    beard(f, "dye.hair_grey", long=0.2)
    hair(f, "dye.hair_grey", fringe=False)
    pointed_hat(f, "dye.lime", band="dye.dark")
    f.item("staff", -1, staff, offset=(0.0, -0.04, 0.0))
    return f


def build_hero_cleric():
    """Mail under a fuchsia tabard with a gold sun cross, a fuchsia mantle and a mace."""
    f = humanoid("hero_cleric", "dye.fair", "iron", "iron", "leather", sleeves="iron")
    apron(f, "dye.fuchsia", top=SHOULDER, bottom=0.26, width=0.19)
    back_panel(f, "dye.fuchsia", bottom=0.26, width=0.19)
    mantle(f, "dye.fuchsia", r=0.3)
    belt(f)
    symbol(f, "dye.gold")
    hair(f, "dye.hair_blond")
    f["head"].box(-0.05, 0.05, HEAD_R * 0.5, HEAD_R + 0.07, NECK - 0.2, NECK + 0.2, "dye.hair_blond", 0.9)  # the braid
    f.item("mace", -1, mace, tilt_x=0.35)
    return f


# --- the townsfolk ----------------------------------------------------------------

def build_barkeep():
    """Round, bald and moustached, in a long white apron, a mug of ale in hand."""
    f = humanoid("barkeep", "dye.fair", "dye.red", "dye.brown", "leather", sleeves="dye.cream", belly=0.07, scale=1.0)
    apron(f, "dye.white", top=1.02, bottom=0.2, width=0.22)
    hair(f, "dye.hair_brown", fringe=False)
    f["head"].box(-0.1, 0.1, -HEAD_R * 0.9, -HEAD_R * 0.8, NECK + 0.14, NECK + 0.19, "dye.hair_brown", 1.0)
    octo(f["head"], head_top() - 0.14, head_top() + 0.01, HEAD_R * 0.8, HEAD_R * 0.6, "dye.fair", 1.0)
    f.item("mug", -1, mug, offset=(0.0, -0.06, 0.02))
    return f


def build_patron():
    """A drinker in a green coat under a wide hat with a red feather."""
    f = humanoid("patron", "dye.tan", "dye.green", "dye.brown", "leather", belly=0.03)
    belt(f)
    hair(f, "dye.hair_black", fringe=False)
    beard(f, "dye.hair_black")
    brimmed_hat(f, "dye.dark", "dye.red", feather="dye.red")
    return f


def build_priest():
    """A long white robe with a purple stole, grey tonsure, a red book."""
    f = humanoid("priest", "dye.fair", "dye.white", "dye.white", "leather", sleeves="dye.white")
    skirt(f, "dye.white", r0=0.3)
    stole(f, "dye.purple")
    belt(f, "dye.purple", r=0.215)
    h = f["head"]
    octo(h, head_top() - 0.16, head_top() - 0.07, HEAD_R + 0.015, HEAD_R + 0.005, "dye.hair_grey", 1.0, cap=False)
    h.box(-HEAD_R * 0.85, HEAD_R * 0.85, HEAD_R * 0.55, HEAD_R + 0.02, NECK + 0.1, head_top() - 0.1, "dye.hair_grey", 0.85)
    f.item("book", 1, book, offset=(-0.02, -0.06, 0.06))
    return f


def build_blacksmith():
    """Big and bare-armed in a dark leather apron, a hammer in hand."""
    f = humanoid("blacksmith", "dye.tan", "dye.grey", "dye.brown", "leather", bare_arms=True, belly=0.04, scale=1.08)
    apron(f, "dye.dark", top=1.04, bottom=0.26, width=0.22)
    belt(f, "dye.dark")
    hair(f, "dye.hair_black", fringe=False)
    beard(f, "dye.hair_black", long=0.06)
    f.item("hammer", -1, hammer, tilt_x=0.2)
    return f


def build_clothier():
    """A purple tunic and headscarf, a bolt of red cloth on the hip."""
    f = humanoid("clothier", "dye.fair", "dye.purple", "dye.dark", "leather", sleeves="dye.cream", scale=0.97)
    skirt(f, "dye.purple", bottom=0.32, r0=0.27)
    belt(f, "dye.gold")
    scarf(f, "dye.ochre")
    f.item("cloth_bolt", 1, lambda p: cloth_bolt(p, "dye.red"), offset=(0.06, -0.06, 0.2))
    return f


def build_alchemist():
    """A stained olive coat, brass goggles on the brow, a glowing flask."""
    f = humanoid("alchemist", "dye.fair", "dye.olive", "dye.brown", "leather", scale=0.98)
    skirt(f, "dye.olive", bottom=0.28, r0=0.27)
    belt(f)
    hair(f, "dye.hair_grey", fringe=False)
    goggles(f)
    f.item("flask", -1, flask, offset=(0.0, -0.08, 0.04))
    return f


def build_mom():
    """A long blue dress and a hair bun, a basket on her arm."""
    f = humanoid("mom", "dye.fair", "dye.blue", "dye.blue", "leather", sleeves="dye.blue", scale=0.94)
    skirt(f, "dye.blue", r0=0.33)
    belt(f, "dye.cream", r=0.215)
    bun(f, "dye.hair_brown")
    f.item("basket", 1, basket, offset=(0.06, -0.04, 0.1))
    return f


def build_peasant():
    """A straw hat and plain brown tunic, a pitchfork upright."""
    f = humanoid("peasant", "dye.brown_skin", "dye.ochre", "dye.brown", "leather", sleeves="dye.ochre")
    belt(f, "dye.brown")
    hair(f, "dye.hair_brown", fringe=False)
    straw_hat(f, "dye.cream")
    f.item("pitchfork", -1, pitchfork, offset=(0.0, -0.04, 0.0))
    return f


def build_enchanter():
    """A violet hooded robe and a glowing orb held before her."""
    f = humanoid("enchanter", "dye.fair", "dye.purple", "dye.purple", "leather", sleeves="dye.purple")
    skirt(f, "dye.purple", r0=0.31)
    belt(f, "dye.gold", r=0.215)
    mantle(f, "dye.dark", r=0.3)
    hair(f, "dye.hair_black")
    hood(f, "dye.purple")
    f.item("orb", 1, orb, offset=(-0.06, -0.12, 0.08))
    return f


BUILDERS = [build_hero_fighter, build_hero_ranger, build_hero_mage, build_hero_cleric,
            build_barkeep, build_patron, build_priest, build_blacksmith, build_clothier,
            build_alchemist, build_mom, build_peasant, build_enchanter]


def glow():
    for name, strength in GLOWS.items():
        mat = bpy.data.materials.get(name)
        if not mat:
            continue
        bsdf = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
        bsdf.inputs["Emission Color"].default_value = (*[dk.linear(c) for c in dk.PALETTE[name][0]], 1.0)
        bsdf.inputs["Emission Strength"].default_value = strength


def build_all():
    names = [build().finish().name for build in BUILDERS]
    glow()
    lay_out_preview(names)
    return names


def lay_out_preview(names):
    """Every character in a row of seven and a row of six on a floor, with the
    game's camera. All in `_preview`, which export skips; the characters stay
    at the origin, hidden."""
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
        empty.location = ((i % 7) * 1.0, -(i // 7) * 1.4, 0.0)
        preview.objects.link(empty)
    floor = dk.Piece("_floor")
    floor.box(-0.75, 6.75, -2.25, 0.75, -0.05, 0.0, "stone", 0.6)
    floor.finish(preview)

    cam = bpy.data.objects.new("_cam", bpy.data.cameras.get("_cam") or bpy.data.cameras.new("_cam"))
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = 8
    yaw, pitch = math.radians(45), math.radians(35)
    look = Vector((math.sin(yaw) * math.cos(pitch), -math.cos(yaw) * math.cos(pitch), math.sin(pitch)))
    cam.location = Vector((3.0, -0.7, 0.8)) + look * 30
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
