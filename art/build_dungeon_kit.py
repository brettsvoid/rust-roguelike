"""Builds the dungeon kit (ticket A01) procedurally in the open .blend.

Run inside Blender (Text Editor, or the MCP), then save and export:
    blender art/dungeon_kit.blend --background --python art/build_dungeon_kit.py
rebuilds every kit collection from scratch; art/export.py then writes the
.glb files. Re-running replaces only the collections and meshes this script
owns, so it is safe to run again after changing a number below.

Blender axes: +X east, +Y north (towards smaller grid y), +Z up. One unit is
one metre and one tile; each piece is built at the origin, the tile centre on
the floor. Wall mask bits (`Map::wall_glyph_at`): N=1, S=2, W=4, E=8, set where
that neighbour is a wall that borders floor.

Colour variation is a vertex colour multiplied into each material's base
colour (Bevy does the same with COLOR_0), so the whole kit needs only three
materials: stone, wood and iron.
"""

import math
import random

import bmesh
import bpy

H = 2.0  # wall height
INSET = 0.05  # depth of the mortar behind the face of a wall
GAP = 0.03  # mortar joint width between stones
COURSE_TOPS = [0.0, 0.62, 1.3, H]  # wall stone courses, shared by every piece so courses line up
MORTAR = 0.35  # vertex-colour brightness of mortar and other recesses

# sRGB colours, converted to the linear values Blender and glTF store.
PALETTE = {
    "stone": ((0.46, 0.49, 0.54), 0.95, 0.0),
    "wood": ((0.47, 0.30, 0.17), 0.9, 0.0),
    "iron": ((0.22, 0.22, 0.24), 0.7, 0.5),
}


def linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def material(name):
    mat = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    rgb, roughness, metallic = PALETTE[name]
    mat.use_nodes = True
    mat.use_backface_culling = True  # exports doubleSided: false, as the game culls back faces
    tree = mat.node_tree
    tree.nodes.clear()
    out = tree.nodes.new("ShaderNodeOutputMaterial")
    bsdf = tree.nodes.new("ShaderNodeBsdfPrincipled")
    attr = tree.nodes.new("ShaderNodeVertexColor")
    attr.layer_name = "Col"
    mix = tree.nodes.new("ShaderNodeMix")
    mix.data_type = "RGBA"
    mix.blend_type = "MULTIPLY"
    mix.inputs["Factor"].default_value = 1.0
    colour = (*[linear(c) for c in rgb], 1.0)
    tree.links.new(attr.outputs["Color"], mix.inputs[6])
    mix.inputs[7].default_value = colour
    tree.links.new(mix.outputs[2], bsdf.inputs["Base Color"])
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic
    tree.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    mat.diffuse_color = colour
    return mat


