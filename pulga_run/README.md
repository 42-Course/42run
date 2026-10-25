# pulga_run

The `42run` game itself — a Temple Run–style endless runner, built per the
`subject.en.pdf` spec in the [42Run](..) root (this crate is a member of its Cargo workspace). Builds the `42run`
binary (see `[[bin]]` in `Cargo.toml`).

Depends only on [`pulga_engine`](../pulga_engine) (game loop, scene,
renderer, HUD) — no GL or windowing code lives here.

## What's playable now

Arrow keys move between 3 lanes, Space jumps, cube obstacles scroll toward
you and end the run on collision (Space restarts), Escape quits. An on-screen
HUD shows the distance and a game-over message. All geometry is procedural
(`primitives::cube` / `ground_plane`), drawn as flat-colored engine entities.

```sh
make run     # from the repo root: cargo run --bin 42run (needs a display)
make         # from the repo root: release build -> ./42run
make test    # from the repo root
```

## Not built yet

- A campus-themed set (currently a flat procedural ground plane).
- The "glued-together tunnel/block" scene-building system described in the
  root README (currently a straight, unobstructed lane).
- Textures.
- Slide, power-ups, or any bonus-part ideas.
