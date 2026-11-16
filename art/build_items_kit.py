"""Builds the items kit (tickets A06 and A08) procedurally in the open .blend.

Run inside Blender (Text Editor, or the MCP with `__file__` set), then save and
export:
    blender art/items_kit.blend --background --python art/build_items_kit.py
rebuilds every item collection from scratch; art/export.py then writes the
.glb files. It reuses the monsters script and through it the characters, props,
town and dungeon helpers: the held weapons (`sword`, `bow`, `staff`, `mace`,
`hammer`, `shield`, `axe`, `big_axe`), the tinted `dye` material and the
glowing materials.

One model per raws item, named for the item in snake case ("Alchemist's Fire"
is `alchemists_fire`), plus `heart_of_the_abyss`. Same conventions as the other
kits: one unit per metre, origin at the tile centre on the floor. An item lies
on the floor: long things lie along the north-east diagonal, across the game
camera's view, and shields lean towards the camera. Everything is drawn larger
than life (0.4 to 0.9 m) and in saturated colours, as the heroes' torch washes
pale colours out to cream. Potions, the Heart and magic trims glow faintly.
"""

import importlib.util
import math
from pathlib import Path

import bpy
from mathutils import Vector

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("build_monsters_kit", HERE / "build_monsters_kit.py")
mk = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(mk)
ck, pk, dk = mk.ck, mk.pk, mk.dk
placed, drum, octo, rod, plate = pk.placed, pk.drum, ck.octo, mk.rod, mk.plate

dk.PALETTE.update({
    # Brighter and not metallic, here only: with nothing to reflect, metal reads near black on the floor.
    "steel": ((0.74, 0.77, 0.82), 0.45, 0.0),
    "gold": ((1.0, 0.72, 0.16), 0.4, 0.3),
    "mithril": ((0.45, 0.88, 1.0), 0.3, 0.3),
    "holy": ((1.0, 0.88, 0.45), 0.5, 0.0),
    "portal": ((0.20, 0.55, 1.0), 0.4, 0.0),
    "heart": ((0.80, 0.0, 0.06), 0.25, 0.0),
    "potion_red": ((0.90, 0.06, 0.10), 0.3, 0.0),
    "potion_pink": ((1.0, 0.10, 0.50), 0.3, 0.0),
    "potion_magenta": ((0.85, 0.05, 0.75), 0.3, 0.0),
    "potion_violet": ((0.62, 0.25, 1.0), 0.3, 0.0),
    "potion_fire": ((1.0, 0.36, 0.02), 0.3, 0.0),
    "potion_holy": ((0.15, 0.70, 1.0), 0.3, 0.0),
    "potion_gold": ((1.0, 0.80, 0.10), 0.3, 0.0),
})
ck.GLOWS.update({
    "gold": 0.25, "mithril": 0.5, "holy": 0.8, "portal": 2.0, "heart": 1.5,
    "potion_red": 0.7, "potion_pink": 0.7, "potion_magenta": 0.7, "potion_violet": 0.7,
    "potion_fire": 1.2, "potion_holy": 0.8, "potion_gold": 1.0,
})
ck.DYES.update({
    "parchment": (0.86, 0.68, 0.38), "cyan": (0.05, 0.66, 0.82), "orange": (0.95, 0.45, 0.05),
    "sky": (0.25, 0.62, 0.98), "cornflower": (0.25, 0.40, 0.92), "tomato": (0.92, 0.24, 0.12), "royal": (0.10, 0.36, 0.95),
    "saddle": (0.50, 0.26, 0.10), "sienna": (0.62, 0.32, 0.16), "tan_cloth": (0.80, 0.58, 0.32),
    "steelblue": (0.42, 0.56, 0.74), "wool": (0.45, 0.32, 0.20), "elven": (0.12, 0.55, 0.20),
    "shadow": (0.34, 0.22, 0.68), "lavender": (0.66, 0.56, 0.96), "stew": (0.48, 0.24, 0.08),
    "bread": (0.84, 0.56, 0.24), "cheese": (0.98, 0.78, 0.16), "apple": (0.40, 0.80, 0.12),
    "leaf": (0.20, 0.55, 0.14), "meat": (0.62, 0.26, 0.10), "honey": (0.98, 0.70, 0.10),
})

