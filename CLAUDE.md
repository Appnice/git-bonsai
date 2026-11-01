# git-bonsai — working notes

Pixel bonsai grown from a GitHub account's public activity. Rust lib + CLI + composite GitHub Action.
Requirements: `~/appnice-apps/projects/git-bonsai/RFC.md` (copy of the Claude Doc). Visual target:
board "07 Commit pixels" (`src/board07.txt`, `~/appnice-apps/projects/git-bonsai/design/`).

## Commands

- `make check` = `cargo fmt --check` + `clippy -D warnings` + `cargo test` (run before every commit).
- `make boards` writes board 07 SVGs to `target/`. Accept golden changes with `INSTA_UPDATE=always cargo test`
  only after looking at the diff.
- Rust is Homebrew rustup, not on PATH: `export PATH=/opt/homebrew/opt/rustup/bin:$PATH`.
- Screenshots of an SVG (frozen CSS animations at given ms): `node ~/.cache/git-bonsai-tools/shot.js <svg> <outPrefix> [scale] [ms,ms]`.

## Layout

`fetch (github/) → Snapshot (snapshot.rs) → grow (grow.rs, rules.rs) → TreeState (state.rs) → layout/ → Grid (grid.rs) → render/ (svg, ascii, gif)`.
Palette and themes: `palette.rs`. Board 07 reference scene: `reference.rs`.

## Rules

- Pure functions in `rules`, `grow`, `layout`, `render`; I/O only in `github/` and `main.rs`.
- Deterministic: no clocks, no `HashMap` iteration, no thread RNG in the render path; RNG = `ChaCha8Rng`
  seeded from the state seed + element name. Same state → same bytes.
- Every change to a tree's shape updates the golden ASCII/SVG in the same commit.
- No `unwrap()` outside tests; `thiserror` in the lib, `anyhow` in the CLI.
- Never change `state.json` without bumping `schema` and adding a migration with a test.
- Never add `.github/workflows/` (no GitHub Actions on appnice repos). `action.yml` is the product.
- Tests never touch the network; snapshots come from `tests/fixtures/`.
- Commit as `developer@appnice.dev` (local git config); stage explicit paths only.
