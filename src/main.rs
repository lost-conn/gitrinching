mod git;
mod graph;
mod state;

use std::path::Path;
use std::sync::{Arc, Mutex};

use rinch::prelude::*;

use state::{AppState, CommitNode, RepoView, LANE_WIDTH, ROW_HEIGHT, lane_color_hex};

fn format_timestamp(ts: i64) -> String {
    let secs = ts as u64;
    let days = secs / 86400;
    let y = 1970 + (days * 400 / 146097);
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let day_of_year = days - (365 * (y - 1970) + ((y - 1970) + 1) / 4);
    let month = (day_of_year * 12 / 366).min(11) + 1;
    let day = (day_of_year - (month - 1) * 30).max(1);
    format!("{y:04}-{month:02}-{day:02} {h:02}:{m:02}")
}

/// A single CSS element to place in the graph cell.
#[derive(Clone, Debug, PartialEq)]
struct GraphElement {
    style: String,
}

/// Pre-computed display data for one commit row.
#[derive(Clone, Debug, PartialEq)]
struct CommitRow {
    oid: String,
    short_oid: String,
    author: String,
    date: String,
    message: String,
    branch_labels: Vec<String>,
    is_head: bool,
    graph_width: f32,
    graph_elements: Vec<GraphElement>,
    commit: CommitNode,
}

/// Build graph CSS elements for one row.
fn build_graph_elements(row_data: &state::RowGraphData, max_lanes: usize) -> Vec<GraphElement> {
    let mut elems = Vec::new();
    let h = ROW_HEIGHT;
    let mid_y = h / 2.0;
    let line_w = 2.0;
    let node_r = 5.0;

    let cx = |lane: usize| -> f32 { lane as f32 * LANE_WIDTH + LANE_WIDTH / 2.0 };

    // Vertical line segments
    for (lane, seg) in row_data.lanes.iter().enumerate() {
        if !seg.is_active() {
            continue;
        }
        let color = lane_color_hex(seg.color_index);
        let x = cx(lane) - line_w / 2.0;

        if seg.line_top && seg.line_bottom && !seg.has_node {
            // Full pass-through line
            elems.push(GraphElement {
                style: format!(
                    "position: absolute; left: {x}px; top: 0; width: {line_w}px; height: {h}px; background: {color};"
                ),
            });
        } else {
            if seg.line_top {
                elems.push(GraphElement {
                    style: format!(
                        "position: absolute; left: {x}px; top: 0; width: {line_w}px; height: {mid_y}px; background: {color};"
                    ),
                });
            }
            if seg.line_bottom {
                elems.push(GraphElement {
                    style: format!(
                        "position: absolute; left: {x}px; top: {mid_y}px; width: {line_w}px; height: {mid_y}px; background: {color};"
                    ),
                });
            }
        }

        // Node circle
        if seg.has_node {
            let nx = cx(lane) - node_r;
            let ny = mid_y - node_r;
            let d = node_r * 2.0;
            elems.push(GraphElement {
                style: format!(
                    "position: absolute; left: {nx}px; top: {ny}px; width: {d}px; height: {d}px; border-radius: 50%; background: {color};"
                ),
            });
        }
    }

    // Horizontal connectors
    for &(from_lane, to_lane, color_idx) in &row_data.connectors {
        let color = lane_color_hex(color_idx);
        let x1 = cx(from_lane);
        let x2 = cx(to_lane);
        let left = x1.min(x2);
        let width = (x2 - x1).abs();
        let top = mid_y - line_w / 2.0;
        elems.push(GraphElement {
            style: format!(
                "position: absolute; left: {left}px; top: {top}px; width: {width}px; height: {line_w}px; background: {color};"
            ),
        });
    }

    // Ensure minimum width for proper layout even with 0 lanes
    let _ = max_lanes;
    elems
}

