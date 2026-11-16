"""Builds the town kit (ticket A02) procedurally in the open .blend.

Run inside Blender (Text Editor, or the MCP with `__file__` set), then save and
export:
    blender art/town_kit.blend --background --python art/build_town_kit.py
rebuilds every kit collection from scratch; art/export.py then writes the .glb
files. It reuses the dungeon kit's helpers (`Piece`, `material`, the stone
wall faces) from build_dungeon_kit.py, with the town's own palette.

Same conventions as the dungeon kit: +X east, +Y north (towards smaller grid
y), +Z up, one unit per metre and tile, each piece built at the origin on the
tile centre. Colour variation is a vertex colour multiplied into the base
colour. Pieces the game asks for (src/models.rs):
- tile_grass, tile_road, tile_wood_floor (each + _2, _3), tile_shallow_water,
  tile_deep_water, tile_bridge: terrain. Every terrain piece is a closed slab
  down to SKIRT, so a lower neighbour (water beside grass) shows a bank, never
  a gap.
- tile_house_wall_<mask>: timber frame and plaster on a stone footing, for
  walls beside a building floor. The straight runs (masks 3 and 12) have
  variants: _2 a lit window, _3 cross braces.
- tile_town_wall_<mask>: the dungeon's stone courses with merlons on top, for
  the town wall and other walls in town.
- roof (+ _2, _3): a shingled tile the game puts on each roofed building tile
  at wall height (src/occlusion.rs).
"""

import importlib.util
import math
from pathlib import Path

import bpy
from mathutils import Vector

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("build_dungeon_kit", HERE / "build_dungeon_kit.py")
dk = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(dk)

H = dk.H
SKIRT = -0.3  # every terrain slab reaches down to here
FOOTING = 0.42  # top of the stone footing under a house wall
INSET = 0.05  # plaster sits this far behind the timber and the footing
POST = 0.12  # timber post width
BEAM = 0.13  # timber beam height
MERLON = 0.28  # merlon height on the town wall

# sRGB base colour, roughness, metallic. `stone` and `wood` replace the dungeon's.
dk.PALETTE.update({
    "stone": ((0.56, 0.54, 0.50), 0.95, 0.0),
    "wood": ((0.36, 0.23, 0.14), 0.9, 0.0),
    "plaster": ((0.86, 0.80, 0.66), 0.95, 0.0),
    "shingle": ((0.60, 0.30, 0.20), 0.9, 0.0),
    "grass": ((0.33, 0.45, 0.22), 1.0, 0.0),
    "earth": ((0.50, 0.40, 0.28), 1.0, 0.0),
    "shallows": ((0.26, 0.60, 0.66), 0.8, 0.0),
    "deeps": ((0.08, 0.30, 0.50), 0.75, 0.0),
    "window": ((1.0, 0.72, 0.38), 0.6, 0.0),
})
WINDOW_GLOW = 2.5  # emission strength of a lit window


def shaded_face(p, verts, mat, shades):
    """A face with its own brightness per corner."""
    f = p.face(verts, mat)
    for loop, s in zip(f.loops, shades):
        loop[p.col] = (s, s, s, 1.0)
    return f


def slab(p, top, mat, skirt_mat, skirt_shade=0.7, rough=0.08):
    """A tile-filling top at `top` (four triangles round a centre that is lighter
    or darker; the corners stay at full brightness so neighbours match) and its
    sides down to SKIRT, closed underneath so the normals recalculate outward."""
    centre = (0.0, 0.0, top)
    shade = 1.0 + p.rng.uniform(-rough, rough)
    corners = [(-0.5, -0.5, top), (0.5, -0.5, top), (0.5, 0.5, top), (-0.5, 0.5, top)]
    for i in range(4):
        shaded_face(p, [corners[i], corners[(i + 1) % 4], centre], mat, [1.0, 1.0, shade])
    p.box(-0.5, 0.5, -0.5, 0.5, SKIRT, top, skirt_mat, skirt_shade, skip=["t"])


def tuft(p, x, y, height, shade):
    """Three leaning blades of grass, each a thin three-sided spike."""
    for k in range(3):
        a = 2 * math.pi * (k / 3 + p.rng.uniform(-0.1, 0.1))
        lean = 0.04
        tip = (x + math.cos(a) * lean, y + math.sin(a) * lean, height * p.rng.uniform(0.75, 1.0))
        bx, by = x + math.cos(a) * 0.015, y + math.sin(a) * 0.015
        base = [(bx + 0.025 * math.cos(a + d), by + 0.025 * math.sin(a + d), -0.005) for d in (0, 2.1, 4.2)]
        for i in range(3):
            p.face([base[i], base[(i + 1) % 3], tip], "grass", shade * p.rng.uniform(0.85, 1.1))


