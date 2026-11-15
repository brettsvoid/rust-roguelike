"""Writes assets/models/test_pyramid.glb: a magenta 1 x 1 m pyramid, 0.8 m tall,
origin at the centre of its base. A stand-in for checking the model pipeline
without Blender. Run with plain Python 3: `python3 art/make_test_model.py`."""

import json
import struct
from pathlib import Path

apex = (0.0, 0.8, 0.0)
corners = [(-0.5, 0.0, -0.5), (0.5, 0.0, -0.5), (0.5, 0.0, 0.5), (-0.5, 0.0, 0.5)]
# Counter-clockwise seen from outside, as glTF expects.
triangles = [(corners[i], corners[(i + 1) % 4], apex)[::-1] for i in range(4)]
triangles += [(corners[0], corners[1], corners[2]), (corners[0], corners[2], corners[3])]


def normal(a, b, c):
    u = [b[i] - a[i] for i in range(3)]
    v = [c[i] - a[i] for i in range(3)]
    n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
    length = sum(x * x for x in n) ** 0.5
    return [x / length for x in n]


positions, normals = [], []
for tri in triangles:
    n = normal(*tri)
    for p in tri:
        positions += p
        normals += n
count = len(positions) // 3
data = struct.pack(f"<{len(positions)}f", *positions) + struct.pack(f"<{len(normals)}f", *normals)
half = len(data) // 2

gltf = {
    "asset": {"version": "2.0"},
    "scene": 0,
    "scenes": [{"nodes": [0]}],
    "nodes": [{"mesh": 0, "name": "test_pyramid"}],
    "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1}, "material": 0}]}],
    "materials": [
        {
            "pbrMetallicRoughness": {
                "baseColorFactor": [0.9, 0.1, 0.8, 1.0],
                "metallicFactor": 0.0,
                "roughnessFactor": 1.0,
            }
        }
    ],
    "buffers": [{"byteLength": len(data)}],
    "bufferViews": [
        {"buffer": 0, "byteOffset": 0, "byteLength": half, "target": 34962},
        {"buffer": 0, "byteOffset": half, "byteLength": half, "target": 34962},
    ],
    "accessors": [
        {
            "bufferView": 0,
            "componentType": 5126,
            "count": count,
            "type": "VEC3",
            "min": [min(positions[i::3]) for i in range(3)],
            "max": [max(positions[i::3]) for i in range(3)],
        },
        {"bufferView": 1, "componentType": 5126, "count": count, "type": "VEC3"},
    ],
}

json_chunk = json.dumps(gltf, separators=(",", ":")).encode()
json_chunk += b" " * (-len(json_chunk) % 4)
data += b"\0" * (-len(data) % 4)
body = (
    struct.pack("<I4s", len(json_chunk), b"JSON")
    + json_chunk
    + struct.pack("<I4s", len(data), b"BIN\0")
    + data
)
out = Path(__file__).resolve().parent.parent / "assets/models/test_pyramid.glb"
out.parent.mkdir(parents=True, exist_ok=True)
out.write_bytes(struct.pack("<4sII", b"glTF", 2, 12 + len(body)) + body)
print(f"wrote {out}")