LIE = dict(tilt_x=-math.pi / 2, yaw=-math.pi / 4)  # a thing built along +z, laid along the NE diagonal
LEAN = dict(tilt_x=0.35, yaw=math.pi / 4)  # a thing built face up, tipped to face the camera (SE)


GROW = 1.25  # every item is built at about life size, then grown by this to read beside the heroes
MAX_SPAN = 1.05  # but no wider than about a tile


class Item(ck.Part):
    """One item: a single mesh, rested on the floor, centred on its tile and grown at `finish`."""

    def __init__(self, name, grow=GROW):
        super().__init__(name, (0.0, 0.0, 0.0))
        self.grow = grow

    def finish(self):
        vs = self.bm.verts
        lo = [min(v.co[i] for v in vs) for i in range(3)]
        hi = [max(v.co[i] for v in vs) for i in range(3)]
        grow = min(self.grow, MAX_SPAN / max(hi[0] - lo[0], hi[1] - lo[1]))
        for v in vs:
            v.co.x = (v.co.x - (lo[0] + hi[0]) / 2) * grow
            v.co.y = (v.co.y - (lo[1] + hi[1]) / 2) * grow
            v.co.z = (v.co.z - lo[2]) * grow
        return super().finish(dk.kit_collection(self.name))


def lay(p, build, scale=(1.0, 1.0, 1.0), roll=0.0):
    """Builds a held thing (grip at the origin, along +z), turned `roll` about its length, laid down."""
    placed(p, lambda: placed(p, build, yaw=roll, scale=scale), **LIE)


def recoloured(p, build, swap):
    """Runs `build()` and swaps the (undyed) materials it used, e.g. {"steel": "gold"}."""
    start = len(p.bm.faces)
    build()
    for f in list(p.bm.faces)[start:]:
        new = swap.get(p.mats[f.material_index])
        if new:
            if new not in p.mats:
                p.mats.append(new)
            f.material_index = p.mats.index(new)


def flat_shield(p, build):
    """A held shield (`ck.shield` stands it on the arm) put back face up, then leant."""
    placed(p, lambda: placed(p, lambda: placed(p, build, yaw=0.5), tilt_x=-math.pi / 2), **LEAN)


# --- weapons -----------------------------------------------------------------------

def build_dagger():
    p = Item("dagger")
    lay(p, lambda: ck.sword(p), scale=(1.5, 1.5, 0.5))
    return p


def build_longsword():
    p = Item("longsword")
    lay(p, lambda: ck.sword(p), scale=(1.35, 1.35, 0.8))
    return p


def build_mithril_sword():
    """A longsword with a glowing pale blue blade and a gold guard."""
    p = Item("mithril_sword")
    lay(p, lambda: recoloured(p, lambda: ck.sword(p), {"steel": "mithril", "iron": "gold"}), scale=(1.45, 1.45, 0.85))
    return p


def build_shortbow():
    p = Item("shortbow")
    lay(p, lambda: ck.bow(p), scale=(1.5, 1.0, 0.62), roll=math.pi / 2)
    return p


def build_battleaxe():
    p = Item("battleaxe")
    lay(p, lambda: pk.axe(p), scale=(1.5, 1.5, 0.72))
    return p


def build_dwarven_greataxe():
    """A double-bitted axe with gold bands on the haft."""
    p = Item("dwarven_greataxe")

    def build():
        # `mk.big_axe`'s blades are single faces, which vanish lying flat: its haft and head, with solid blades.
        p.prism(0, 0, -0.35, 0.95, 0.03, 0.03, 5, "wood", 0.9)
        p.box(-0.02, 0.02, -0.04, 0.04, 0.6, 0.95, "iron", 0.9)
        for side in (-1, 1):
            pk.slab_from_outline(p, [(side * 0.02, 0.62), (side * 0.32, 0.5), (side * 0.32, 1.05), (side * 0.02, 0.92)][::side], -0.02, 0.02, "steel")
        for z in (0.0, 0.4):
            drum(p, 0, 0, z, z + 0.06, 0.045, 0.045, 6, "gold", 1.0)
    lay(p, build, scale=(1.0, 1.5, 0.58))
    return p


