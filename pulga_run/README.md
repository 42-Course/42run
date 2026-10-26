# pulga_run

The `42run` game itself — a Temple Run–style endless runner, built per the
`subject.en.pdf` spec in the [42Run](..) root — as a
[`pulga_engine`](../pulga_engine) **project**:

- `project.toml` — the zones (track pieces, obstacles, scenery) and groups,
  designed in `pulga_editor` (`make edit` from the root). Embedded into the
  binary at compile time.
- `src/main.rs` — free mode's rules in Rust: speed ramp, restart, best
  distance, HUD. Track, runner and collisions come from the engine.

## How it plays

Left/Right (or A/D) change lane — or turn, when you're at a junction; miss
the turn and you run off the floor. Space/Up/W jumps. Solid boxes are
obstacles: jump the low ones, dodge the rest. The track starts with a short
runway, then glues the project's free-mode pool at random (forks included);
it speeds up as you go. Escape quits.

```sh
make run     # from the repo root: cargo run --bin 42run (needs a display)
make         # from the repo root: release build -> ./42run
make edit    # from the repo root: open this project in the editor
```

## Not built yet

- A campus-themed set (textures, 42 architecture), slide, power-ups, coins.
