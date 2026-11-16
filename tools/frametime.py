#!/usr/bin/env python3
"""Frame-time benchmark: runs fixed harness scenes on a release binary and reports frame times.

    cargo build --release --no-default-features
    tools/frametime.py                          # every scene, 3 runs each, target/release binary
    tools/frametime.py --bin a --bin b town     # two binaries, runs interleaved a, b, a, b, ...
    tools/frametime.py --runs 5 --frames 900 dungeon combat

Each run is the harness with FRAME_TIME_LOG=1: the window stays hidden, the picture is drawn
off-screen at 3200x2000 (HARNESS_SCALE=2, the pixels of a Retina window; --scale 1 is the
test captures' 1600x1000), the GPU is held to two frames in flight like a window's swap
chain, and frames past the warm-up are timed by the wall clock. A run holds the scene
runner's machine-wide lock exclusively and waits while any other harness game runs (a
runner whose temp dir differs takes a different lock); a run another game overlapped is
taken again. The 1-minute load and the GPU utilisation (other apps use the GPU too) are
noted before each run, and the CPU seconds the game used; repeat a run that looks off.
The last run of each scene and binary leaves its capture in --out, to compare by eye.

"GPU wait" is how long the render thread waited each frame for the GPU to finish the frame
two back: near zero means the CPU limits the frame, most of the frame means the GPU does.
"""

import argparse
import fcntl
import os
import re
import resource
import shlex
import shutil
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GPU_LOCK = Path(tempfile.gettempdir()) / "abyss-scene.lock"
TOWN = ROOT / "tools" / "scenes-town.json"

MONSTERS = ["Goblin", "Orc", "Kobold", "Skeleton", "Rat", "Hobgoblin", "Orc Warrior", "Orc Shaman", "Wraith", "Imp",
            "Hellhound", "Giant Spider", "Giant Bat", "Lich", "Goblin", "Orc", "Skeleton", "Kobold", "Imp", "Wraith"]
# Twenty monsters on a ring two to five tiles from the active hero.
OFFSETS = [(dx, dy) for dy in (-5, -3, 3, 5) for dx in (-4, -2, 0, 2, 4)]
HORDE = " ".join(f'--console "place {dx} {dy} {name}"' for (dx, dy), name in zip(OFFSETS, MONSTERS))
# The deepest level, an open one, fully explored (`reveal`) with the fog of war on, as late
# in a level's play; the full party of four.
DUNGEON = f"--seed 3 --depth 12 --console godmode --console reveal {HORDE}"

SCENES = {
    "dungeon": DUNGEON,
    "dungeon_far": f'{DUNGEON} --keys "-------"',
    # Every tile lit and in view (`--reveal-map` turns the fog off): more than play ever shows.
    "dungeon_nofog": DUNGEON.replace("--console reveal", "--reveal-map"),
    "town": '--seed 42 --depth 0 --console reveal --console "teleport 12 23"',
    "combat": "--seed 42 --depth 2 --console reveal --console godmode "
              + " ".join(f'--console "place {dx} -3 {name}"' for dx, name in zip((-2, 0, 2, 4), MONSTERS))
              + ' --do "effect fireball" --keys e --do "effect fireball" --keys e --do "effect missile" --keys e'
              + ' --do "effect fireball" --keys e --do "effect fireball" --keys e --do "effect missile" --keys e',
}

SCALE = "2"
TIME_LINE = re.compile(r"frame time: median ([\d.]+) ms, p95 ([\d.]+) ms, mean ([\d.]+) ms .*GPU wait ([\d.]+) ms")


def gpu_utilisation():
    out = subprocess.run(["ioreg", "-r", "-c", "IOAccelerator", "-d", "1"], capture_output=True, text=True).stdout
    found = re.search(r'"Device Utilization %"=(\d+)', out)
    return int(found.group(1)) if found else -1


def other_games(own=None):
    """Harness games running beside ours: a runner that skips the lock still shares the GPU."""
    out = subprocess.run(["pgrep", "-f", "--", "--screenshot"], capture_output=True, text=True).stdout.split()
    return [pid for pid in map(int, out) if pid not in (own, os.getpid())]


