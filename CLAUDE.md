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

The app is a git commit graph visualizer built on the Rinch GUI framework. Commits are displayed as a scrollable list of regular UI elements, with inline SVG for the graph lane visualization.

### Data flow

1. **`git.rs`** — Opens repos via `git2`, walks commits (topological+time order, max 500), collects branch labels. `scan_for_repos` finds repos up to 2 levels deep in a directory.
2. **`graph.rs`** — `assign_lanes` does greedy lane (column) assignment: first parent stays in lane, merge parents get new lanes. `build_edges` creates edge structs. `compute_row_graph_data` produces per-row `RowGraphData` with lane segments and cross-lane connectors for SVG rendering. `build_graph_state` combines all into `GraphState`.
3. **`main.rs`** — Loads repos, builds `CommitRow` display structs (pre-computed SVG, message HTML), wires Rinch signals, renders a scrollable list via `rsx!`. Each row has an SVG graph column (via `dangerous_inner_html`) and text columns for hash, message, author, date. Click opens a detail side panel.

### Key types (`state.rs`)

- `CommitNode` — single commit with OID, message, author, lane/row assignment, branch labels
- `GraphState` — all commits + per-row graph data for one repo
- `RowGraphData` / `LaneSegment` — per-row rendering instructions (which lanes have lines, nodes, connectors)
- `AppState` — holds multiple `RepoView`s for multi-repo mode
- Layout constants: `LANE_WIDTH`, `ROW_HEIGHT`

### Rendering model

The graph column uses inline SVG per row — vertical line segments for active lanes, circles for commit nodes, horizontal lines for cross-lane connectors. The SVG is pre-generated as HTML strings and injected via `dangerous_inner_html`. All other UI (commit info, toolbar, detail panel) uses standard Rinch components and rsx elements.