fn load_single_repo(path: &str) -> Result<AppState, String> {
    let commits = git::load_repo(path)?;
    let gs = graph::build_graph_state(commits);
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    let mut app = AppState::new();
    app.repos.push(RepoView {
        path: path.to_string(),
        name,
        graph: gs,
    });
    Ok(app)
}

fn load_multi_repos(path: &str) -> Result<AppState, String> {
    let root = Path::new(path);
    let repo_paths = git::scan_for_repos(root);

    if repo_paths.is_empty() {
        return Err(format!("No git repositories found in {path}"));
    }

    if repo_paths.len() == 1 {
        let p = repo_paths[0].display().to_string();
        return load_single_repo(&p);
    }

    let mut app = AppState::new();
    for repo_path in &repo_paths {
        let p = repo_path.display().to_string();
        match git::load_repo(&p) {
            Ok(commits) => {
                let gs = graph::build_graph_state(commits);
                let name = repo_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.clone());
                app.repos.push(RepoView {
                    path: p,
                    name,
                    graph: gs,
                });
            }
            Err(_) => continue,
        }
    }

    if app.repos.is_empty() {
        return Err(format!("Failed to load any repos from {path}"));
    }

    Ok(app)
}

/// Build display rows from a repo's graph state.
fn build_commit_rows(gs: &state::GraphState) -> Vec<CommitRow> {
    gs.commits
        .iter()
        .enumerate()
        .map(|(i, commit)| {
            let graph_elements = build_graph_elements(&gs.row_graph[i], gs.max_lanes);
            CommitRow {
                oid: commit.oid.clone(),
                short_oid: commit.short_oid.clone(),
                author: commit.author.clone(),
                date: format_timestamp(commit.timestamp),
                message: commit.message.clone(),
                branch_labels: commit.branch_labels.clone(),
                is_head: commit.is_head,
                graph_elements,
                graph_width: (gs.max_lanes.max(1) as f32) * LANE_WIDTH,
                commit: commit.clone(),
            }
        })
        .collect()
}

