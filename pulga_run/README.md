# pulga_run

The `42run` game itself — a Temple Run–style endless runner, built per the
`subject.en.pdf` spec in the [42Run](..) root (this crate is a member of its Cargo workspace). Builds the `42run`
binary (see `[[bin]]` in `Cargo.toml`).

Depends on [`pulga_win_lib`](../pulga_win_lib) (windowing/OpenGL/mesh/shader/
camera), [`pulga_gui_lib`](../pulga_gui_lib) (GUI, not wired in yet), and the
published [`pulgamecanica_matrix`](https://crates.io/crates/pulgamecanica_matrix)
crate (matrix/transform math).

## What's playable now

Arrow keys move between 3 lanes, Space jumps, cube obstacles scroll toward
you and end the run on collision (Space restarts), the window title shows a
live distance meter. All geometry is procedural (`pulga_win_lib::primitives`)
using the same shaded-cube/plane pipeline the OBJ-mesh demo uses.

```sh
make run     # from the repo root: cargo run --bin 42run (needs a display)
make         # from the repo root: release build -> ./42run
make test    # from the repo root
```

## Not built yet

- A campus-themed set (currently a flat procedural ground plane).
- The "glued-together tunnel/block" scene-building system described in the
  root README (currently a straight, unobstructed lane).
- Textures, a real HUD (distance meter is the window title, not on-screen
  text — `pulga_gui_lib` has no text rendering yet either).
- Slide, power-ups, or any bonus-part ideas.