class Piece:
    """One mesh under construction: quads with a material and a brightness each."""

    def __init__(self, name, seed=None):
        self.name = name
        self.bm = bmesh.new()
        self.col = self.bm.loops.layers.color.new("Col")
        self.rng = random.Random(seed if seed is not None else name)
        self.mats = []

    def face(self, verts, mat, shade=1.0):
        if mat not in self.mats:
            self.mats.append(mat)
        f = self.bm.faces.new([self.bm.verts.new(v) for v in verts])
        f.material_index = self.mats.index(mat)
        for loop in f.loops:
            loop[self.col] = (shade, shade, shade, 1.0)
        return f

    def box(self, x0, x1, y0, y1, z0, z1, mat, shade=1.0, skip=()):
        """An axis-aligned box; `skip` names faces to leave out (n s e w t b)."""
        sides = {
            "t": [(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)],
            "b": [(x0, y0, z0), (x0, y1, z0), (x1, y1, z0), (x1, y0, z0)],
            "s": [(x0, y0, z0), (x1, y0, z0), (x1, y0, z1), (x0, y0, z1)],
            "n": [(x1, y1, z0), (x0, y1, z0), (x0, y1, z1), (x1, y1, z1)],
            "e": [(x1, y0, z0), (x1, y1, z0), (x1, y1, z1), (x1, y0, z1)],
            "w": [(x0, y1, z0), (x0, y0, z0), (x0, y0, z1), (x0, y1, z1)],
        }
        for key, verts in sides.items():
            if key not in skip:
                self.face(verts, mat, shade)

    def prism(self, cx, cy, z0, z1, r0, r1, sides, mat, shade=1.0, cap=True):
        """A vertical prism or frustum, radius r0 at z0 and r1 at z1."""
        ring = lambda r, z: [
            (cx + r * math.cos(2 * math.pi * i / sides), cy + r * math.sin(2 * math.pi * i / sides), z)
            for i in range(sides)
        ]
        lo, hi = ring(r0, z0), ring(r1, z1)
        for i in range(sides):
            j = (i + 1) % sides
            self.face([lo[i], lo[j], hi[j], hi[i]], mat, shade)
        if cap:
            self.face(hi, mat, shade)

    def finish(self, collection):
        bmesh.ops.remove_doubles(self.bm, verts=self.bm.verts, dist=1e-5)
        bmesh.ops.recalc_face_normals(self.bm, faces=self.bm.faces)
        mesh = bpy.data.meshes.new(self.name)
        self.bm.to_mesh(mesh)
        self.bm.free()
        for name in self.mats:
            mesh.materials.append(material(name))
        obj = bpy.data.objects.new(self.name, mesh)
        collection.objects.link(obj)
        return obj


def kit_collection(name):
    """A fresh top-level collection named for the model the game asks for."""
    old = bpy.data.collections.get(name)
    if old:
        for obj in list(old.objects):
            mesh = obj.data
            bpy.data.objects.remove(obj)
            if mesh and mesh.users == 0:
                bpy.data.meshes.remove(mesh)
        bpy.data.collections.remove(old)
    col = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(col)
    return col


# --- floor -------------------------------------------------------------------

FLOOR_BASE = -0.04  # mortar bed under the flagstones; flagstone tops sit at 0 or just below
FLOOR_LAYOUTS = [
    [(-0.5, 0.03, -0.5, -0.05), (0.03, 0.5, -0.5, -0.05), (-0.5, 0.03, -0.05, 0.5), (0.03, 0.5, -0.05, 0.5)],
    [(-0.5, 0.5, 0.1, 0.5), (-0.5, -0.12, -0.5, 0.1), (-0.12, 0.5, -0.5, 0.1)],
    [(-0.5, -0.08, -0.5, 0.5), (-0.08, 0.5, 0.18, 0.5), (-0.08, 0.5, -0.5, 0.18)],
]


def flagstone(p, x0, x1, y0, y1, top, bevel=0.025):
    """A slab with chamfered top edges and slightly uneven corners."""
    g = GAP / 2
    x0, x1, y0, y1 = x0 + g, x1 - g, y0 + g, y1 - g
    z = [top - p.rng.uniform(0.0, 0.012) for _ in range(4)]
    shade = p.rng.uniform(0.82, 1.08)
    outer = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    inner = [(x0 + bevel, y0 + bevel), (x1 - bevel, y0 + bevel), (x1 - bevel, y1 - bevel), (x0 + bevel, y1 - bevel)]
    lip = [(x, y, zc - bevel * 0.6) for (x, y), zc in zip(outer, z)]
    tops = [(x, y, zc) for (x, y), zc in zip(inner, z)]
    p.face(tops, "stone", shade)
    for i in range(4):
        j = (i + 1) % 4
        p.face([lip[i], lip[j], tops[j], tops[i]], "stone", shade * 0.9)
        p.face([(*outer[i], FLOOR_BASE), (*outer[j], FLOOR_BASE), lip[j], lip[i]], "stone", shade * 0.75)


def build_floor(name, layout):
    p = Piece(name)
    p.face([(-0.5, -0.5, FLOOR_BASE), (0.5, -0.5, FLOOR_BASE), (0.5, 0.5, FLOOR_BASE), (-0.5, 0.5, FLOOR_BASE)], "stone", MORTAR)
    for rect in layout:
        flagstone(p, *rect, top=0.0)
    return p