fn main() {
    let repo_path = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());

    let theme = ThemeProviderProps {
        primary_color: Some("blue".into()),
        dark_mode: true,
        ..Default::default()
    };

    run_with_theme(
        "gitrinching",
        1200,
        800,
        move |__scope: &mut RenderScope| {
            let app_state = Arc::new(Mutex::new(AppState::new()));

            let repo_path_sig = Signal::new(repo_path.clone());
            let status_msg = Signal::new(String::new());
            let selected_commit = Signal::new(Option::<CommitNode>::None);
            let drawer_open = Signal::new(false);
            let commit_rows: Signal<Vec<CommitRow>> = Signal::new(Vec::new());

            let as_load = app_state.clone();
            let load_repo = Arc::new(Mutex::new(move |path: String| {
                let result = load_single_repo(&path).or_else(|_| load_multi_repos(&path));

                match result {
                    Ok(new_app) => {
                        let count = new_app.repos.len();
                        let names: Vec<String> =
                            new_app.repos.iter().map(|r| r.name.clone()).collect();

                        if let Some(repo) = new_app.repos.first() {
                            commit_rows.set(build_commit_rows(&repo.graph));
                        }

                        {
                            let mut app = as_load.lock().unwrap();
                            *app = new_app;
                        }

                        if count == 1 {
                            status_msg.set(format!("Loaded: {path}"));
                        } else {
                            status_msg
                                .set(format!("Loaded {count} repos: {}", names.join(", ")));
                        }
                    }
                    Err(e) => {
                        status_msg.set(format!("Error: {e}"));
                    }
                }
            }));

            // Initial load
            {
                let load = load_repo.clone();
                let path = repo_path_sig.get();
                if let Ok(load_fn) = load.lock() {
                    load_fn(path);
                }
            }

            let load_repo_btn = load_repo.clone();

            rsx! {
                div { style: "display: flex; flex-direction: column; height: 100vh; color: #ccc; font-family: monospace; background: #1e1e1e;",
                    // Toolbar
                    div { style: "display: flex; align-items: center; gap: 8px; padding: 8px 12px; background: #2d2d2d; border-bottom: 1px solid #444; flex-shrink: 0;",
                        span { style: "font-weight: bold; color: #61afef;", "gitrinching" }
                        TextInput {
                            placeholder: "Repository or folder path...",
                            value: repo_path_sig.get(),
                            oninput: move |v: String| { repo_path_sig.set(v); }
                        }
                        Button {
                            onclick: move || {
                                let path = repo_path_sig.get();
                                if let Ok(load_fn) = load_repo_btn.lock() {
                                    load_fn(path);
                                }
                            },
                            "Load"
                        }
                        Button {
                            onclick: {
                                let load_repo_browse = load_repo.clone();
                                move || {
                                    if let Some(path) = rinch::dialogs::pick_folder()
                                        .set_title("Select Repository")
                                        .pick()
                                    {
                                        let path_str = path.display().to_string();
                                        repo_path_sig.set(path_str.clone());
                                        if let Ok(load_fn) = load_repo_browse.lock() {
                                            load_fn(path_str);
                                        }
                                    }
                                }
                            },
                            "Browse"
                        }
                    }
                    // Commit list (scrollable)
                    div { style: "flex: 1; overflow-y: auto; min-height: 0;",
                        // Header row
                        div { style: "display: flex; align-items: center; padding: 4px 8px; background: #252525; border-bottom: 1px solid #333; font-size: 11px; color: #888; position: sticky; top: 0; z-index: 10;",
                            div { style: {move || {
                                let rows = commit_rows.get();
                                let gw = rows.first().map(|r| r.graph_width).unwrap_or(48.0);
                                format!("width: {}px; flex-shrink: 0;", gw)
                            }}, "Graph" }
                            div { style: "width: 80px; flex-shrink: 0; padding-left: 8px;", "Hash" }
                            div { style: "flex: 1; padding-left: 8px;", "Message" }
                            div { style: "width: 140px; flex-shrink: 0; padding-left: 8px;", "Author" }
                            div { style: "width: 120px; flex-shrink: 0; padding-left: 8px;", "Date" }
                        }
                        // Commit rows
                        for row in commit_rows.get() {
                            div {
                                key: row.oid.clone(),
                                style: {
                                    let oid = row.oid.clone();
                                    move || {
                                        let is_selected = selected_commit.get()
                                            .as_ref()
                                            .map(|c| c.oid == oid)
                                            .unwrap_or(false);
                                        let bg = if is_selected { "#2a2d3e" } else { "transparent" };
                                        format!(
                                            "display: flex; align-items: center; height: {}px; \
                                             border-bottom: 1px solid #2a2a2a; cursor: pointer; \
                                             background: {}; padding-right: 8px;",
                                            ROW_HEIGHT, bg
                                        )
                                    }
                                },
                                onclick: {
                                    let commit = row.commit.clone();
                                    move || {
                                        selected_commit.set(Some(commit.clone()));
                                        drawer_open.set(true);
                                    }
                                },
                                // Graph column — positioned divs for lines and nodes
                                div {
                                    style: {
                                        let gw = row.graph_width;
                                        format!("width: {}px; height: {}px; flex-shrink: 0; position: relative;", gw, ROW_HEIGHT)
                                    },
                                    for elem in row.graph_elements.clone() {
                                        div { style: elem.style.clone() }
                                    }
                                }
                                // Short OID
                                div { style: "width: 80px; flex-shrink: 0; padding-left: 8px; color: #61afef; font-size: 12px;",
                                    {row.short_oid.clone()}
                                }
                                // Message + branch labels
                                div { style: "flex: 1; padding-left: 8px; font-size: 12px; overflow: hidden; white-space: nowrap; display: flex; align-items: center; gap: 6px;",
                                    for label in row.branch_labels.clone() {
                                        span { style: "color: #1e1e1e; background: #ffd700; border-radius: 3px; padding: 1px 5px; font-size: 10px; font-weight: bold; flex-shrink: 0;",
                                            {label.clone()}
                                        }
                                    }
                                    if row.is_head {
                                        span { style: "color: #1e1e1e; background: #61afef; border-radius: 3px; padding: 1px 5px; font-size: 10px; font-weight: bold; flex-shrink: 0;",
                                            "HEAD"
                                        }
                                    }
                                    span { style: "overflow: hidden; white-space: nowrap;",
                                        {row.message.clone()}
                                    }
                                }
                                // Author
                                div { style: "width: 140px; flex-shrink: 0; padding-left: 8px; font-size: 12px; color: #98c379; overflow: hidden; white-space: nowrap;",
                                    {row.author.clone()}
                                }
                                // Date
                                div { style: "width: 120px; flex-shrink: 0; padding-left: 8px; font-size: 11px; color: #888;",
                                    {row.date.clone()}
                                }
                            }
                        }
                    }
                    // Status bar
                    div { style: "padding: 4px 12px; background: #2d2d2d; border-top: 1px solid #444; font-size: 11px; color: #888; flex-shrink: 0;",
                        {|| status_msg.get()}
                    }
                    // Commit detail side panel
                    div { style: {move || format!(
                            "position: fixed; top: 0; right: 0; bottom: 0; width: 380px; \
                             background: #1e1e1e; border-left: 1px solid #444; \
                             z-index: 200; overflow-y: auto; \
                             transition: transform 300ms ease; \
                             transform: translateX({});",
                            if drawer_open.get() { "0" } else { "100%" }
                        )},
                        div { style: "display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; border-bottom: 1px solid #444;",
                            span { style: "font-size: 16px; font-weight: 600; color: #ccc;", "Commit Details" }
                            Button {
                                onclick: move || drawer_open.set(false),
                                "X"
                            }
                        }
                        div { style: "padding: 16px; font-family: monospace; color: #ccc;",
                            div { style: "margin-bottom: 12px;",
                                div { style: "color: #888; font-size: 11px;", "OID" }
                                div { style: "color: #61afef; word-break: break-all;",
                                    {move || selected_commit.get().map(|c| c.oid.clone()).unwrap_or_default()}
                                }
                            }
                            div { style: "margin-bottom: 12px;",
                                div { style: "color: #888; font-size: 11px;", "Author" }
                                div {
                                    {move || selected_commit.get().map(|c| c.author.clone()).unwrap_or_default()}
                                }
                            }
                            div { style: "margin-bottom: 12px;",
                                div { style: "color: #888; font-size: 11px;", "Date" }
                                div {
                                    {move || selected_commit.get().map(|c| format_timestamp(c.timestamp)).unwrap_or_default()}
                                }
                            }
                            div { style: "margin-bottom: 12px;",
                                div { style: "color: #888; font-size: 11px;", "Message" }
                                div { style: "white-space: pre-wrap;",
                                    {move || selected_commit.get().map(|c| c.message.clone()).unwrap_or_default()}
                                }
                            }
                            div { style: "margin-bottom: 12px;",
                                div { style: "color: #888; font-size: 11px;", "Parents" }
                                div {
                                    {move || selected_commit.get().map(|c| {
                                        if c.parent_oids.is_empty() {
                                            "None (root commit)".to_string()
                                        } else {
                                            c.parent_oids.iter().map(|p| p[..7.min(p.len())].to_string()).collect::<Vec<_>>().join(", ")
                                        }
                                    }).unwrap_or_default()}
                                }
                            }
                            div { style: "margin-bottom: 12px;",
                                div { style: "color: #888; font-size: 11px;", "Branches" }
                                div { style: "color: #ffd700;",
                                    {move || selected_commit.get().map(|c| c.branch_labels.join(", ")).unwrap_or_default()}
                                }
                            }
                        }
                    }
                }
            }
        },
        theme,
    );
}
