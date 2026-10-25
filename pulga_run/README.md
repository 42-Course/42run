# pulga_run

The `42run` game itself — a Temple Run–style endless runner, built per the
`subject.en.pdf` spec in the [42Run](..) root — as a
[`pulga_engine`](../pulga_engine) **project**:

- `project.toml` — the tiles (track pieces, obstacles, scenery), edited with
  `pulga_editor` (`make edit` from the root). Embedded into the binary at
  compile time.
- `src/main.rs` — the game's rules in Rust: lanes, jump, speed, collisions,
  HUD. No GL or windowing code lives here.

## What's playable now

Arrow keys move between 3 lanes, Space jumps, Escape quits. The track is
glued from the project's straight tiles in random order (the first two are
obstacle-free); hitting an obstacle prop ends the run (Space restarts). An
on-screen HUD shows the distance and a game-over message.

```sh
make run     # from the repo root: cargo run --bin 42run (needs a display)
make         # from the repo root: release build -> ./42run
make edit    # from the repo root: open this project in the editor
```

## Not built yet

- Turning at left/right/fork tiles (they exist in the project and in editor
  previews; the game skips them until the runner can turn).
- A campus-themed set, textures, slide, power-ups.
