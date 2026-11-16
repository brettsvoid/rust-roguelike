"""Builds the monsters kit (tickets A05 and A08) procedurally in the open .blend.

Run inside Blender (Text Editor, or the MCP with `__file__` set), then save and
export:
    blender art/monsters_kit.blend --background --python art/build_monsters_kit.py
rebuilds every monster collection from scratch; art/export.py then writes the
.glb files. It reuses the characters script: `humanoid`, `Figure`, the head,
hat and item helpers and the tinted `dye` material.

Same conventions as the characters: one unit per metre, origin on the floor at
the tile centre, facing south (-Y), separate objects per body part with their
origins at the joints (A09 rigs them). Size tells rank: a rat is knee high, an
orc stands over a hero, the demon lord over everything. Colours are saturated
and dark, as the heroes' warm torch washes pale colours out to cream; what
tells monsters apart is mostly the silhouette (ears, tusks, horns, wings,
tails, weapons). Glowing eyes (`ember`, `ghost`) mark the dungeon's worse things.
"""

import importlib.util
import math
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("build_characters_kit", HERE / "build_characters_kit.py")
ck = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ck)
pk, dk = ck.pk, ck.dk
placed, drum, octo, Figure, humanoid = pk.placed, pk.drum, ck.octo, ck.Figure, ck.humanoid
HEAD_R, NECK, HIP, SHOULDER, SHOULDER_X, DEPTH = ck.HEAD_R, ck.NECK, ck.HIP, ck.SHOULDER, ck.SHOULDER_X, ck.DEPTH

dk.PALETTE.update({
    "ember": ((1.0, 0.36, 0.06), 0.5, 0.0),
    "ghost": ((0.40, 1.0, 0.86), 0.5, 0.0),
})
ck.GLOWS.update({"ember": 3.0, "ghost": 3.0})

ck.DYES.update({
    "rat": (0.30, 0.22, 0.18), "rat_tail": (0.78, 0.42, 0.44),
    "kobold": (0.70, 0.33, 0.08), "goblin": (0.30, 0.56, 0.12), "hobgoblin": (0.66, 0.30, 0.06),
    "orc": (0.22, 0.36, 0.16), "ivory": (0.84, 0.78, 0.62), "horn": (0.20, 0.16, 0.13),
    "bone": (0.60, 0.56, 0.46), "rust": (0.48, 0.24, 0.12), "wraith": (0.18, 0.22, 0.30),
    "teal": (0.04, 0.38, 0.34), "imp": (0.66, 0.08, 0.08), "hound": (0.22, 0.06, 0.05),
    "demon": (0.50, 0.07, 0.05), "membrane": (0.26, 0.05, 0.05), "bat": (0.42, 0.28, 0.10),
    "bat_wing": (0.24, 0.15, 0.08), "spider": (0.22, 0.28, 0.09), "spider_leg": (0.12, 0.13, 0.06),
    "blood_red": (0.56, 0.07, 0.06), "fur": (0.25, 0.17, 0.11),
})

EYE = NECK + 0.04 + ck.HEAD * 0.52  # eye height on a humanoid head
FRONT = -HEAD_R * 0.924 * 0.95  # the front of a humanoid face
TOP = ck.head_top()


class Beast(Figure):
    """A `Figure` with no parts yet, for bodies that are not humanoid."""

    def __init__(self, name, scale=1.0):
        self.name, self.scale, self.parts = name, scale, {}


# --- shape helpers -------------------------------------------------------------

def rod(p, a, b, r0, r1, mat, shade=1.0, sides=6, closed=False, squash=1.0):
    """A prism from point a (radius r0) to point b (radius r1), flattened on its local y by `squash`."""
    a, b = Vector(a), Vector(b)
    d = b - a
    start = len(p.bm.faces)
    if closed:
        drum(p, 0, 0, 0.0, d.length, r0, r1, sides, mat, shade)
    else:
        p.prism(0, 0, 0.0, d.length, r0, r1, sides, mat, shade, cap=r1 > 0)
    m = (Matrix.Translation(a) @ Vector((0, 0, 1)).rotation_difference(d).to_matrix().to_4x4()
         @ Matrix.Diagonal((1.0, squash, 1.0, 1.0)))
    for f in list(p.bm.faces)[start:]:
        for v in f.verts:
            v.co = m @ v.co