def pebble(p, x, y, r, h, mat="stone", shade=1.0):
    p.prism(x, y, -0.01, h, r, r * 0.7, 5, mat, shade * p.rng.uniform(0.85, 1.1))


def spots(p, n, margin=0.36):
    return [(p.rng.uniform(-margin, margin), p.rng.uniform(-margin, margin)) for _ in range(n)]


# --- terrain -----------------------------------------------------------------

def build_grass(name, tufts, stones):
    p = dk.Piece(name)
    slab(p, 0.0, "grass", "earth", rough=0.1)
    for x, y in spots(p, tufts):
        tuft(p, x, y, p.rng.uniform(0.1, 0.17), p.rng.uniform(0.75, 1.15))
    for x, y in spots(p, stones, 0.32):
        pebble(p, x, y, p.rng.uniform(0.04, 0.07), 0.035, shade=0.9)
    return p


ROAD_TOP = -0.015  # the road is worn a little below the grass


def build_road(name, stones, tufts):
    p = dk.Piece(name)
    slab(p, ROAD_TOP, "earth", "earth", rough=0.12)
    for x, y in spots(p, stones, 0.38):
        pebble(p, x, y, p.rng.uniform(0.03, 0.06), ROAD_TOP + 0.015)
    for x, y in spots(p, tufts, 0.4):
        tuft(p, x, y, 0.08, 0.9)
    return p


def build_wood_floor(name):
    """Planks running east to west on a dark bed, their end joints staggered."""
    p = dk.Piece(name)
    base = -0.03
    p.face([(-0.5, -0.5, base), (0.5, -0.5, base), (0.5, 0.5, base), (-0.5, 0.5, base)], "wood", 0.35)
    p.box(-0.5, 0.5, -0.5, 0.5, SKIRT, base, "wood", 0.5, skip=["t"])
    g = 0.008
    for k in range(4):
        y0, y1 = -0.5 + k * 0.25 + g, -0.5 + (k + 1) * 0.25 - g
        joints = [-0.5, p.rng.uniform(-0.3, 0.3), 0.5] if p.rng.random() < 0.6 else [-0.5, 0.5]
        for a, b in zip(joints, joints[1:]):
            x0 = a + (g if a > -0.5 else 0)
            x1 = b - (g if b < 0.5 else 0)
            skip = ["b"] + (["w"] if a == -0.5 else []) + (["e"] if b == 0.5 else [])
            p.box(x0, x1, y0, y1, base, -p.rng.uniform(0.0, 0.006), "wood", p.rng.uniform(0.85, 1.25), skip=skip)
    return p


def build_water(name, top, mat):
    p = dk.Piece(name)
    slab(p, top, mat, mat, skirt_shade=0.6, rough=0.15)
    return p


def build_bridge():
    """A pier deck of planks laid north to south on two stringers, with posts,
    over deep water (piers reach out past the shallows)."""
    p = dk.Piece("tile_bridge")
    slab(p, -0.14, "deeps", "deeps", skirt_shade=0.6, rough=0.1)
    for y in (-0.33, 0.33):
        p.box(-0.5, 0.5, y - 0.05, y + 0.05, -0.12, 0.02, "wood", 0.55, skip=["e", "w", "b", "t"])
    edges = [-0.5 + k * 0.2 for k in range(6)]
    for a, b in zip(edges, edges[1:]):
        y0, y1 = -0.43 + p.rng.uniform(0, 0.03), 0.43 - p.rng.uniform(0, 0.03)
        p.box(a + 0.008, b - 0.008, y0, y1, 0.02, 0.08, "wood", p.rng.uniform(0.85, 1.2), skip=["b"])
    for y in (-0.44, 0.44):
        p.box(-0.05, 0.05, y - 0.05, y + 0.05, -0.2, 0.22, "wood", 0.7, skip=["b"])
    return p


# --- house walls -------------------------------------------------------------

SIDES = dk.SIDES  # outward normal, the face's right direction, mask bit