def build_warhammer():
    """A war hammer: a steel head with a spike behind."""
    p = Item("warhammer")

    def build():
        recoloured(p, lambda: ck.hammer(p), {"iron": "steel"})
        p.prism(0, 0.12, 0.47, 0.55, 0.05, 0.0, 4, "steel", 1.0, cap=False)
        placed(p, lambda: p.prism(0, 0, 0.0, 0.16, 0.05, 0.0, 4, "steel", 1.1, cap=False), at=(0, 0.12, 0.51), tilt_x=-math.pi / 2)
    lay(p, build, scale=(1.6, 1.6, 1.15), roll=math.pi / 2)
    return p


def build_saints_hammer():
    """A hammer with a glowing white-gold head and gold bands."""
    p = Item("saints_hammer")

    def build():
        recoloured(p, lambda: ck.hammer(p), {"iron": "holy"})
        for z in (0.1, 0.3):
            drum(p, 0, 0, z, z + 0.04, 0.03, 0.03, 6, "gold", 1.0)
    lay(p, build, scale=(1.6, 1.6, 1.15), roll=math.pi / 2)
    return p


def build_blessed_mace():
    p = Item("blessed_mace")
    lay(p, lambda: recoloured(p, lambda: ck.mace(p), {"steel": "gold"}), scale=(1.5, 1.5, 1.05))
    return p


def build_runed_staff():
    """A staff topped with a glowing violet crystal, two gold rune bands."""
    p = Item("runed_staff")

    def build():
        ck.staff(p, "arcane")
        for z in (0.2, 0.6):
            drum(p, 0, 0, z, z + 0.05, 0.035, 0.035, 6, "gold", 1.0)
    lay(p, build, scale=(1.7, 1.7, 0.48))
    return p


def build_staff_of_the_archmage():
    """A dark staff with a big glowing orb held in gold claws."""
    p = Item("staff_of_the_archmage")

    def build():
        p.prism(0, 0, -0.66, 1.0, 0.03, 0.03, 6, "dye.dark", 1.0)
        drum(p, 0, 0, 1.0, 1.06, 0.04, 0.08, 6, "gold", 1.0)
        for k in range(3):
            a = 2 * math.pi * k / 3
            rod(p, (math.cos(a) * 0.07, math.sin(a) * 0.07, 1.05), (math.cos(a) * 0.14, math.sin(a) * 0.14, 1.32), 0.025, 0.0, "gold", 1.1, sides=4)
        drum(p, 0, 0, 1.08, 1.2, 0.1, 0.14, 8, "arcane", 1.0)
        p.prism(0, 0, 1.2, 1.32, 0.14, 0.08, 8, "arcane", 1.1)
    lay(p, build, scale=(1.7, 1.7, 0.5))
    return p


# --- shields -----------------------------------------------------------------------

def build_shield():
    p = Item("shield")
    flat_shield(p, lambda: placed(p, lambda: ck.shield(p, "dye.royal"), scale=(1.2, 1.2, 1.2)))
    return p


def build_ward_of_the_faithful():
    """A round shield with a gold rim and a gold sun cross on white."""
    p = Item("ward_of_the_faithful")

    def build():
        recoloured(p, lambda: ck.shield(p, "dye.white"), {"steel": "gold"})
        p.box(-0.035, 0.035, -0.22, 0.22, 0.05, 0.065, "gold", 1.1)
        p.box(-0.22, 0.22, -0.035, 0.035, 0.05, 0.065, "gold", 1.1)
    flat_shield(p, lambda: placed(p, build, scale=(1.25, 1.25, 1.25)))
    return p


def build_spellguard_buckler():
    """A small violet buckler with a glowing violet boss."""
    p = Item("spellguard_buckler")
    flat_shield(p, lambda: placed(p, lambda: recoloured(p, lambda: ck.shield(p, "dye.purple"), {"steel": "arcane"}), scale=(0.9, 0.9, 1.0)))
    return p


def build_tower_shield():
    """A tall oblong shield, red with an iron rim, band and boss."""
    p = Item("tower_shield")

    def build():
        p.box(-0.24, 0.24, -0.4, 0.4, 0.0, 0.04, "iron", 0.85)
        p.box(-0.21, 0.21, -0.37, 0.37, 0.04, 0.055, "dye.red", 1.0)
        p.box(-0.21, 0.21, -0.04, 0.04, 0.055, 0.07, "iron", 1.0)
        drum(p, 0, 0, 0.055, 0.11, 0.08, 0.05, 6, "steel", 1.1)
    placed(p, build, **LEAN)
    return p


