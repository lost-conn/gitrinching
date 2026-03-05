# gitrinching

A git commit graph visualizer built with [rinch](https://github.com/joeleaver/rinch), featuring CPU-rendered graphs with zoom, pan, and multi-repo support.

![Rust](https://img.shields.io/badge/rust-2024_edition-orange)

## Features

- **Commit graph visualization** — lane-based layout with bezier curves for merge edges
- **Interactive** — click commits for details, drag to pan, scroll to zoom
- **Multi-repo mode** — point at a directory of repos to view them in a tiled grid
- **Branch labels** — shows branch names and HEAD indicator on commit nodes
- **Browse** — file dialog to pick repositories or folders

## Usage

```bash
# View current directory
cargo run

# View a specific repo
cargo run -- /path/to/repo

# View all repos in a directory (scans 2 levels deep)
cargo run -- ~/projects
```

## Building

Requires Rust (edition 2024). The [rinch](https://github.com/joeleaver/rinch) framework is fetched automatically via git dependency.

```bash
cargo build
cargo build --release
```

No system dependencies needed — `libgit2` is vendored automatically.

## Controls

| Action | Input |
|--------|-------|
| Pan | Click and drag |
| Zoom | Scroll wheel |
| Select commit | Click on a node |
| Close detail panel | Click **X** |

## Screenshot

*Coming soon*

## Planned Features

- Remote repo visualization
- Checkout from the graph
- Drag 'n drop to merge
- Making it look a lil' nicer
- Other cool stuff, maybe, idk