def plate(p, outline, mat, shade=1.0, thick=0.02):
    """A thin flat sheet cut to an (x, y) outline, from z 0 to `thick` (wings)."""
    p.face([(x, y, thick) for x, y in outline], mat, shade)
    p.face([(x, y, 0.0) for x, y in reversed(outline)], mat, shade * 0.7)
    for (xa, ya), (xb, yb) in zip(outline, outline[1:] + outline[:1]):
        p.face([(xa, ya, 0.0), (xb, yb, 0.0), (xb, yb, thick), (xa, ya, thick)], mat, shade * 0.8)


def shift(part, d):
    """Moves a part's faces and joint together."""
    d = Vector(d)
    for v in part.bm.verts:
        v.co += d
    part.pivot += d


def glowing_eyes(f, mat, z=EYE, front=FRONT, size=0.03):
    for side in (-1, 1):
        f["head"].box(side * 0.065 - size, side * 0.065 + size, front - 0.02, front + 0.01, z - size, z + size * 0.8, mat, 1.0, skip=["n"])


def ears(f, skin, length=0.26, rise=0.12):
    for side in (-1, 1):
        rod(f["head"], (side * HEAD_R * 0.85, 0.02, EYE), (side * (HEAD_R + length), 0.08, EYE + rise), 0.07, 0.0, skin, 1.0, sides=4, squash=0.35)


def tusks(f):
    for side in (-1, 1):
        rod(f["head"], (side * 0.07, FRONT + 0.01, NECK + 0.08), (side * 0.09, FRONT - 0.04, NECK + 0.22), 0.025, 0.0, "dye.ivory", 1.1, sides=4)


def brow(f, skin):
    f["head"].box(-0.15, 0.15, FRONT - 0.05, FRONT + 0.04, EYE + 0.03, EYE + 0.08, skin, 0.75)


def horns(f, mat="dye.horn", size=1.0, out=0.12):
    for side in (-1, 1):
        base = Vector((side * 0.1, 0.02, TOP - 0.06))
        mid = base + Vector((side * out, 0.03, 0.12)) * size
        tip = mid + Vector((side * 0.03, 0.08, 0.16)) * size
        rod(f["head"], base, mid, 0.05 * size, 0.035 * size, mat, 1.0, sides=5)
        rod(f["head"], mid, tip, 0.035 * size, 0.0, mat, 1.1, sides=5)


def tail(f, start, end, r0, mat, tip=None):
    t = f.part("tail", start)
    rod(t, start, end, r0, r0 * 0.3, mat, 0.9, sides=5)
    if tip:
        rod(t, end, Vector(end) + Vector((0, 0.1, 0.05)), 0.06, 0.0, tip, 1.0, sides=4, squash=0.3)
    return t


def back_wings(f, outline, mat, size=1.0, z=SHOULDER - 0.05):
    """Two upright wings on the back, swept back, each a part jointed at the spine."""
    back = 0.26 * DEPTH * 0.924
    for side, key in ((1, "l"), (-1, "r")):
        pts = [(side * x * size, y * size) for x, y in (outline if side > 0 else reversed(outline))]
        w = f.part(f"wing_{key}", (side * 0.05, back, z))
        placed(w, lambda w=w, pts=pts: plate(w, pts, mat), at=(side * 0.05, back, z), tilt_x=math.pi / 2, yaw=side * 0.5)


WING = [(0.0, -0.1), (0.0, 0.15), (0.3, 0.45), (0.75, 0.65), (0.85, 0.55), (0.7, 0.2),
        (0.62, -0.15), (0.45, 0.0), (0.3, -0.25), (0.15, -0.05)]


def iron_cap(f, mat="iron"):
    octo(f["head"], TOP - 0.16, TOP + 0.06, HEAD_R + 0.025, HEAD_R * 0.55, mat, 1.0)