def build_kite_shield():
    """A steel kite shield with a red cross."""
    p = Item("kite_shield")
    kite = [(-0.26, 0.2), (-0.24, 0.32), (0.0, 0.38), (0.24, 0.32), (0.26, 0.2), (0.0, -0.42)]

    def build():
        plate(p, kite, "steel", 1.0, thick=0.05)
        p.box(-0.035, 0.035, -0.28, 0.32, 0.05, 0.065, "dye.red", 1.0)
        p.box(-0.2, 0.2, 0.14, 0.21, 0.05, 0.065, "dye.red", 1.0)
    placed(p, build, **LEAN)
    return p


# --- armour --------------------------------------------------------------------------

def garment(p, mat, sleeve=0.14, skirt=0.0, flare=0.0, bulk=0.05):
    """A shirt laid flat, neck to the north: body, sleeves, a dark neck hole; a robe adds a skirt."""
    p.box(-0.22, 0.22, -0.28, 0.24, 0.0, bulk, mat, 0.95)
    p.box(-0.17, 0.17, -0.18, 0.2, bulk, bulk + 0.03, mat, 1.05)
    for side in (-1, 1):
        x0, x1 = sorted((side * 0.22, side * (0.22 + sleeve)))
        p.box(x0, x1, 0.04, 0.24, 0.0, bulk * 0.8, mat, 0.9)
    p.box(-0.07, 0.07, 0.17, 0.24, bulk + 0.03, bulk + 0.035, "dye.dark", 1.0)
    if skirt:
        w = 0.22 + flare
        p.face([(-0.22, -0.28, bulk), (0.22, -0.28, bulk), (w, -0.28 - skirt, 0.02), (-w, -0.28 - skirt, 0.02)], mat, 1.0)
        p.face([(-w, -0.28 - skirt, 0.02), (w, -0.28 - skirt, 0.02), (w, -0.28 - skirt, 0.0), (-w, -0.28 - skirt, 0.0)], mat, 0.8)
        for side in (-1, 1):
            p.face([(side * 0.22, -0.28, 0.0), (side * 0.22, -0.28, bulk), (side * w, -0.28 - skirt, 0.02), (side * w, -0.28 - skirt, 0.0)][::side], mat, 0.85)


def belt(p, z, mat="dye.dark"):
    p.box(-0.225, 0.225, -0.2, -0.13, 0.0, z, mat, 0.9)


def build_padded_tunic():
    """A quilted tan tunic: darker seams across it."""
    p = Item("padded_tunic")
    garment(p, "dye.tan_cloth", bulk=0.07)
    for y in (-0.14, -0.02, 0.1):
        p.box(-0.175, 0.175, y - 0.012, y + 0.012, 0.1, 0.105, "dye.tan_cloth", 0.6)
    return p


def build_leather_armour():
    """A brown leather jerkin with shoulder pads and a belt."""
    p = Item("leather_armour")
    garment(p, "dye.saddle", sleeve=0.06)
    belt(p, 0.085)
    for side in (-1, 1):
        p.box(side * 0.14 - 0.1, side * 0.14 + 0.1, 0.08, 0.26, 0.05, 0.1, "dye.saddle", 1.15)
    return p


def build_studded_leather():
    """The leather jerkin with rows of steel studs."""
    p = Item("studded_leather")
    garment(p, "dye.sienna", sleeve=0.06)
    belt(p, 0.085)
    for x in (-0.11, 0.0, 0.11):
        for y in (-0.08, 0.03, 0.14):
            p.prism(x, y, 0.08, 0.12, 0.03, 0.0, 4, "steel", 1.2, cap=False)
    return p


def build_chain_mail():
    """A mail shirt with long sleeves, rows of dark rings across it."""
    p = Item("chain_mail")
    garment(p, "steel", sleeve=0.18)
    for y in (-0.22, -0.12, -0.02, 0.08):
        p.box(-0.175, 0.175, y - 0.015, y + 0.015, 0.08, 0.085, "iron", 1.0)
    return p