def run(binary, args, frames, out_png):
    """One timed run, taken again while another game shared the machine."""
    for _ in range(10):
        result, shared = run_once(binary, args, frames, out_png)
        if not shared:
            return result
        print(f"  (another game ran beside {Path(binary).name}; running again)", flush=True)
    raise SystemExit("other games kept running; no clean measurement")


def run_once(binary, args, frames, out_png):
    work = Path(tempfile.mkdtemp(prefix="frametime-"))
    shutil.copy(TOWN, work / "town.json")
    env = dict(os.environ, FRAME_TIME_LOG="1", HARNESS_SCALE=SCALE, BEVY_ASSET_ROOT=str(ROOT))
    command = [binary, "--screenshot", str(out_png), "--frames", str(frames), *shlex.split(args)]
    with open(GPU_LOCK, "w") as turn:
        fcntl.flock(turn, fcntl.LOCK_EX)
        while other_games():
            time.sleep(1)
        time.sleep(2)  # the GPU figure is a recent average, so let the last run's work fall out of it
        load = os.getloadavg()[0]
        gpu = gpu_utilisation()
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        with open(work / "out.log", "w") as log:
            game = subprocess.Popen(command, cwd=work, stdout=log, stderr=subprocess.STDOUT, env=env)
            shared = False
            while game.poll() is None:
                shared = shared or bool(other_games(game.pid))
                time.sleep(0.5)
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        output = (work / "out.log").read_text(errors="replace")
    shutil.rmtree(work, ignore_errors=True)
    if shared:
        return None, True
    cpu = (after.ru_utime + after.ru_stime) - (before.ru_utime + before.ru_stime)
    found = TIME_LINE.search(output)
    if game.returncode != 0 or not found:
        tail = "\n".join(output.splitlines()[-15:])
        raise SystemExit(f"{binary} {args}: exit {game.returncode}, no frame time line\n{tail}")
    scene = re.search(r"scene: (.*)", output)
    median, p95, mean, wait = map(float, found.groups())
    return {"median": median, "p95": p95, "mean": mean, "wait": wait, "load": load, "gpu": gpu, "cpu": cpu,
            "scene": scene.group(1) if scene else ""}, False


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("scenes", nargs="*", default=list(SCENES))
    parser.add_argument("--bin", action="append", help="release binary (repeatable)")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--frames", type=int, default=600, help="frames per run; the first 120 are warm-up")
    parser.add_argument("--out", type=Path, default=Path(tempfile.gettempdir()) / "frametime")
    parser.add_argument("--scale", default="2", help="pixels per layout pixel: 2 is a Retina window, 1 the test captures")
    args = parser.parse_args()
    global SCALE
    SCALE = args.scale
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    binaries = args.bin or [str(target / "release" / "rust-roguelike")]
    args.out.mkdir(parents=True, exist_ok=True)
    print(f"{'scene':<12} {'binary':<16} {'median ms':>10} {'p95 ms':>8} {'fps':>5} {'GPU wait':>9} {'CPU s':>6}"
          "  runs (median/p95, load, GPU% before)")
    for name in args.scenes:
        results = {b: [] for b in binaries}
        for _ in range(args.runs):
            for b in binaries:
                results[b].append(run(b, SCENES[name], args.frames, args.out / f"{name}-{Path(b).name}.png"))
        for b, rs in results.items():
            median = statistics.median(r["median"] for r in rs)
            p95 = statistics.median(r["p95"] for r in rs)
            wait = statistics.median(r["wait"] for r in rs)
            cpu = statistics.median(r["cpu"] for r in rs)
            each = ", ".join(f"{r['median']:.1f}/{r['p95']:.1f} ({r['load']:.1f}, {r['gpu']}%)" for r in rs)
            print(f"{name:<12} {Path(b).name:<16} {median:>10.2f} {p95:>8.2f} {1000 / median:>5.0f} {wait:>8.2f} {cpu:>6.1f}  {each}")
        print(f"{'':<12} {rs[-1]['scene']}", flush=True)


if __name__ == "__main__":
    main()