def lowered(build, z):
    """A held item built from its foot, moved so the hand holds it `z` up the shaft."""
    return lambda p: placed(p, lambda: build(p), at=(0, 0, -z))


def big_axe(p):
    p.prism(0, 0, -0.35, 0.95, 0.03, 0.03, 5, "wood", 0.9)
    p.box(-0.02, 0.02, -0.04, 0.04, 0.6, 0.95, "iron", 0.9)
    for side in (-1, 1):
        p.face([(side * 0.02, 0.0, 0.62), (side * 0.32, 0.0, 0.5), (side * 0.32, 0.0, 1.05), (side * 0.02, 0.0, 0.92)], "steel", 1.0)
        p.face([(side * 0.02, 0.0, 0.92), (side * 0.32, 0.0, 1.05), (side * 0.32, 0.0, 0.5), (side * 0.02, 0.0, 0.62)], "steel", 0.8)


def flame_sword(p):
    p.box(-0.022, 0.022, -0.022, 0.022, -0.12, 0.08, "leather", 0.6)
    p.box(-0.16, 0.16, -0.03, 0.03, 0.08, 0.13, "iron", 0.9)
    p.box(-0.055, 0.055, -0.015, 0.015, 0.13, 0.95, "ember", 1.0)
    p.prism(0, 0, 0.95, 1.08, 0.055, 0.0, 4, "ember", 1.0, cap=False)


def crown(f):
    h = f["head"]
    octo(h, TOP - 0.1, TOP - 0.02, HEAD_R + 0.03, HEAD_R + 0.03, "dye.gold", 1.0, cap=False)
    for k in range(5):
        a = math.pi / 2 + (k - 2) * 0.5
        x, y = math.cos(a) * (HEAD_R + 0.02), -math.sin(a) * (HEAD_R + 0.02)
        rod(h, (x, y, TOP - 0.03), (x * 1.1, y * 1.1, TOP + 0.12), 0.035, 0.0, "dye.gold", 1.15, sides=4)


# --- humanoids --------------------------------------------------------------------

def build_kobold():
    """Small and rust orange: a snout, little horns, a tail and a spear."""
    f = humanoid("kobold", "dye.kobold", "dye.kobold", "dye.kobold", "dye.kobold", scale=0.66)
    ck.apron(f, "leather", top=HIP + 0.12, bottom=0.36, width=0.16)
    ck.belt(f)
    f["head"].box(-0.08, 0.08, FRONT - 0.16, FRONT + 0.02, EYE - 0.14, EYE - 0.03, "dye.kobold", 0.95)
    glowing_eyes(f, "ember", size=0.025)
    horns(f, size=0.6, out=0.04)
    tail(f, (0, 0.1, HIP + 0.02), (0, 0.6, 0.06), 0.07, "dye.kobold")
    f.item("spear", -1, lowered(pk.spear, 0.55))
    return f


def build_goblin():
    """Small, hunched and bright green: huge ears, a long nose, a rag tunic and a dagger."""
    f = humanoid("goblin", "dye.goblin", "dye.fur", "dye.dark", "leather", sleeves="dye.goblin", scale=0.8)
    ck.belt(f)
    ears(f, "dye.goblin", length=0.3, rise=0.14)
    f["head"].box(-0.03, 0.03, FRONT - 0.12, FRONT, EYE - 0.12, EYE - 0.02, "dye.goblin", 1.05)
    glowing_eyes(f, "ember", size=0.025)
    shift(f["head"], (0, -0.06, -0.06))
    f.item("dagger", -1, lambda p: placed(p, lambda: ck.sword(p), scale=(0.8, 0.8, 0.55)), tilt_x=0.6)
    return f


def build_hobgoblin():
    """A hero's height, burnt orange, in an iron cap and blue tunic with a spear and shield."""
    f = humanoid("hobgoblin", "dye.hobgoblin", "dye.blue", "dye.dark", "leather", sleeves="dye.hobgoblin")
    ck.belt(f)
    ears(f, "dye.hobgoblin", length=0.18, rise=0.08)
    brow(f, "dye.hobgoblin")
    iron_cap(f)
    f.item("spear", -1, lowered(pk.spear, 0.5))
    f.item("shield", 1, lambda p: ck.shield(p, "dye.dark"), offset=(0.06, -0.08, 0.12))
    return f