def build_scale_mail():
    """A shirt of overlapping steel-blue scales, a belt, short sleeves."""
    p = Item("scale_mail")
    garment(p, "dye.steelblue", sleeve=0.1)
    for row, y in enumerate((-0.1, 0.03, 0.16)):
        for k in range(3 - row % 2):
            x = -0.12 + k * 0.12 + (row % 2) * 0.06
            placed(p, lambda: p.box(-0.055, 0.055, -0.07, 0.0, 0.0, 0.025, "steel", 1.1), at=(x, y, 0.08), tilt_x=-0.3)
    belt(p, 0.085)
    return p


def cuirass(p, metal, trim):
    """A breastplate stood on the floor, pauldrons on its shoulders and a trimmed rim."""
    octo(p, 0.0, 0.08, 0.2, 0.19, trim, 0.9, depth=0.7, bottom=True)
    octo(p, 0.08, 0.42, 0.19, 0.26, metal, 1.0, depth=0.7, cap=False)
    octo(p, 0.42, 0.48, 0.26, 0.14, metal, 1.1, depth=0.7)
    octo(p, 0.44, 0.5, 0.11, 0.1, trim, 1.0, depth=0.8)
    p.box(-0.03, 0.03, -0.19, -0.13, 0.1, 0.42, metal, 1.2)  # the ridge down the front
    for side in (-1, 1):
        octo(p, 0.3, 0.48, 0.13, 0.08, metal, 1.05, at=(side * 0.27, 0.0), bottom=True)
        octo(p, 0.29, 0.33, 0.135, 0.135, trim, 1.0, at=(side * 0.27, 0.0), cap=False)


def build_plate_armour():
    p = Item("plate_armour")
    cuirass(p, "steel", "gold")
    return p


def build_mithril_plate():
    p = Item("mithril_plate")
    cuirass(p, "mithril", "steel")
    return p


def cloak(p, colour, clasp="gold"):
    """A cloak laid out flat: a wide trapezoid, a hood at the top, a clasp."""
    outline = [(-0.36, -0.36), (0.36, -0.36), (0.18, 0.2), (-0.18, 0.2)]
    plate(p, outline, colour, 1.0, thick=0.04)
    p.box(-0.3, 0.3, -0.3, 0.12, 0.04, 0.06, colour, 0.85)
    for x in (-0.12, 0.12):
        p.box(x - 0.02, x + 0.02, -0.34, 0.14, 0.06, 0.07, colour, 0.7)  # folds
    octo(p, 0.0, 0.1, 0.16, 0.1, colour, 1.05, at=(0.0, 0.26), depth=0.8, bottom=True)
    drum(p, 0, 0.17, 0.06, 0.1, 0.05, 0.05, 6, clasp, 1.1)


def build_wool_cloak():
    p = Item("wool_cloak")
    cloak(p, "dye.wool", "iron")
    return p


def build_elven_cloak():
    p = Item("elven_cloak")
    cloak(p, "dye.elven", "gold")
    return p


def build_shadow_cloak():
    p = Item("shadow_cloak")
    cloak(p, "dye.shadow", "arcane")
    return p


def build_spidersilk_robe():
    """A long lavender robe with a silver sash."""
    p = Item("spidersilk_robe")
    garment(p, "dye.lavender", sleeve=0.16, skirt=0.3, flare=0.08, bulk=0.04)
    belt(p, 0.075, "steel")
    return p


# --- potions -----------------------------------------------------------------------

