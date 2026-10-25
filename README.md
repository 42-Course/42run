# Like Tempe run for 42 school

All projects are Rust. This repo is the Cargo workspace for the engine
(`pulga_engine`) and the game (`pulga_run`, the `42run` binary). The two libraries are **their own git
repos**, cloned into this root and gitignored here, so they stay usable on
their own:

- `pulga_win_lib` — windowing/graphics lib (OpenGL/Vulkan, parsing, camera, shaders, 2d/3d canvas) — [repo](https://github.com/pulgamecanica/pulga_win_lib)
- `pulga_gui_lib` — GUI widgets, depends on `pulga_win_lib` — [repo](https://github.com/pulgamecanica/pulga_gui_lib)
- `pulga_engine` — the game engine, part of this repo: game loop + frame timing, scene (`World` of entities with a transform hierarchy), renderer (the only code that calls GL), HUD text with a built-in bitmap font. Depends on both libs
- `pulga_run` — the game binary, part of this repo: only the game's rules, on top of `pulga_engine`

`pulga_run` also depends on [`pulgamecanica_matrix`](https://crates.io/crates/pulgamecanica_matrix)
(matrix/3d-transform math), already published on crates.io.

## Makefile

`make` clones the libs if they're missing and builds the release `./42run`.
`make test` / `make fmt` / `make clippy` / `make clean` / `make fclean` /
`make re` cover the libs and the workspace; `make run` runs the game;
`make help` lists everything.

## Releases

Every push to `main` runs `.github/workflows/release.yml` and publishes a
GitHub release named after the commit's secret codename (`make release-name`,
e.g. "Lunar Bocal") with the unique tag `make release-tag` (`v<version>-<sha>`).
Files are prefixed `make release-pkg` (`42run-v<version>-<Codename>`):

- `42run-v0.1.0-Lunar-Bocal-src.tar.gz` — this repo + the libs, self-contained (`make dist`)
- `…-linux-x86_64.tar.gz`, `…-linux-aarch64.tar.gz` (built on Ubuntu 22.04 for older-glibc compatibility)
- `…-windows-x86_64.zip`
- `…-macos-universal.tar.gz` (Intel + Apple Silicon, unsigned)

Each binary is built from the source tarball itself, so a release proves its
own source builds on every OS. The lib commits that went in are in the
release notes. Branches and PRs only get `ci.yml` (build + test).

---

## pulga_win_lib
	Graphical library which allows compilation for any system, and deployment for a fully prepared and packed application (Windoxs, iOS, Android, iPhone, Linux - deb, package)

	- OpenGL support (this project specifically will use OpenGL version)
	- Vulkan support
	- Objct Parsing
	- Camera support
	- Transformations supprot
	- Shaders & Renderizer support
	- 2d & 3d canvas support

## pulga_gui_lib: An underlying library which depends on pulga_win_lib and includes a bunch of ready GUI elements, such as text support, Canvases, Selectors; etc

## pulgamecanica_matrix:
	location: crates.io/crates/pulgamecanica_matrix (already published, used as a regular dependency)
	it's a library with mathematical support of matrixes and even contains a transformation helper for 3d perspective view matrix

---

This Game consists on a templerun-like game which respects the subject (subject.en.pdf)

The theme will be 42 and the scenes will be designed to resamble 42school.

There must be a 'scene' creator which alows you to create 'spaces' and link other spaces, and even apply 'effect's to spaces to edit them and create kinda like 'tunnels' which will be linked (glued) togeather, to create an infinite map. The edit mode aows you to expore the space and pllace items; etc. I am not entirely sure but I believe there are 'limites' amount of types for each 'scene' or tunnel, I would like to chat more about this, I believe that the correct way to do it is to do 'sware zone by square zone, and building bocks, each block (square) has a type, and you can build 'tunnels' from those types, for exampe straight (south to north), turn left (south to west), turn right (south to east), turn either side (south to east/west).

The game is challenging and you can jump, slide, have powers, stay to either side, turn to a side.

The engine itself should allow for the creation and editing of the game itself and playing it as well
