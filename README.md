# Like Tempe run for 42 school

All projects are Rust. This repo is the Cargo workspace for the game
(`pulga_run`, the `42run` binary). The two libraries are **their own git
repos**, cloned into this root and gitignored here, so they stay usable on
their own:

- `pulga_win_lib` — windowing/graphics lib (OpenGL/Vulkan, parsing, camera, shaders, 2d/3d canvas) — [repo](https://github.com/pulgamecanica/pulga_win_lib)
- `pulga_gui_lib` — GUI widgets, depends on `pulga_win_lib` — [repo](https://github.com/pulgamecanica/pulga_gui_lib)
- `pulga_run` — the game binary, part of this repo, depends on both libs

`pulga_run` also depends on [`pulgamecanica_matrix`](https://crates.io/crates/pulgamecanica_matrix)
(matrix/3d-transform math), already published on crates.io.

## Makefile

`make` clones the libs if they're missing and builds the release `./42run`.
`make test` / `make fmt` / `make clippy` / `make clean` / `make fclean` /
`make re` cover the libs and the workspace; `make run` runs the game;
`make help` lists everything.

## Releases

Pushing a `v*` tag runs `.github/workflows/release.yml`: it bundles this repo
plus the libs (at their current `main`, commits listed in the release notes)
into one self-contained source tarball, checks it builds without cloning
anything, and attaches it to the GitHub release.

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