def to_world(side, u, depth, v):
    (nx, ny), (tx, ty), _ = SIDES[side]
    d = 0.5 - depth
    return (nx * d + tx * u, ny * d + ty * u, v)


def face_box(p, side, u0, u1, d0, d1, v0, v1, mat, shade, skip_local=()):
    """A box in a face's own frame (along u, into the wall by depth, up by v).
    `skip_local` names faces to leave out: front, back, top, bottom, left, right."""
    pts = [to_world(side, u, d, v) for u in (u0, u1) for d in (d0, d1) for v in (v0, v1)]
    xs, ys, zs = [q[0] for q in pts], [q[1] for q in pts], [q[2] for q in pts]
    (nx, ny), (tx, ty), _ = SIDES[side]
    world = {"front": dk.side_of((nx, ny)), "back": dk.side_of((-nx, -ny)),
             "right": dk.side_of((tx, ty)), "left": dk.side_of((-tx, -ty)), "top": "t", "bottom": "b"}
    p.box(min(xs), max(xs), min(ys), max(ys), min(zs), max(zs), mat, shade, skip=[world[k] for k in skip_local])


def quad(p, side, u0, u1, v0, v1, depth, mat, shade):
    p.face([to_world(side, u0, depth, v0), to_world(side, u1, depth, v0),
            to_world(side, u1, depth, v1), to_world(side, u0, depth, v1)], mat, shade)


def house_face(p, side, mask, style):
    """Footing, plaster and timber on one exposed face. E and W faces own the
    outside corners (the corner post), N and S faces stop at it."""
    (nx, ny), (tx, ty), _ = SIDES[side]

    def corner(sign):
        # True where the face ends at an outside corner: the side round it is exposed too.
        return not mask & SIDES[dk.side_of((tx * sign, ty * sign))][2]

    lo = -0.5 + (POST if corner(-1) else 0)
    hi = 0.5 - (POST if corner(1) else 0)
    # The plaster spans the wall core: to the boundary where a wall continues, else to the inset.
    m0 = -0.5 + (INSET if corner(-1) else 0)
    m1 = 0.5 - (INSET if corner(1) else 0)

    # Stone footing, flush with the tile edge; at a corner the two ledges meet on the diagonal.
    f0, f1 = -0.5, 0.5
    stones = [f0, -0.17 + p.rng.uniform(-0.05, 0.05), 0.2 + p.rng.uniform(-0.05, 0.05), f1]
    for a, b in zip(stones, stones[1:]):
        quad(p, side, a + (0.012 if a > f0 else 0), b - (0.012 if b < f1 else 0), 0.0, FOOTING, 0.0, "stone", p.rng.uniform(0.8, 1.05))
    quad(p, side, f0, f1, 0.0, FOOTING, 0.012, "stone", dk.MORTAR)
    # The footing's top ledge, back to the plaster.
    p.face([to_world(side, f0, 0.0, FOOTING), to_world(side, f1, 0.0, FOOTING),
            to_world(side, min(f1, m1), INSET, FOOTING), to_world(side, max(f0, m0), INSET, FOOTING)], "stone", 0.95)

    # Timber: sill and head beams, a post at each outside corner, posts round a window or a braced middle post.
    face_box(p, side, lo, hi, 0.0, INSET, FOOTING, FOOTING + BEAM * 0.8, "wood", 0.9, skip_local=["back", "bottom", "left", "right"])
    face_box(p, side, lo, hi, 0.0, INSET, H - BEAM, H, "wood", 0.85, skip_local=["back", "top", "left", "right"])
    if side in "ew":
        for sign in (-1, 1):
            if corner(sign):
                u0, u1 = (-0.5, -0.5 + POST) if sign < 0 else (0.5 - POST, 0.5)
                face_box(p, side, u0, u1, 0.0, POST, FOOTING, H, "wood", 0.8, skip_local=["back", "top", "bottom"])
    v0, v1 = FOOTING + BEAM * 0.8, H - BEAM
    plaster = []  # (u0, u1, w0, w1) rectangles of plaster between the timbers
    if style == "window":
        w0, w1, z0, z1 = -0.24, 0.24, 0.92, 1.58
        for u0, u1 in ((w0 - POST * 0.7, w0), (w1, w1 + POST * 0.7)):
            face_box(p, side, u0, u1, 0.0, INSET, v0, v1, "wood", 0.85, skip_local=["back", "top", "bottom"])
        face_box(p, side, w0, w1, 0.0, INSET + 0.02, z0 - 0.07, z0, "wood", 0.95, skip_local=["back", "left", "right"])
        face_box(p, side, w0, w1, 0.0, INSET, z1, z1 + 0.06, "wood", 0.85, skip_local=["back", "left", "right"])
        # The window: a glowing pane set back, with a cross of glazing bars and reveals.
        pane = INSET + 0.06
        quad(p, side, w0, w1, z0, z1, pane, "window", 1.0)
        p.face([to_world(side, w0, INSET, z0), to_world(side, w0, pane, z0), to_world(side, w0, pane, z1), to_world(side, w0, INSET, z1)], "wood", 0.5)
        p.face([to_world(side, w1, INSET, z1), to_world(side, w1, pane, z1), to_world(side, w1, pane, z0), to_world(side, w1, INSET, z0)], "wood", 0.5)
        p.face([to_world(side, w1, INSET, z1), to_world(side, w0, INSET, z1), to_world(side, w0, pane, z1), to_world(side, w1, pane, z1)], "wood", 0.4)
        p.face([to_world(side, w0, INSET, z0), to_world(side, w1, INSET, z0), to_world(side, w1, pane, z0), to_world(side, w0, pane, z0)], "wood", 0.7)
        face_box(p, side, -0.02, 0.02, INSET + 0.02, pane, z0, z1, "wood", 0.7, skip_local=["back", "top", "bottom"])
        face_box(p, side, w0, w1, INSET + 0.02, pane, 1.24, 1.28, "wood", 0.7, skip_local=["back", "left", "right"])
        cols = [(m0, w0 - POST * 0.7), (w1 + POST * 0.7, m1)]
        plaster += [(a, b, v0, v1) for a, b in cols]
        plaster += [(w0, w1, v0, z0 - 0.07), (w0, w1, z1 + 0.06, v1)]
    elif style == "brace":
        face_box(p, side, -POST / 2, POST / 2, 0.0, INSET, v0, v1, "wood", 0.85, skip_local=["back", "top", "bottom"])
        plaster += [(m0, -POST / 2, v0, v1), (POST / 2, m1, v0, v1)]
        for a, b in ((-POST / 2 - 0.02, lo + 0.02), (POST / 2 + 0.02, hi - 0.02)):
            brace(p, side, a, b, v0, v1)
    else:
        plaster.append((m0, m1, v0, v1))  # plain: one long panel, so a wall run reads as one wall
    for u0, u1, w0, w1 in plaster:
        quad(p, side, u0, u1, w0, w1, INSET, "plaster", p.rng.uniform(0.95, 1.03))


