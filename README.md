# The Abyss Below

An isometric 3D tactical roguelike built with Bevy. Lead a party of four heroes
down twelve levels, take the Heart of the Abyss and bring it home. Fights are
turn-based with action points, hit chances, cover and overwatch. The town
trades what you sell, grows, and outlives every party.

It began as the [Roguelike Tutorial](https://bfnightly.bracketproductions.com/rustbook/)
in ASCII. The design is in [docs/design.md](docs/design.md) and the overhaul's
ticket list in [docs/TODO.md](docs/TODO.md).

## Controls

| Input | Action |
|-----|--------|
| Left click | Walk, attack, pick up, take stairs, talk to a vendor, select a hero |
| Tab | Next hero |
| W A S D | Pan the camera (C recentres, scroll or pinch zooms) |
| G / I / X / R | Pick up / inventory / drop / remove equipment |
| . | Take the stairs |
| Space | Wait |
| 1 / 2 | The active hero's abilities |
| O / H / E | Overwatch / hunker down / end turn (combat) |
| T | Town ledger |
| Escape | Cancel or close a menu |
| Q | Save and quit |

## Running

```bash
cargo run
```

Desktop only.