def build_orc():
    """Big, dark green and bare armed: tusks, a heavy brow, a topknot, an axe."""
    f = humanoid("orc", "dye.orc", "leather", "dye.fur", "leather", belly=0.06, bare_arms=True, scale=1.15)
    ck.belt(f, "dye.dark")
    brow(f, "dye.orc")
    tusks(f)
    ears(f, "dye.orc", length=0.08, rise=0.04)
    octo(f["head"], TOP - 0.03, TOP + 0.14, 0.06, 0.03, "dye.hair_black", 1.0, at=(0, 0.05))
    f.item("axe", -1, lowered(pk.axe, 0.25), tilt_x=0.3)
    return f


def build_orc_warrior():
    """The biggest orc: iron mail, spiked pauldrons, a horned helm, a red tabard, a great axe."""
    f = humanoid("orc_warrior", "dye.orc", "iron", "dye.fur", "iron", belly=0.04, bare_arms=True, scale=1.28)
    ck.apron(f, "dye.blood_red", top=SHOULDER, bottom=0.24, width=0.2)
    ck.back_panel(f, "dye.blood_red", bottom=0.24, width=0.2)
    ck.belt(f, "dye.dark")
    ck.pauldrons(f, "iron")
    for side, key in ((1, "l"), (-1, "r")):
        rod(f[f"arm_{key}"], (side * (SHOULDER_X + 0.05), 0, SHOULDER + 0.04), (side * (SHOULDER_X + 0.1), 0, SHOULDER + 0.2), 0.04, 0.0, "steel", 1.1, sides=4)
    tusks(f)
    iron_cap(f)
    horns(f, "dye.ivory", size=0.9, out=0.14)
    f.item("axe", -1, big_axe, tilt_x=0.3)
    f.item("shield", 1, lambda p: ck.shield(p, "dye.blood_red"), offset=(0.06, -0.08, 0.12))
    return f


def build_orc_shaman():
    """A robed orc in purple and fur, a skull on its head, a staff with a glowing violet orb."""
    f = humanoid("orc_shaman", "dye.orc", "dye.purple", "dye.purple", "leather", sleeves="dye.purple", scale=1.1)
    ck.skirt(f, "dye.purple", r0=0.3)
    ck.mantle(f, "dye.fur", r=0.32)
    ck.belt(f, "dye.bone")
    tusks(f)
    brow(f, "dye.orc")
    h = f["head"]
    octo(h, TOP - 0.06, TOP + 0.14, HEAD_R * 0.85, HEAD_R * 0.6, "dye.bone", 1.0, at=(0, 0.02))
    for side in (-1, 1):
        h.box(side * 0.05 - 0.025, side * 0.05 + 0.025, -HEAD_R * 0.8 - 0.01, -HEAD_R * 0.8 + 0.01, TOP + 0.03, TOP + 0.08, "dye.eye", 1.0)
        rod(h, (side * 0.09, 0.03, TOP + 0.06), (side * 0.2, 0.08, TOP + 0.28), 0.03, 0.0, "dye.ivory", 1.0, sides=4)
    f.item("staff", -1, lambda p: ck.staff(p, "arcane"), offset=(0.0, -0.04, 0.0))
    return f


