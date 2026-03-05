# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build                    # build
cargo run                      # run against current directory
cargo run -- /path/to/repo     # run against specific repo or folder of repos
```

No tests exist yet. The `git2` crate uses vendored libgit2, so no system libgit2 is needed.

## Dependencies

- **rinch** — GUI framework from `https://github.com/joeleaver/rinch` (features: desktop, components, theme, debug, file-dialogs). Fetched automatically as a git dependency.
- **git2** — git repository access with vendored libgit2.

Rust edition 2024.

## Architecture

The app is a git commit graph visualizer built on the Rinch GUI framework. It renders commit graphs to a CPU pixel buffer displayed via Rinch's `RenderSurface`.

### Data flow

1. **`git.rs`** — Opens repos via `git2`, walks commits (topological+time order, max 500), collects branch labels. `scan_for_repos` finds repos up to 2 levels deep in a directory.
2. **`graph.rs`** — `assign_lanes` does greedy lane (column) assignment: first parent stays in lane, merge parents get new lanes. `build_edges` creates parent-child edge structs. `build_graph_state` combines both into a `GraphState`.
3. **`render.rs`** — CPU software renderer. Draws directly into RGBA `Vec<u8>` pixel buffers using custom primitives (anti-aliased circles, lines, bezier curves, 5x7 bitmap font). Supports single-repo and tiled multi-repo rendering with clip rects. Also provides `hit_test` for click detection on commit nodes.
4. **`main.rs`** — Wires everything together: loads repos, creates Rinch signals for UI state, sets up render/event callbacks on the surface, builds the toolbar/canvas/status bar/detail panel via `rsx!`.

### Key types (`state.rs`)

- `CommitNode` — single commit with OID, message, author, lane/row assignment, branch labels
- `GraphState` — all commits + edges + view transform (zoom, pan, selection) for one repo
- `AppState` — holds multiple `RepoView`s for multi-repo mode
- Layout constants: `NODE_RADIUS`, `LANE_WIDTH`, `ROW_HEIGHT`
- Coordinate helpers: `commit_position`, `world_to_screen`, `screen_to_world`

### Rendering model

All rendering is CPU-based — no GPU/WebGL. The render callback fires on dirty flag or resize, produces a pixel buffer, and submits it to the surface. Mouse events (drag to pan, scroll to zoom, click to select) update `GraphState` transform and set dirty. Multi-repo mode tiles repos in a grid with clip rects per cell.