# --- walls -------------------------------------------------------------------

# Outward normal, then the face's own "right" direction, per side.
SIDES = {
    "n": ((0, 1), (-1, 0), 1),
    "s": ((0, -1), (1, 0), 2),
    "w": ((-1, 0), (0, -1), 4),
    "e": ((1, 0), (0, 1), 8),
}


def side_of(direction):
    return next(k for k, (n, _, _) in SIDES.items() if n == direction)


def wall_face(p, side, mask):
    """Stone courses on one exposed face. E and W faces own the corners."""
    (nx, ny), (tx, ty), _ = SIDES[side]

    def point(u, v, depth):
        d = 0.5 - depth
        return (nx * d + tx * u, ny * d + ty * u, v)

    def end(sign):
        # The face's end towards the side at `sign` along its right direction.
        neighbour = SIDES[side_of((tx * sign, ty * sign))][2]
        if mask & neighbour:
            return sign * (0.5 - GAP / 2)  # wall continues next door: a joint at the boundary
        if side in "ew":
            return sign * 0.5  # an outside corner this face wraps
        return sign * (0.5 - INSET - GAP / 2)  # the E or W face owns the corner

    def core(sign):
        # The mortar spans the core of the wall: to the boundary where a wall continues, else to the inset.
        neighbour = SIDES[side_of((tx * sign, ty * sign))][2]
        return sign * (0.5 if mask & neighbour else 0.5 - INSET)

    lo, hi = end(-1), end(1)
    m0, m1 = core(-1), core(1)
    p.face([point(m0, 0, INSET), point(m1, 0, INSET), point(m1, H, INSET), point(m0, H, INSET)], "stone", MORTAR)
    for k in range(len(COURSE_TOPS) - 1):
        v0 = COURSE_TOPS[k] + (GAP / 2 if k else 0)
        v1 = COURSE_TOPS[k + 1] - (GAP / 2 if k + 1 < len(COURSE_TOPS) - 1 else 0)
        joints = [p.rng.uniform(-0.08, 0.08)] if k % 2 == 0 else [-0.22 + p.rng.uniform(-0.05, 0.05), 0.22 + p.rng.uniform(-0.05, 0.05)]
        edges = [lo] + joints + [hi]
        for i in range(len(edges) - 1):
            u0 = edges[i] + (GAP / 2 if i else 0)
            u1 = edges[i + 1] - (GAP / 2 if i + 1 < len(edges) - 1 else 0)
            base = p.rng.uniform(0.0, 0.02)
            front = [
                point(u0, v0, base + p.rng.uniform(0, 0.015)),
                point(u1, v0, base + p.rng.uniform(0, 0.015)),
                point(u1, v1, base + p.rng.uniform(0, 0.015)),
                point(u0, v1, base + p.rng.uniform(0, 0.015)),
            ]
            back = [point(u0, v0, INSET), point(u1, v0, INSET), point(u1, v1, INSET), point(u0, v1, INSET)]
            shade = p.rng.uniform(0.8, 1.08)
            p.face(front, "stone", shade)
            for a in range(4):
                b = (a + 1) % 4
                if (a == 0 and v0 == 0) or (a == 2 and v1 == H):
                    continue  # on the floor, or under the cap
                p.face([back[a], back[b], front[b], front[a]], "stone", shade * 0.8)


def build_wall(mask):
    p = Piece(f"tile_wall_{mask}")
    exposed = [s for s, (_, _, bit) in SIDES.items() if not mask & bit]
    for side in exposed:
        wall_face(p, side, mask)
    # Faces towards a neighbouring wall stay plain: hidden unless that wall is cut away.
    x0 = -0.5 + (INSET if "w" in exposed else 0)
    x1 = 0.5 - (INSET if "e" in exposed else 0)
    y0 = -0.5 + (INSET if "s" in exposed else 0)
    y1 = 0.5 - (INSET if "n" in exposed else 0)
    p.box(x0, x1, y0, y1, 0, H, "stone", 0.9, skip=["t", "b"] + exposed)
    # The cap covers the stone tops and the mortar between them.
    p.face([(-0.5, -0.5, H), (0.5, -0.5, H), (0.5, 0.5, H), (-0.5, 0.5, H)], "stone", 0.55)
    return p