def build_skeleton():
    """Bare bones: a ribcage, thin limbs, a skull with glowing eyes, a rusty sword and shield."""
    f = Figure("skeleton")
    t, h = f["torso"], f["head"]
    t.box(-0.13, 0.13, -0.06, 0.06, HIP - 0.06, HIP + 0.04, "dye.bone", 0.9)
    rod(t, (0, 0.04, HIP), (0, 0.04, SHOULDER), 0.035, 0.035, "dye.bone", 0.8, sides=4)
    for k, z in enumerate((0.8, 0.9, 1.0)):
        octo(t, z - 0.03, z + 0.02, 0.15 + k * 0.02, 0.16 + k * 0.02, "dye.bone", 1.0, depth=0.7, cap=False)
    t.box(-0.26, 0.26, -0.04, 0.04, SHOULDER - 0.02, SHOULDER + 0.03, "dye.bone", 1.0)
    for side, key in ((1, "l"), (-1, "r")):
        leg, x = f[f"leg_{key}"], side * 0.1
        rod(leg, (x, 0, HIP), (x, -0.02, 0.33), 0.035, 0.03, "dye.bone", 0.95, sides=4)
        rod(leg, (x, -0.02, 0.33), (x, 0, 0.06), 0.03, 0.025, "dye.bone", 0.95, sides=4)
        leg.box(x - 0.045, x + 0.045, -0.12, 0.04, 0.0, 0.06, "dye.bone", 0.85)
        arm, hand = f[f"arm_{key}"], Figure.hand(side)
        shoulder = Vector((side * SHOULDER_X, 0, SHOULDER))
        rod(arm, shoulder, hand + Vector((0, 0, 0.05)), 0.03, 0.025, "dye.bone", 0.95, sides=4)
        arm.box(hand.x - 0.04, hand.x + 0.04, -0.05, 0.03, hand.z - 0.06, hand.z + 0.05, "dye.bone", 1.0)
    rod(h, (0, 0.02, NECK - 0.04), (0, 0.02, NECK + 0.06), 0.03, 0.03, "dye.bone", 0.8, sides=4)
    octo(h, NECK + 0.12, TOP - 0.02, HEAD_R * 0.95, HEAD_R * 0.8, "dye.bone", 1.0, depth=1.05, bottom=True)
    h.box(-0.1, 0.1, -HEAD_R * 0.8, HEAD_R * 0.3, NECK + 0.04, NECK + 0.12, "dye.bone", 0.8)  # the jaw
    glowing_eyes(f, "ember", z=EYE + 0.02, front=-HEAD_R * 0.95 * 0.924 * 1.05, size=0.03)
    f.item("sword", -1, lambda p: placed(p, lambda: ck.sword(p), scale=(1.0, 1.0, 0.85)), tilt_x=0.5)
    f.item("shield", 1, lambda p: ck.shield(p, "dye.rust"), offset=(0.06, -0.08, 0.12))
    return f


def build_wraith():
    """A floating hooded shade in slate rags, no legs, two glowing eyes in the dark of the hood."""
    f = humanoid("wraith", "dye.dark", "dye.wraith", "dye.wraith", "dye.wraith", scale=1.1)
    del f.parts["leg_l"], f.parts["leg_r"]
    t = f["torso"]
    octo(t, 0.0, HIP + 0.24, 0.07, 0.25, "dye.wraith", 0.85, depth=0.8, bottom=True)
    for k in range(5):
        a = math.pi * (0.15 + 0.7 * k / 4)
        x, y = math.cos(a) * 0.2, math.sin(a) * 0.16
        rod(t, (x, y, 0.3), (x * 1.3, y * 1.6 + 0.12, -0.12), 0.06, 0.0, "dye.wraith", 0.7, sides=4, squash=0.5)
    ck.mantle(f, "dye.wraith", r=0.33)
    ck.hood(f, "dye.wraith")
    glowing_eyes(f, "ghost", size=0.03)
    for side, key in ((1, "l"), (-1, "r")):
        hand = Figure.hand(side)
        for dx in (-0.04, 0.0, 0.04):
            rod(f[f"arm_{key}"], hand + Vector((dx, -0.03, -0.04)), hand + Vector((dx * 1.5, -0.1, -0.2)), 0.015, 0.0, "dye.bone", 1.0, sides=3)
    for p in f.parts.values():
        shift(p, (0, 0, 0.3))
    return f