def brace(p, side, ua, ub, v0, v1, w=0.13):
    """A diagonal timber from the middle post's foot (ua, v0) up to (ub, v1)."""
    d = 0.02
    pts = [(ua, v0), (ua, v0 + w * 1.6), (ub, v1), (ub, v1 - w * 1.6)]
    front = [to_world(side, u, d, v) for u, v in pts]
    back = [to_world(side, u, INSET, v) for u, v in pts]
    if (ub - ua) < 0:
        front, back = front[::-1], back[::-1]
    p.face(front, "wood", 0.8)
    for i in range(4):
        j = (i + 1) % 4
        p.face([back[i], back[j], front[j], front[i]], "wood", 0.6)


def build_house_wall(mask, style="plain", suffix=""):
    p = dk.Piece(f"tile_house_wall_{mask}{suffix}")
    exposed = [s for s, (_, _, bit) in SIDES.items() if not mask & bit]
    for side in exposed:
        house_face(p, side, mask, style)
    # Faces towards a neighbouring wall stay plain plaster: hidden unless that wall is cut away.
    x0 = -0.5 + (INSET if "w" in exposed else 0)
    x1 = 0.5 - (INSET if "e" in exposed else 0)
    y0 = -0.5 + (INSET if "s" in exposed else 0)
    y1 = 0.5 - (INSET if "n" in exposed else 0)
    p.box(x0, x1, y0, y1, 0, H, "plaster", 0.8, skip=["t", "b"] + exposed)
    # A dark timber wall plate on top, framing the roof.
    p.face([(-0.5, -0.5, H), (0.5, -0.5, H), (0.5, 0.5, H), (-0.5, 0.5, H)], "wood", 0.55)
    return p


# --- town wall ---------------------------------------------------------------