# --- stairs ------------------------------------------------------------------

STEP = 0.2


def build_down_stairs():
    """A stairwell cut into the floor, its steps going down to the north."""
    p = Piece("tile_down_stairs")
    r = 0.4  # half the opening
    # Rim: a stone frame at floor height round the opening.
    o, i = 0.5, r
    rim = [((-o, -o), (o, -o), (i, -i), (-i, -i)), ((o, -o), (o, o), (i, i), (i, -i)),
           ((o, o), (-o, o), (-i, i), (i, i)), ((-o, o), (-o, -o), (-i, -i), (-i, i))]
    for quad in rim:
        p.face([(x, y, 0.0) for x, y in quad], "stone", 0.95)
    steps = 4
    depth = steps * STEP
    run = 2 * r / steps
    for k in range(steps):
        y0, y1 = -r + k * run, -r + (k + 1) * run
        z = -(k + 1) * STEP
        shade = 0.9 - 0.12 * k  # darker the deeper it goes
        p.face([(-r, y0, z), (r, y0, z), (r, y1, z), (-r, y1, z)], "stone", shade)
        p.face([(-r, y0, z), (-r, y0, z + STEP), (r, y0, z + STEP), (r, y0, z)], "stone", shade * 0.75)
    # Shaft walls, facing in. The south one is never seen from the camera but closes the hole.
    for (x0, y0), (x1, y1) in [((-r, -r), (-r, r)), ((-r, r), (r, r)), ((r, r), (r, -r)), ((r, -r), (-r, -r))]:
        p.face([(x0, y0, 0.0), (x0, y0, -depth), (x1, y1, -depth), (x1, y1, 0.0)], "stone", MORTAR)
    return p


def build_up_stairs():
    """A short flight rising to the north on a flagstone floor, with stone cheeks."""
    p = Piece("tile_up_stairs")
    p.face([(-0.5, -0.5, FLOOR_BASE), (0.5, -0.5, FLOOR_BASE), (0.5, 0.5, FLOOR_BASE), (-0.5, 0.5, FLOOR_BASE)], "stone", MORTAR)
    flagstone(p, -0.5, 0.5, -0.5, -0.1, top=0.0)
    w = 0.32  # half width of the treads
    steps = 3
    run = 0.6 / steps
    for k in range(steps):
        y0 = -0.1 + k * run
        top = (k + 1) * STEP
        p.box(-w, w, y0, 0.5, 0.0, top, "stone", 1.05 - 0.08 * k, skip=["b", "n", "e", "w"])
    for x0, x1 in [(-0.5, -w), (w, 0.5)]:
        p.box(x0, x1, -0.15, 0.5, 0.0, steps * STEP + 0.05, "stone", 0.7, skip=["b"])
    return p


# --- door --------------------------------------------------------------------

def door_frame(p):
    """Posts and lintel filling the doorway between the walls east and west."""
    p.box(-0.49, -0.38, -0.12, 0.12, 0.0, H, "wood", 0.7, skip=["b"])
    p.box(0.38, 0.49, -0.12, 0.12, 0.0, H, "wood", 0.7, skip=["b"])
    p.box(-0.38, 0.38, -0.12, 0.12, 1.8, H, "wood", 0.75, skip=["e", "w"])


