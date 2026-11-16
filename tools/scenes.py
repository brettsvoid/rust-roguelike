#!/usr/bin/env python3
"""Runs harness scenes in one batch and checks them.

    tools/scenes.py                 every scene in tools/scenes.json
    tools/scenes.py combat cover    only these (prefix match with a trailing *)
    tools/scenes.py --list          names and notes
    tools/scenes.py --jobs 2        run two games at a time (default 2)
    tools/scenes.py --out DIR       where the PNGs, logs and sheet go

Each scene runs in its own scratch folder with a copy of tools/scenes-town.json,
so it never touches the town.json or save in the repo and always sees the same
town. A scene passes when the game exits cleanly and every `expect` string
appears in its game log, in order. The run ends with one table and one contact
sheet (sheet.png) of every capture: read the table first, then look at the
sheet once, instead of opening a picture per change.
"""

import argparse
import concurrent.futures
import json
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "tools" / "scenes.json"
TOWN = ROOT / "tools" / "scenes-town.json"
TIMEOUT_SECS = 180
SHEET_COLUMNS = 4
SHEET_TILE = "800x500"
FONT = "/System/Library/Fonts/Supplemental/Arial.ttf"


def in_order(lines, expected):
    """The first expected string not found, searching the log in order."""
    at = 0
    for want in expected:
        hit = next((i for i in range(at, len(lines)) if want in lines[i]), None)
        if hit is None:
            return want
        at = hit + 1
    return None


def run_scene(scene, out):
    name = scene["name"]
    png = out / f"{name}.png"
    png.unlink(missing_ok=True)
    work = Path(tempfile.mkdtemp(prefix=f"scene-{name}-"))
    shutil.copy(TOWN, work / "town.json")
    command = [
        "cargo", "run", "-q", "--manifest-path", str(ROOT / "Cargo.toml"), "--",
        "--screenshot", str(png), *shlex.split(scene["args"]),
    ]
    started = time.monotonic()
    try:
        done = subprocess.run(command, cwd=work, capture_output=True, text=True, timeout=TIMEOUT_SECS)
        output, problem = done.stdout + done.stderr, None
        if done.returncode != 0:
            problem = f"exit code {done.returncode}"
    except subprocess.TimeoutExpired as hung:
        output = (hung.stdout or b"").decode(errors="replace") + (hung.stderr or b"").decode(errors="replace")
        problem = f"hung for {TIMEOUT_SECS}s"
    shutil.rmtree(work, ignore_errors=True)
    log = [line.split("log> ", 1)[1] for line in output.splitlines() if "log> " in line]
    (out / f"{name}.log").write_text("\n".join(log) + "\n")
    for line in output.splitlines():
        if "panicked" in line or "B0004" in line:
            problem = problem or line.strip()[:120]
    if problem is None and not png.exists():
        problem = "no capture"
    if problem is None:
        missing = in_order(log, scene.get("expect", []))
        if missing is not None:
            problem = f"log lacks: {missing}"
    return name, problem, time.monotonic() - started


def contact_sheet(names, out):
    if not shutil.which("magick"):
        return None
    tiles = []
    for name in names:
        tile = out / f".tile-{name}.png"
        label = ["-font", FONT, "-pointsize", "22", "-fill", "white", "-undercolor", "#000000c0",
                 "-gravity", "NorthWest", "-annotate", "+8+6", f" {name} "] if Path(FONT).exists() else []
        subprocess.run(["magick", str(out / f"{name}.png"), "-resize", SHEET_TILE, *label, str(tile)], check=True)
        tiles.append(tile)
    rows = [tiles[i:i + SHEET_COLUMNS] for i in range(0, len(tiles), SHEET_COLUMNS)]
    command = ["magick"]
    for row in rows:
        command += ["(", *map(str, row), "+append", ")"]
    sheet = out / "sheet.png"
    subprocess.run([*command, "-background", "black", "-append", str(sheet)], check=True)
    for tile in tiles:
        tile.unlink()
    return sheet


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("names", nargs="*")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--out", type=Path, default=Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "scenes")
    args = parser.parse_args()

    scenes = json.loads(MANIFEST.read_text())["scenes"]
    if args.list:
        for scene in scenes:
            print(f"{scene['name']:<14} {scene.get('note', '')}")
        return 0
    if args.names:
        def wanted(name):
            return any(name == n or (n.endswith("*") and name.startswith(n[:-1])) for n in args.names)
        scenes = [scene for scene in scenes if wanted(scene["name"])]
        if not scenes:
            sys.exit("no scene matches " + " ".join(args.names))
    args.out.mkdir(parents=True, exist_ok=True)

    build = subprocess.run(["cargo", "build", "-q", "--manifest-path", str(ROOT / "Cargo.toml")])
    if build.returncode != 0:
        sys.exit("build failed")

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        results = list(pool.map(lambda scene: run_scene(scene, args.out), scenes))

    failed = 0
    for name, problem, seconds in results:
        failed += problem is not None
        print(f"{'FAIL' if problem else 'ok  '} {name:<14} {seconds:5.1f}s  {problem or ''}")
    captured = [name for name, _, _ in results if (args.out / f"{name}.png").exists()]
    sheet = contact_sheet(captured, args.out) if captured else None
    print(f"{len(results) - failed} of {len(results)} passed. Logs and captures in {args.out}")
    if sheet:
        print(f"Contact sheet: {sheet}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