def bottle(p, liquid, shape="round", size=1.0, cork="leather"):
    """A stoppered bottle of glowing liquid: round, tall, square or a thin vial."""
    s = size
    if shape == "round":
        drum(p, 0, 0, 0.0, 0.16 * s, 0.11 * s, 0.14 * s, 8, liquid, 1.0)
        p.prism(0, 0, 0.16 * s, 0.24 * s, 0.14 * s, 0.045 * s, 8, liquid, 1.1, cap=False)
        neck = 0.24 * s
    elif shape == "tall":
        drum(p, 0, 0, 0.0, 0.26 * s, 0.1 * s, 0.1 * s, 6, liquid, 1.0)
        p.prism(0, 0, 0.26 * s, 0.31 * s, 0.1 * s, 0.04 * s, 6, liquid, 1.1, cap=False)
        neck = 0.31 * s
    elif shape == "square":
        p.box(-0.1 * s, 0.1 * s, -0.1 * s, 0.1 * s, 0.0, 0.22 * s, liquid, 1.0)
        p.prism(0, 0, 0.22 * s, 0.25 * s, 0.07 * s, 0.045 * s, 4, liquid, 1.1, cap=False)
        neck = 0.25 * s
    else:  # vial
        drum(p, 0, 0, 0.0, 0.28 * s, 0.065 * s, 0.075 * s, 6, liquid, 1.0)
        neck = 0.28 * s
    p.prism(0, 0, neck, neck + 0.06 * s, 0.045 * s, 0.045 * s, 6, liquid, 1.2, cap=False)
    drum(p, 0, 0, neck + 0.05 * s, neck + 0.1 * s, 0.05 * s, 0.045 * s, 6, cork, 0.9)
    return neck + 0.1 * s


def build_health_potion():
    p = Item("health_potion")
    bottle(p, "potion_red")
    return p


def build_greater_health_potion():
    p = Item("greater_health_potion")
    bottle(p, "potion_pink", size=1.2)
    return p


def build_superior_health_potion():
    """A tall magenta bottle with a gold collar and stopper."""
    p = Item("superior_health_potion")
    bottle(p, "potion_magenta", "tall", 1.2, cork="gold")
    p.prism(0, 0, 0.1, 0.14, 0.125, 0.125, 6, "gold", 1.0, cap=False)
    return p


def build_potion_of_restoration():
    p = Item("potion_of_restoration")
    bottle(p, "potion_violet", "square", 1.2)
    return p


def build_elixir_of_life():
    """A tall golden elixir in a gold-banded bottle."""
    p = Item("elixir_of_life")
    bottle(p, "potion_gold", "tall", 1.25, cork="gold")
    for z in (0.04, 0.26):
        p.prism(0, 0, z, z + 0.04, 0.13, 0.13, 6, "gold", 1.0, cap=False)
    return p


def build_alchemists_fire():
    """A round flask of fire with a rag stuffed in its neck."""
    p = Item("alchemists_fire")
    top = bottle(p, "potion_fire", cork="dye.cream")
    placed(p, lambda: p.box(-0.03, 0.03, -0.02, 0.02, 0.0, 0.12, "dye.cream", 0.9), at=(0, 0, top - 0.02), tilt_x=0.6)
    return p


def build_holy_water():
    p = Item("holy_water")
    bottle(p, "potion_holy", "vial", 1.3, cork="gold")
    return p


def build_greater_holy_water():
    """A taller holy water flask with a gold cross on its front."""
    p = Item("greater_holy_water")
    bottle(p, "potion_holy", "tall", 1.15, cork="gold")
    p.box(-0.015, 0.015, -0.13, -0.11, 0.07, 0.22, "gold", 1.1)
    p.box(-0.05, 0.05, -0.13, -0.11, 0.15, 0.18, "gold", 1.1)
    return p


# --- scrolls -----------------------------------------------------------------------

def scroll(name, ribbon, seal=None):
    """A rolled parchment lying along the diagonal, a sheet unrolled from it,
    a coloured ribbon round the middle and a wax seal in the same colour."""
    p = Item(name)

    def build():
        drum(p, 0, 0, -0.27, 0.27, 0.08, 0.08, 8, "dye.parchment", 1.0)
        for z0, z1 in ((-0.31, -0.27), (0.27, 0.31)):
            drum(p, 0, 0, z0, z1, 0.035, 0.035, 6, "wood", 0.8)
        drum(p, 0, 0, -0.07, 0.07, 0.09, 0.09, 8, ribbon, 1.0)
        p.box(0.06, 0.08, 0.0, 0.33, -0.25, 0.25, "dye.parchment", 1.1)  # the unrolled sheet, on the floor
        placed(p, lambda: drum(p, 0, 0, 0.0, 0.03, 0.065, 0.065, 6, seal or ribbon, 0.85), at=(-0.085, 0, 0), tilt_y=-math.pi / 2)
    placed(p, lambda: placed(p, build, yaw=math.pi / 2), **LIE)
    return p