def door_leaf(p, open_):
    """Three planks and two iron bands, hinged on the west post. Open swings it north."""
    def place(x, y, z):
        # Closed: the leaf spans x. Open: turned 90 degrees about the hinge at x = -0.38.
        return (-0.36 - y, 0.02 + (x + 0.38), z) if open_ else (x, y, z)

    def box(x0, x1, y0, y1, z0, z1, mat, shade):
        start = len(p.bm.faces)
        p.box(x0, x1, y0, y1, z0, z1, mat, shade, skip=["b"])
        for f in list(p.bm.faces)[start:]:
            for v in f.verts:
                v.co = place(*v.co)

    edges = [-0.375, -0.13, 0.12, 0.375]
    for k in range(3):
        top = 1.78 - p.rng.uniform(0.0, 0.04)
        box(edges[k] + 0.006, edges[k + 1] - 0.006, -0.04, 0.04, 0.02, top, "wood", p.rng.uniform(0.85, 1.05))
    for z in (0.35, 1.4):
        box(-0.36, 0.36, -0.055, 0.055, z, z + 0.08, "iron", 1.0)


def build_door(open_):
    p = Piece("door_open" if open_ else "door", seed="door")
    door_frame(p)
    door_leaf(p, open_)
    return p


# --- wall torch --------------------------------------------------------------

def build_wall_torch():
    """A bracket and torch on the south face of its wall tile. The game rotates
    the model to the open side and draws the flame (src/lighting.rs) at 0.6 m
    out and 1.5 m up, so the cup sits just under that point."""
    p = Piece("wall_torch")
    face = -0.5
    p.box(-0.07, 0.07, face - 0.03, face + 0.01, 1.08, 1.34, "iron", 1.0, skip=["n"])
    p.box(-0.02, 0.02, -0.6, face - 0.03, 1.2, 1.25, "iron", 0.9, skip=["n"])
    p.prism(0.0, -0.6, 1.1, 1.38, 0.03, 0.035, 6, "wood", 0.9)
    p.prism(0.0, -0.6, 1.3, 1.44, 0.045, 0.08, 6, "iron", 0.8)
    return p


# --- build -------------------------------------------------------------------

def build_all():
    pieces = [build_floor("tile_floor", FLOOR_LAYOUTS[0]),
              build_floor("tile_floor_2", FLOOR_LAYOUTS[1]),
              build_floor("tile_floor_3", FLOOR_LAYOUTS[2])]
    pieces += [build_wall(mask) for mask in range(16)]
    pieces += [build_down_stairs(), build_up_stairs(), build_door(False), build_door(True), build_wall_torch()]
    for p in pieces:
        p.finish(kit_collection(p.name))
    lay_out_preview([p.name for p in pieces])


def lay_out_preview(names):
    """Instances of every piece in a grid, in `_preview` (export skips `_` names).
    The pieces themselves stay at the origin, hidden in the viewport."""
    preview = kit_collection("_preview")
    layers = bpy.context.view_layer.layer_collection.children
    for i, name in enumerate(names):
        layers[name].hide_viewport = True
        empty = bpy.data.objects.new(f"_{name}", None)
        empty.instance_type = "COLLECTION"
        empty.instance_collection = bpy.data.collections[name]
        empty.location = ((i % 8) * 1.5, -(i // 8) * 3.0, 0.0)
        preview.objects.link(empty)
    # A small room to judge the pieces together: a wall row with a torch, then floor and stairs.
    room = {(x, 1): f"tile_wall_{(4 if x else 0) | (8 if x < 4 else 0)}" for x in range(5)}
    room.update({(x, y): ("tile_floor", "tile_floor_2", "tile_floor_3")[(x + 2 * y) % 3] for x in range(5) for y in (0, -1, -2)})
    room[(1, -1)] = "tile_down_stairs"
    room[(3, -1)] = "tile_up_stairs"
    room[(2, 1, "torch")] = "wall_torch"
    for key, name in room.items():
        empty = bpy.data.objects.new(f"_room_{name}", None)
        empty.instance_type = "COLLECTION"
        empty.instance_collection = bpy.data.collections[name]
        empty.location = (key[0], key[1] - 12.0, 0.0)
        preview.objects.link(empty)

if __name__ == "__main__":
    build_all()