def build_lich():
    """A skull under a gold crown, a dark teal robe, glowing eyes and a staff with a glowing orb."""
    f = humanoid("lich", "dye.bone", "dye.teal", "dye.teal", "dye.dark", sleeves="dye.teal", scale=1.12)
    ck.skirt(f, "dye.teal", r0=0.33)
    ck.mantle(f, "dye.dark", r=0.33)
    ck.belt(f, "dye.gold", r=0.215)
    f["head"].box(-0.09, 0.09, FRONT - 0.005, FRONT + 0.04, NECK + 0.04, NECK + 0.11, "dye.dark", 1.0)  # the grin
    glowing_eyes(f, "ghost", size=0.032)
    crown(f)
    f.item("staff", -1, lambda p: ck.staff(p, "ghost"), offset=(0.0, -0.04, 0.0))
    return f


def build_imp():
    """Small and crimson: horns, bat wings, a spade-tipped tail and a trident."""
    f = humanoid("imp", "dye.imp", "dye.imp", "dye.imp", "dye.dark", scale=0.62)
    ears(f, "dye.imp", length=0.12, rise=0.1)
    horns(f, "dye.horn", size=0.9, out=0.06)
    glowing_eyes(f, "ember", size=0.03)
    back_wings(f, WING, "dye.membrane", size=0.85)
    tail(f, (0, 0.1, HIP), (0, 0.55, 0.15), 0.05, "dye.imp", tip="dye.imp")
    f.item("trident", -1, ck.pitchfork, offset=(0.0, -0.04, 0.0))
    return f


def build_demon_lord():
    """Huge, blood red and horned, black iron on the chest, great wings and a burning sword."""
    f = humanoid("demon_lord", "dye.demon", "iron", "dye.demon", "dye.dark", belly=0.02, bare_arms=True, scale=1.55)
    ck.pauldrons(f, "iron")
    ck.belt(f, "dye.gold")
    ck.apron(f, "dye.dark", top=HIP + 0.12, bottom=0.25, width=0.17)
    horns(f, "dye.horn", size=1.5, out=0.16)
    brow(f, "dye.demon")
    glowing_eyes(f, "ember", size=0.03)
    back_wings(f, WING, "dye.membrane", size=1.05, z=SHOULDER)
    tail(f, (0, 0.12, HIP), (0.15, 0.7, 0.08), 0.06, "dye.demon", tip="dye.horn")
    f.item("sword", -1, flame_sword, tilt_x=0.45)
    return f


# --- beasts -----------------------------------------------------------------------

def quadruped(name, coat, leg, length, girth, scale=1.0):
    """Four legs, a body lying along y, and a tail: the rat and the hellhound.
    The head is left to the caller, jointed at the front of the body."""
    f = Beast(name, scale)
    z = leg + girth * 0.7
    body = f.part("body", (0, 0, z))
    rod(body, (0, length / 2, z), (0, -length / 2, z + girth * 0.15), girth, girth * 0.8, coat, 1.0, sides=8, closed=True, squash=0.85)
    for (sx, sy), key in (((1, -1), "fl"), ((-1, -1), "fr"), ((1, 1), "bl"), ((-1, 1), "br")):
        x, y = sx * girth * 0.6, sy * length * 0.36
        p = f.part(f"leg_{key}", (x, y, z))
        rod(p, (x, y, z), (x, y - 0.02, 0.0), girth * 0.32, girth * 0.18, coat, 0.85, sides=5)
    f.part("head", (0, -length / 2, z + girth * 0.15))
    return f, z


def build_rat():
    """Knee high, a giant of its kind: a fat brown body, a pointed snout, pink ears and a long pink tail."""
    f, z = quadruped("rat", "dye.rat", 0.1, 0.42, 0.13, scale=1.5)
    h, neck = f["head"], f["head"].pivot
    rod(h, neck + Vector((0, 0.05, 0)), neck + Vector((0, -0.24, -0.04)), 0.11, 0.02, "dye.rat", 1.0, sides=6, closed=True)
    for side in (-1, 1):
        rod(h, neck + Vector((side * 0.05, 0.0, 0.06)), neck + Vector((side * 0.1, 0.03, 0.14)), 0.05, 0.035, "dye.rat_tail", 1.0, sides=6, squash=0.3)
        h.box(side * 0.05 - 0.018, side * 0.05 + 0.018, neck.y - 0.09, neck.y - 0.06, neck.z + 0.02, neck.z + 0.05, "ember", 1.0)
    tail(f, (0, 0.2, z), (0, 0.72, 0.03), 0.035, "dye.rat_tail")
    return f