def build_town_wall(mask):
    """The dungeon's stone courses in the town's stone, with merlons along the top."""
    p = dk.build_wall(mask)
    p.name = f"tile_town_wall_{mask}"
    ns, ew = mask & 3, mask & 12
    top = H + MERLON
    if ns and not ew:
        for c in (-0.25, 0.25):
            p.box(-0.42, 0.42, c - 0.14, c + 0.14, H, top, "stone", p.rng.uniform(0.8, 1.0), skip=["b"])
    elif ew and not ns:
        for c in (-0.25, 0.25):
            p.box(c - 0.14, c + 0.14, -0.42, 0.42, H, top, "stone", p.rng.uniform(0.8, 1.0), skip=["b"])
    else:
        p.box(-0.3, 0.3, -0.3, 0.3, H, top + 0.06, "stone", 0.85, skip=["b"])
    return p


# --- roof --------------------------------------------------------------------

COURSES = 2
ROOF_LOW, ROOF_HIGH = 0.05, 0.18  # shingle course height at its top (north) and butt (south) edge


def build_roof(name):
    """Overlapping shingle courses running east to west, each butt edge facing
    south. Built at z = 0: the game lifts it onto the wall tops."""
    p = dk.Piece(name)
    run = 1.0 / COURSES
    for k in range(COURSES):
        y0, y1 = -0.5 + k * run, -0.5 + (k + 1) * run
        joints = [-0.5] + [-0.5 + (i + (0.5 if k % 2 else 0.0)) / 3 for i in range(1, 4) if -0.5 < -0.5 + (i + (0.5 if k % 2 else 0.0)) / 3 < 0.5] + [0.5]
        for a, b in zip(joints, joints[1:]):
            s = p.rng.uniform(0.8, 1.1)
            p.face([(a, y0, ROOF_HIGH), (b, y0, ROOF_HIGH), (b, y1, ROOF_LOW), (a, y1, ROOF_LOW)], "shingle", s)
            bottom = 0.0 if k == 0 else ROOF_LOW
            p.face([(a, y0, bottom), (b, y0, bottom), (b, y0, ROOF_HIGH), (a, y0, ROOF_HIGH)], "shingle", s * 0.7)
        # Ends: the sawtooth profile, closed down to the wall tops.
        p.face([(0.5, y0, 0.0), (0.5, y1, 0.0), (0.5, y1, ROOF_LOW), (0.5, y0, ROOF_HIGH)], "shingle", 0.6)
        p.face([(-0.5, y1, 0.0), (-0.5, y0, 0.0), (-0.5, y0, ROOF_HIGH), (-0.5, y1, ROOF_LOW)], "shingle", 0.6)
    p.face([(0.5, 0.5, 0.0), (-0.5, 0.5, 0.0), (-0.5, 0.5, ROOF_LOW), (0.5, 0.5, ROOF_LOW)], "shingle", 0.6)
    p.face([(-0.5, -0.5, 0.0), (-0.5, 0.5, 0.0), (0.5, 0.5, 0.0), (0.5, -0.5, 0.0)], "shingle", 0.4)
    return p


# --- build -------------------------------------------------------------------

def light_windows():
    mat = bpy.data.materials.get("window")
    if not mat:
        return
    bsdf = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Emission Color"].default_value = (*[dk.linear(c) for c in dk.PALETTE["window"][0]], 1.0)
    bsdf.inputs["Emission Strength"].default_value = WINDOW_GLOW


def build_all():
    pieces = [build_grass("tile_grass", 2, 0), build_grass("tile_grass_2", 5, 0), build_grass("tile_grass_3", 3, 1),
              build_road("tile_road", 2, 0), build_road("tile_road_2", 4, 0), build_road("tile_road_3", 1, 1),
              build_wood_floor("tile_wood_floor"), build_wood_floor("tile_wood_floor_2"), build_wood_floor("tile_wood_floor_3"),
              build_water("tile_shallow_water", -0.08, "shallows"), build_water("tile_deep_water", -0.14, "deeps"),
              build_bridge(),
              build_roof("roof"), build_roof("roof_2"), build_roof("roof_3")]
    pieces += [build_house_wall(mask) for mask in range(16)]
    for mask in (3, 12):
        pieces += [build_house_wall(mask, "window", "_2"), build_house_wall(mask, "brace", "_3")]
    pieces += [build_town_wall(mask) for mask in range(16)]
    for p in pieces:
        p.finish(dk.kit_collection(p.name))
    light_windows()
    lay_out_preview([p.name for p in pieces])


