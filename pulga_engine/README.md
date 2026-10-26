# pulga_engine

The 42run game engine: a library games are written against in Rust, plus
**`pulga_editor`**, a scene designer for a game's zones.

Drawing, windowing, input, text and widgets live in the libraries
([`pulga_win_lib`](../pulga_win_lib), [`pulga_gui_lib`](../pulga_gui_lib)).
The engine is what turns them into a game:

| Module | What it does |
|---|---|
| `lib.rs` | `Game` trait + `run()`: frame loop, clamped `dt`, input → UI, held keys, mouse ray, clean quit |
| `scene` | `World` of entities: transform hierarchy, generation-checked ids, cascading despawn, overlay (on-top) entities |
| `zone` | the designed pieces of track: box props (solid = has mass), kinds (straight / left / right / fork) with exit sockets |
| `project` | zones + **groups** (a zone made of zones and groups, in sequence or shuffled); the free-mode pool = whatever no group holds; `project.toml` |
| `track` | the endless track: a runway, then the source's zones glued on their sockets, forks branching until the runner commits; never crosses itself |
| `runner` | the runner: lanes, Temple Run turns at junctions (same keys), jump, collisions from mass (stand on, crash into, fall off) |
| `physics` | AABBs and ray casts |
| `gizmo` | the move gizmo: sphere-tipped axis arrows, picking and axis dragging |
| `history` | undo/redo by snapshots |
| `view` | fly camera (editing) and chase camera (running) |
| `render` | the engine's meshes (built-in cube and sphere), fog, drawing the world, overlay and UI through the libraries |

## Projects

A project is a directory with a `project.toml` (zones and groups, written by
the editor) and, optionally, a Rust crate that depends on `pulga_engine` and
implements `Game` — [`pulga_run`](../pulga_run) is one. The game embeds the
file at build time (`include_str!("../project.toml")`).

## The editor

```sh
make editor                      # run it on ../pulga_run
make editor PROJECT=path/to/dir  # any project (created if missing)
```

- **Scene** (left): zones and groups; groups expand to show their contents.
  `+ New` makes zones of any kind and groups. Duplicate, delete (with
  confirmation, undoable).
- **Viewport**: the selected zone, or a group's zones glued in order, on a
  grid with entry/exit arrows. Hold the right mouse button to look, fly with
  WASD/Q/E (Shift faster), wheel to move, `F` to focus. Click a box to
  select it and drag the arrows of its move gizmo; `A` adds a box under the
  cursor.
- **Inspector** (right): name, kind, boxes (position/size fields you drag or
  type into, solid toggle, color), group order and contents (reorder, add,
  remove), group membership.
- **Run preview** (`P`): the game's runner on a track repeating the
  selection, in the viewport.
- `Ctrl+Z`/`Ctrl+Y` undo/redo, `Ctrl+S` save (also on close), `Ctrl+D`
  duplicate, Delete, F5 plays the game (`cargo run --release` in the
  project). Dark and light themes.
- Startup options for scripted screenshots: `--select NAME`, `--prop N`,
  `--preview`, `--light` (with `PULGA_SCREENSHOT=out.png`).

## Usage

```sh
make build    # library + editor
make test     # unit + doc tests (no display needed)
make doc      # API docs
make          # make help
```