SCROLLS = [
    ("magic_missile_scroll", "dye.cyan", None),
    ("fireball_scroll", "dye.orange", None),
    ("confusion_scroll", "dye.pink", None),
    ("town_portal", "dye.blue", "portal"),
    ("scroll_of_magic_mapping", "dye.cornflower", None),
    ("prayer_scroll", "dye.white", "gold"),
    ("scroll_of_divine_light", "dye.gold", "holy"),
    ("scroll_of_lightning", "dye.sky", None),
    ("scroll_of_inferno", "dye.red", "potion_fire"),
    ("scroll_of_meteor_storm", "dye.tomato", None),
]


# --- food --------------------------------------------------------------------------

def loaf(p, at, length=0.3, r=0.09):
    placed(p, lambda: octo(p, -length / 2, length / 2, r, r * 0.8, "dye.bread", 1.0, depth=0.7, bottom=True), at=at, tilt_y=math.pi / 2)


def build_rations():
    """A loaf, a wedge of cheese and a green apple."""
    p = Item("rations")
    loaf(p, (-0.05, 0.08, 0.06))
    placed(p, lambda: p.prism(0, 0, 0.0, 0.1, 0.13, 0.13, 3, "dye.cheese", 1.0), at=(0.1, -0.15, 0.0))
    drum(p, -0.2, -0.16, 0.0, 0.12, 0.07, 0.07, 6, "dye.apple", 1.0)
    p.box(-0.21, -0.19, -0.17, -0.15, 0.12, 0.16, "wood", 0.8)
    return p


def build_hearty_stew():
    """A wooden bowl of brown stew with a spoon in it."""
    p = Item("hearty_stew")
    drum(p, 0, 0, 0.0, 0.13, 0.13, 0.2, 8, "wood", 0.95)
    p.prism(0, 0, 0.11, 0.115, 0.18, 0.18, 8, "dye.stew", 1.0)
    for x, y in ((0.06, 0.04), (-0.07, -0.02), (0.0, -0.08)):
        p.box(x - 0.025, x + 0.025, y - 0.025, y + 0.025, 0.115, 0.14, "dye.orange", 1.0)
    rod(p, (0.02, 0.02, 0.12), (0.2, 0.2, 0.3), 0.018, 0.018, "wood", 1.1, sides=4)
    return p


def build_waybread():
    """Three flat cakes stacked on a pair of green leaves."""
    p = Item("waybread")
    for side in (-1, 1):
        plate(p, [(0, 0), (side * 0.14, 0.1), (side * 0.3, 0.0), (side * 0.14, -0.1)], "dye.leaf", 1.0, thick=0.015)
    for k in range(3):
        placed(p, lambda: p.box(-0.12, 0.12, -0.12, 0.12, 0.0, 0.035, "dye.bread", 1.0 - k * 0.06), at=(0, 0, 0.015 + k * 0.04), yaw=k * 0.35)
    return p


def build_dwarven_feast():
    """A roast leg on a wooden platter beside a tankard."""
    p = Item("dwarven_feast")
    drum(p, 0, 0, 0.0, 0.03, 0.28, 0.28, 8, "wood", 0.85)
    rod(p, (-0.16, -0.08, 0.1), (0.08, 0.06, 0.12), 0.11, 0.07, "dye.meat", 1.0, sides=6, closed=True, squash=0.8)
    rod(p, (0.08, 0.06, 0.12), (0.2, 0.13, 0.13), 0.025, 0.025, "dye.bone", 1.1, sides=4, closed=True)
    placed(p, lambda: ck.mug(p), at=(-0.1, 0.16, 0.03), scale=(1.4, 1.4, 1.4))
    return p


def build_ambrosia():
    """A glowing golden honey pot with a wooden lid."""
    p = Item("ambrosia")
    drum(p, 0, 0, 0.0, 0.18, 0.11, 0.15, 8, "dye.honey", 1.0)
    p.prism(0, 0, 0.18, 0.24, 0.15, 0.09, 8, "dye.honey", 1.05, cap=False)
    drum(p, 0, 0, 0.23, 0.26, 0.1, 0.1, 8, "potion_gold", 1.0)
    drum(p, 0, 0, 0.26, 0.3, 0.11, 0.1, 8, "wood", 0.9)
    p.box(-0.02, 0.02, -0.02, 0.02, 0.3, 0.36, "wood", 1.0)
    return p