def lay_out_preview(names):
    """Every piece in a grid, and a town corner: a house with a door, a roof
    over half of it, grass, road, the river with a pier, and the town wall.
    All in `_preview`, which export skips; the pieces stay at the origin, hidden."""
    for name in ("_cam", "_sun"):  # not meshes, so kit_collection cannot clear them
        if name in bpy.data.objects:
            bpy.data.objects.remove(bpy.data.objects[name])
    preview = dk.kit_collection("_preview")
    layers = bpy.context.view_layer.layer_collection.children
    for name in names:
        layers[name].hide_viewport = True

    def place(name, x, y, z=0.0, tag=""):
        empty = bpy.data.objects.new(f"_{tag}{name}", None)
        empty.instance_type = "COLLECTION"
        empty.instance_collection = bpy.data.collections[name]
        empty.location = (x, y, z)
        preview.objects.link(empty)

    for i, name in enumerate(names):
        place(name, (i % 10) * 1.5, 20 + (i // 10) * 3.0, tag="grid_")

    # The corner, in grid terms (gx east, gy south), drawn at (gx, -gy).
    w, h = 12, 10
    cells = {(gx, gy): "grass" for gx in range(w) for gy in range(h)}
    for gy in range(h):
        cells[(0, gy)] = "deep_water"
        cells[(1, gy)] = "shallow_water"
    for gx in range(0, 4):
        cells[(gx, 2)] = "bridge"
    for gx in range(2, w):
        cells[(gx, 7)] = "road"
    house = (4, 1, 6, 5)  # x, y, w, h
    hx, hy, hw, hh = house
    for gy in range(hy, hy + hh):
        for gx in range(hx, hx + hw):
            edge = gx in (hx, hx + hw - 1) or gy in (hy, hy + hh - 1)
            cells[(gx, gy)] = "wall" if edge else "wood_floor"
    door = (hx + 2, hy + hh - 1)
    cells[door] = "door"
    for gy in range(hy + hh, 7):
        cells[(door[0], gy)] = "road"
    for gy in range(h):
        cells[(w - 1, gy)] = "road" if gy in (6, 7, 8) else "town_wall"

    def is_wall(c):
        return cells.get(c) in ("wall", "town_wall")

    for (gx, gy), kind in cells.items():
        x, y = gx, -gy - 2
        v = dk.random.Random(gx * 31 + gy).choice(["", "_2", "_3"])
        if kind in ("wall", "town_wall"):
            mask = (1 if is_wall((gx, gy - 1)) else 0) | (2 if is_wall((gx, gy + 1)) else 0) \
                | (4 if is_wall((gx - 1, gy)) else 0) | (8 if is_wall((gx + 1, gy)) else 0)
            base = f"tile_{'house' if kind == 'wall' else 'town'}_wall_{mask}"
            place(base + (v if base + v in bpy.data.collections else ""), x, y)
        elif kind == "door":
            place("tile_wood_floor", x, y)
        else:
            name = f"tile_{kind}" + (v if f"tile_{kind}{v}" in bpy.data.collections else "")
            place(name, x, y)
        # The roof over the house's west half and its walls, the way the game shows it from outside.
        if kind in ("wood_floor", "wall") and gx < hx + 3:
            place("roof" + v, x, y, H)

    # The game's view: orthographic, yaw 45 degrees from the south-east, pitch 35, and a low warm sun.
    cam = bpy.data.objects.new("_cam", bpy.data.cameras.get("_cam") or bpy.data.cameras.new("_cam"))
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = 13
    yaw, pitch = math.radians(45), math.radians(35)
    look = Vector((math.sin(yaw) * math.cos(pitch), -math.cos(yaw) * math.cos(pitch), math.sin(pitch)))
    cam.location = Vector((6, -6.5, 0.5)) + look * 30
    cam.rotation_euler = (-look).to_track_quat("-Z", "Y").to_euler()
    preview.objects.link(cam)
    bpy.context.scene.camera = cam
    sun = bpy.data.objects.new("_sun", bpy.data.lights.get("_sun") or bpy.data.lights.new("_sun", "SUN"))
    sun.data.energy = 3
    sun.data.color = (1.0, 0.8, 0.6)
    sun.rotation_euler = (math.radians(60), 0, math.radians(30))
    preview.objects.link(sun)


if __name__ == "__main__":
    build_all()
