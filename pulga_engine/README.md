# pulga_engine

The 42run game engine: a library games are written against in Rust, plus
**`pulga_editor`**, the program you build a game's scenery with.

Drawing, windowing, input and text live in the libraries
([`pulga_win_lib`](../pulga_win_lib), [`pulga_gui_lib`](../pulga_gui_lib)).
The engine is what turns them into a game:

| Module | What it does |
|---|---|
| `lib.rs` | `Game` trait + `run()`: frame loop, clamped `dt`, camera aspect, mouse ray, UI queue, clean quit |
| `scene` | `World` of entities: transform hierarchy, generation-checked ids, cascading despawn |
| `tile` | the building blocks ("cubes") a run is glued from: box props, kinds (straight/left/right/fork), exit sockets |
| `track` | glues tiles end to end on their sockets, never letting the track cross itself |
| `project` | `project.toml`: what the editor saves and the game embeds |
| `physics` | AABBs, ray casts (mouse picking, hits) |
| `render` | the engine's meshes + drawing the world and UI through the libraries |

## Projects

A project is a directory with a `project.toml` (tiles, written by the
editor) and, optionally, a Rust crate that depends on `pulga_engine` and
implements `Game` — [`pulga_run`](../pulga_run) is one. The game embeds the
file at build time (`include_str!("../project.toml")`).

## The editor

```sh
make editor                      # run it on ../pulga_run
make editor PROJECT=path/to/dir  # any project (created if missing)
```

Left panel: tiles (click to edit), add/delete tiles, save, preview a
generated run, play (`cargo run --release` in the project). Viewport:
right-drag orbits, wheel zooms, click selects a prop, `A` adds a box under
the cursor; arrows/`Q`/`E` move the selection (Shift resizes), `O` toggles
obstacle, `C` color, `D` duplicate, Delete removes; `K` cycles the tile's
kind, Tab next tile, `Ctrl+S` saves, `P` previews, F5 plays. Unsaved changes
are saved on close.

## Usage

```sh
make build    # library + editor
make test     # unit + doc tests (no display needed)
make doc      # API docs
make          # make help
```