# --- treasure ----------------------------------------------------------------------

def build_gold():
    """A heap of coins with two short stacks beside it."""
    p = Item("gold")
    p.prism(0, 0, 0.0, 0.1, 0.22, 0.06, 8, "gold", 1.0)
    for x, y, n in ((0.2, -0.12, 3), (-0.18, -0.16, 2)):
        for k in range(n):
            drum(p, x + k * 0.01, y, k * 0.035, k * 0.035 + 0.03, 0.07, 0.07, 6, "gold", 0.9 + 0.1 * k)
    for x, y in ((0.04, 0.24), (-0.24, 0.06)):
        drum(p, x, y, 0.0, 0.02, 0.06, 0.06, 6, "gold", 1.1)
    return p


def build_heart_of_the_abyss():
    """The goal of the game: a big faceted crimson heart, glowing, held up by three iron claws."""
    p = Item("heart_of_the_abyss", grow=1.6)
    drum(p, 0, 0, 0.0, 0.08, 0.22, 0.18, 6, "dye.dark", 1.0)
    for k in range(3):
        a = 2 * math.pi * k / 3 + math.pi / 2
        rod(p, (math.cos(a) * 0.15, math.sin(a) * 0.15, 0.06), (math.cos(a) * 0.2, math.sin(a) * 0.2, 0.36), 0.035, 0.0, "iron", 1.1, sides=4)
    # The heart: a point down, two lobes up, faceted.
    def heart():
        p.prism(0, 0, 0.12, 0.42, 0.0, 0.2, 6, "heart", 0.9, cap=False)
        for side in (-1, 1):
            drum(p, side * 0.1, 0, 0.36, 0.48, 0.12, 0.11, 6, "heart", 1.0)
            p.prism(side * 0.1, 0, 0.48, 0.54, 0.11, 0.05, 6, "heart", 1.15)
    placed(p, heart, scale=(1.0, 0.75, 1.0), yaw=math.pi / 4)
    return p


BUILDERS = [
    build_dagger, build_longsword, build_mithril_sword, build_shortbow, build_battleaxe,
    build_dwarven_greataxe, build_warhammer, build_saints_hammer, build_blessed_mace,
    build_runed_staff, build_staff_of_the_archmage,
    build_shield, build_ward_of_the_faithful, build_spellguard_buckler, build_tower_shield, build_kite_shield,
    build_padded_tunic, build_leather_armour, build_studded_leather, build_chain_mail, build_scale_mail,
    build_plate_armour, build_mithril_plate, build_wool_cloak, build_elven_cloak, build_shadow_cloak,
    build_spidersilk_robe,
    build_health_potion, build_greater_health_potion, build_superior_health_potion,
    build_potion_of_restoration, build_elixir_of_life, build_alchemists_fire, build_holy_water,
    build_greater_holy_water,
    *[lambda s=s: scroll(*s) for s in SCROLLS],
    build_rations, build_hearty_stew, build_waybread, build_dwarven_feast, build_ambrosia,
    build_gold, build_heart_of_the_abyss,
]


def build_all():
    names = [build().finish().name for build in BUILDERS]
    ck.glow()
    lay_out_preview(names)
    return names


def lay_out_preview(names):
    """The characters' preview (floor, `_cam`, `_sun`), respaced to rows of eight."""
    ck.lay_out_preview(names)
    cols = 8
    for i, name in enumerate(names):
        bpy.data.objects[f"_{name}"].location = ((i % cols) * 1.0, -(i // cols) * 1.0, 0.0)
    rows = (len(names) + cols - 1) // cols
    floor = bpy.data.objects["_floor"]
    floor.scale = (cols / 7.5, (rows - 0.5) / 2.25, 1.0)
    floor.location = (0.0, 0.0, 0.0)
    cam = bpy.data.objects["_cam"]
    cam.data.ortho_scale = 8
    yaw, pitch = math.radians(45), math.radians(35)
    look = Vector((math.sin(yaw) * math.cos(pitch), -math.cos(yaw) * math.cos(pitch), math.sin(pitch)))
    cam.location = Vector(((cols - 1) / 2, -(rows - 1) / 2, 0.0)) + look * 30


if __name__ == "__main__":
    build_all()
