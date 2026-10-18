# pulga_run

The `42run` game itself — a Temple Run–style endless runner set in the 42
school campus, built per the `subject.en.pdf` spec in the
[42Run](../42Run) root. Builds the `42run` binary (see `[[bin]]` in
`Cargo.toml`).

Depends on [`PulgaWinLib`](../PulgaWinLib) (windowing/OpenGL),
[`PulgaGuiLib`](../PulgaGuiLib) (GUI), and the published
[`pulgamecanica_matrix`](https://crates.io/crates/pulgamecanica_matrix) crate
(matrix/transform math).

## Usage

```sh
make build   # cargo build
make run     # cargo run -> ./42run
make test    # cargo test
make         # make help
```