def build_hellhound():
    """A black-red hound as tall as a hero's waist: glowing eyes and jaws, a ridge of horn spikes."""
    f, z = quadruped("hellhound", "dye.hound", 0.42, 0.72, 0.24)
    h, neck = f["head"], f["head"].pivot
    rod(h, neck + Vector((0, 0.06, -0.04)), neck + Vector((0, -0.12, 0.16)), 0.13, 0.12, "dye.hound", 1.0, sides=6, closed=True)
    h.box(-0.08, 0.08, neck.y - 0.42, neck.y - 0.08, neck.z + 0.04, neck.z + 0.16, "dye.hound", 0.95)  # the muzzle
    h.box(-0.065, 0.065, neck.y - 0.4, neck.y - 0.1, neck.z + 0.01, neck.z + 0.04, "ember", 1.0)  # the jaws
    h.box(-0.07, 0.07, neck.y - 0.4, neck.y - 0.12, neck.z - 0.04, neck.z + 0.01, "dye.hound", 0.8)
    for side in (-1, 1):
        h.box(side * 0.07 - 0.03, side * 0.07 + 0.03, neck.y - 0.2, neck.y - 0.17, neck.z + 0.18, neck.z + 0.22, "ember", 1.0)
        rod(h, neck + Vector((side * 0.08, -0.02, 0.24)), neck + Vector((side * 0.12, 0.12, 0.4)), 0.05, 0.0, "dye.horn", 1.0, sides=4)
    body = f["body"]
    for k in range(4):
        y = 0.3 - k * 0.2
        rod(body, (0, y, z + 0.14), (0, y + 0.08, z + 0.34 - k * 0.02), 0.05, 0.0, "dye.horn", 1.05, sides=4)
    tail(f, (0, 0.38, z + 0.05), (0, 0.8, z + 0.2), 0.05, "dye.hound", tip="ember")
    return f


def build_giant_bat():
    """Hovering at head height on wide brown wings, big ears, glowing eyes."""
    f = Beast("giant_bat", scale=1.5)
    z = 0.72
    body = f.part("body", (0, 0, z))
    octo(body, z - 0.16, z + 0.1, 0.06, 0.11, "dye.bat", 1.0, depth=0.9, bottom=True)
    for side in (-1, 1):
        rod(body, (side * 0.04, 0.0, z - 0.14), (side * 0.06, 0.04, z - 0.26), 0.02, 0.01, "dye.bat_wing", 0.8, sides=3)
    h = f.part("head", (0, 0, z + 0.1))
    octo(h, z + 0.08, z + 0.24, 0.09, 0.07, "dye.bat", 1.05, depth=0.95, bottom=True)
    h.box(-0.04, 0.04, -0.13, -0.06, z + 0.1, z + 0.16, "dye.bat", 0.9)
    for side in (-1, 1):
        rod(h, (side * 0.05, 0.0, z + 0.2), (side * 0.1, 0.02, z + 0.38), 0.05, 0.0, "dye.bat", 1.0, sides=4, squash=0.35)
        h.box(side * 0.035 - 0.015, side * 0.035 + 0.015, -0.1, -0.08, z + 0.16, z + 0.19, "ember", 1.0)
    outline = [(0.0, -0.06), (0.25, -0.12), (0.55, -0.08), (0.72, 0.04), (0.56, 0.06), (0.48, 0.18),
               (0.34, 0.1), (0.22, 0.2), (0.1, 0.1), (0.0, 0.08)]
    for side, key in ((1, "l"), (-1, "r")):
        pts = [(side * x, y) for x, y in (outline if side > 0 else reversed(outline))]
        w = f.part(f"wing_{key}", (side * 0.06, 0, z))
        placed(w, lambda w=w, pts=pts: plate(w, pts, "dye.bat_wing"), at=(side * 0.06, 0, z - 0.02), tilt_y=-side * 0.35)
        for x, y in ((0.55, -0.08), (0.72, 0.04), (0.48, 0.18), (0.22, 0.2)):
            rod(w, (side * 0.06, 0, z), (side * (0.06 + x * math.cos(0.35)), y, z + x * math.sin(0.35) + 0.02), 0.015, 0.008, "dye.bat", 1.1, sides=3)
    return f


