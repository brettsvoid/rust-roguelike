"""Exports every top-level collection of the open .blend to
assets/models/<collection name>.glb, one model per collection.

Run from the repo root:
    blender art/dungeon_kit.blend --background --python art/export.py

Conventions (docs/TODO.md): one Blender unit is one metre and one tile, the
model's origin is the tile centre at floor height, walls are 2 m tall. In
Blender's top view +X is east and +Y is north (towards smaller grid y). The
export keeps world positions, so build each collection at the world origin.
Name a collection the way the game asks for it: a raws `model` value such as
`orc`, or a tile name such as `tile_floor` or `tile_wall_10` (see
src/models.rs). Excluded collections are skipped.

Untested: written without a Blender run. Check the first export by eye.
"""

from pathlib import Path

import bpy

out_dir = Path(__file__).resolve().parent.parent / "assets" / "models"
out_dir.mkdir(parents=True, exist_ok=True)

view_layer = bpy.context.view_layer
for layer_collection in view_layer.layer_collection.children:
    if layer_collection.exclude:
        continue
    name = layer_collection.name
    view_layer.active_layer_collection = layer_collection
    path = out_dir / f"{name}.glb"
    bpy.ops.export_scene.gltf(
        filepath=str(path),
        export_format="GLB",
        use_active_collection=True,
        use_active_collection_with_nested=True,
        export_yup=True,
        export_apply=True,
        export_cameras=False,
        export_lights=False,
    )
    print(f"exported {path}")