def build_giant_spider():
    """Low and wide: eight bent black legs, a swollen olive abdomen with a red mark, glowing eyes."""
    f = Beast("giant_spider")
    z = 0.32
    body = f.part("body", (0, 0, z))
    rod(body, (0, 0.04, z), (0, -0.24, z + 0.02), 0.14, 0.11, "dye.spider", 0.9, sides=8, closed=True, squash=0.8)
    rod(body, (0, 0.02, z + 0.04), (0, 0.24, z + 0.14), 0.12, 0.27, "dye.spider", 1.0, sides=8, closed=True, squash=0.85)
    rod(body, (0, 0.24, z + 0.14), (0, 0.58, z + 0.1), 0.27, 0.08, "dye.spider", 1.0, sides=8, closed=True, squash=0.85)
    top = z + 0.14 + 0.27 * 0.85
    body.face([(0, 0.16, top + 0.005), (0.07, 0.26, top + 0.005), (-0.07, 0.26, top + 0.005)], "dye.blood_red", 1.1)
    body.face([(-0.07, 0.3, top + 0.005), (0.07, 0.3, top + 0.005), (0, 0.4, top - 0.01)], "dye.blood_red", 1.1)
    for side in (-1, 1):
        body.box(side * 0.05 - 0.025, side * 0.05 + 0.025, -0.36, -0.33, z + 0.04, z + 0.08, "ember", 1.0)
        body.box(side * 0.025 - 0.015, side * 0.025 + 0.015, -0.355, -0.335, z + 0.1, z + 0.12, "ember", 1.0)
        rod(body, (side * 0.04, -0.33, z - 0.02), (side * 0.03, -0.4, z - 0.14), 0.025, 0.0, "dye.spider_leg", 1.0, sides=4)
    for side, key in ((1, "l"), (-1, "r")):
        for k, deg in enumerate((-60, -25, 12, 45)):
            a = math.radians(deg)
            d = Vector((side * math.cos(a), math.sin(a), 0))
            hip = Vector((0, -0.1, z)) + d * 0.12
            knee = Vector((0, -0.1, 0)) + d * 0.48 + Vector((0, 0, 0.58))
            foot = Vector((0, -0.1, 0)) + d * 0.72
            leg = f.part(f"leg_{key}{k + 1}", hip)
            rod(leg, hip, knee, 0.035, 0.028, "dye.spider_leg", 1.0, sides=4)
            rod(leg, knee, foot, 0.028, 0.008, "dye.spider_leg", 0.9, sides=4)
    return f


BUILDERS = [build_rat, build_kobold, build_goblin, build_hobgoblin, build_orc, build_orc_warrior,
            build_orc_shaman, build_skeleton, build_wraith, build_lich, build_imp, build_hellhound,
            build_demon_lord, build_giant_bat, build_giant_spider]


def build_all():
    names = [build().finish().name for build in BUILDERS]
    ck.glow()
    lay_out_preview(names)
    return names


def lay_out_preview(names):
    """The characters' preview (floor, `_cam`, `_sun`), respaced to rows of five for the bigger monsters."""
    ck.lay_out_preview(names)
    for i, name in enumerate(names):
        bpy.data.objects[f"_{name}"].location = ((i % 5) * 1.6, -(i // 5) * 1.8, 0.0)
    floor = bpy.data.objects["_floor"]
    floor.scale = (1.25, 2.4, 1.0)
    floor.location = (-0.1, -0.6, 0.0)
    cam = bpy.data.objects["_cam"]
    cam.data.ortho_scale = 10
    cam.location += Vector((0.2, -1.2, 0.0))


if __name__ == "__main__":
    build_all()
