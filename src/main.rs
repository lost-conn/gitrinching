mod git;
mod graph;
mod render;
mod state;

use std::path::Path;
use std::sync::{Arc, Mutex};

use rinch::prelude::*;
use rinch::render_surface::{SurfaceEvent, SurfaceMouseButton, create_render_surface};

use state::{AppState, CommitNode, RepoView};

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

/// Load a single repo into an AppState.
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

/// Scan a directory for repos and load them all.
fn load_multi_repos(path: &str) -> Result<AppState, String> {
    let root = Path::new(path);
    let repo_paths = git::scan_for_repos(root);

    if repo_paths.is_empty() {
        return Err(format!("No git repositories found in {path}"));
    }

    // If only one repo found (the path itself), load as single
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
            // Shared app state (multi-repo)
            let app_state = Arc::new(Mutex::new(AppState::new()));

            // Signals
            let repo_path_sig = Signal::new(repo_path.clone());
            let status_msg = Signal::new(String::new());
            let zoom_level = Signal::new(1.0f32);
            let drawer_open = Signal::new(false);
            let selected_commit = Signal::new(Option::<CommitNode>::None);

            // Create render surface
            let surface = create_render_surface();
            let writer = surface.writer();

            // Bootstrap frame
            {
                let w = 4u32;
                let h = 4u32;
                let mut pixels = vec![0u8; (w * h * 4) as usize];
                for i in 0..(w * h) as usize {
                    pixels[i * 4] = 30;
                    pixels[i * 4 + 1] = 30;
                    pixels[i * 4 + 2] = 30;
                    pixels[i * 4 + 3] = 255;
                }
                writer.submit_frame(&pixels, w, h);
            }

            // Track canvas size in app state for render callback
            let canvas_w = Arc::new(Mutex::new(0u32));
            let canvas_h = Arc::new(Mutex::new(0u32));

            // Render callback
            {
                let as_cb = app_state.clone();
                let cw = canvas_w.clone();
                let ch = canvas_h.clone();
                surface.set_render_callback(move |writer, w, h| {
                    let mut app = as_cb.lock().unwrap();
                    if w == 0 || h == 0 {
                        return;
                    }

                    // Track size
                    *cw.lock().unwrap() = w;
                    *ch.lock().unwrap() = h;

                    // Update size for single-repo case
                    let size_changed = if app.repos.len() == 1 {
                        let gs = &mut app.repos[0].graph;
                        if gs.width != w || gs.height != h {
                            gs.width = w;
                            gs.height = h;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if size_changed {
                        app.dirty = true;
                    }

                    if app.dirty {
                        app.dirty = false;
                        let pixels = if app.repos.len() == 1 {
                            render::render_graph(&app.repos[0].graph)
                        } else {
                            render::render_multi_graph(&app, w, h)
                        };
                        if !pixels.is_empty() {
                            writer.submit_frame(&pixels, w, h);
                        }
                    }
                });
            }

            // Load function: tries single repo first, then scans for multi
            let as_load = app_state.clone();
            let cw_load = canvas_w.clone();
            let ch_load = canvas_h.clone();
            let load_repo = Arc::new(Mutex::new(move |path: String| {
                // Try single repo first
                let result = load_single_repo(&path).or_else(|_| load_multi_repos(&path));

                match result {
                    Ok(mut new_app) => {
                        let w = *cw_load.lock().unwrap();
                        let h = *ch_load.lock().unwrap();

                        // Set canvas size for single-repo
                        if new_app.repos.len() == 1 {
                            new_app.repos[0].graph.width = w;
                            new_app.repos[0].graph.height = h;
                        }

                        let count = new_app.repos.len();
                        let names: Vec<String> = new_app.repos.iter().map(|r| r.name.clone()).collect();

                        {
                            let mut app = as_load.lock().unwrap();
                            *app = new_app;
                        }

                        if count == 1 {
                            status_msg.set(format!("Loaded: {path}"));
                        } else {
                            status_msg.set(format!("Loaded {count} repos: {}", names.join(", ")));
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

            // Event handler
            let as_event = app_state.clone();
            let cw_event = canvas_w.clone();
            let ch_event = canvas_h.clone();

            let dragging = Arc::new(Mutex::new(false));
            let last_mouse = Arc::new(Mutex::new((0.0f32, 0.0f32)));
            let drag_repo_idx = Arc::new(Mutex::new(0usize));

            let dragging_eh = dragging.clone();
            let last_mouse_eh = last_mouse.clone();
            let drag_repo_eh = drag_repo_idx.clone();

            surface.set_event_handler(move |event| {
                match event {
                    SurfaceEvent::MouseDown {
                        x,
                        y,
                        button: SurfaceMouseButton::Left,
                    } => {
                        let w = *cw_event.lock().unwrap();
                        let h = *ch_event.lock().unwrap();

                        let hit = {
                            let app = as_event.lock().unwrap();
                            if app.repos.len() == 1 {
                                render::hit_test(&app.repos[0].graph, x, y).map(|oid| (0, oid))
                            } else {
                                render::hit_test_multi(&app, x, y, w, h)
                            }
                        };

                        if let Some((repo_idx, oid)) = hit {
                            let mut app = as_event.lock().unwrap();
                            app.repos[repo_idx].graph.selected_oid = Some(oid.clone());
                            app.dirty = true;
                            if let Some(idx) = app.repos[repo_idx].graph.oid_to_index.get(&oid) {
                                let commit = app.repos[repo_idx].graph.commits[*idx].clone();
                                selected_commit.set(Some(commit));
                                drawer_open.set(true);
                            }
                        } else {
                            *dragging_eh.lock().unwrap() = true;
                            *last_mouse_eh.lock().unwrap() = (x, y);
                            // Determine which repo cell we're dragging in
                            let app = as_event.lock().unwrap();
                            let n = app.repos.len();
                            let idx = render::cell_index_at(x, y, n, w, h).unwrap_or(0);
                            *drag_repo_eh.lock().unwrap() = idx;
                        }
                    }
                    SurfaceEvent::MouseMove { x, y } => {
                        let is_dragging = *dragging_eh.lock().unwrap();
                        if is_dragging {
                            let (lx, ly) = *last_mouse_eh.lock().unwrap();
                            let dx = x - lx;
                            let dy = y - ly;
                            let repo_idx = *drag_repo_eh.lock().unwrap();
                            {
                                let mut app = as_event.lock().unwrap();
                                if repo_idx < app.repos.len() {
                                    app.repos[repo_idx].graph.offset_x += dx;
                                    app.repos[repo_idx].graph.offset_y += dy;
                                    app.dirty = true;
                                }
                            }
                            *last_mouse_eh.lock().unwrap() = (x, y);
                        }
                    }
                    SurfaceEvent::MouseUp {
                        button: SurfaceMouseButton::Left,
                        ..
                    } => {
                        *dragging_eh.lock().unwrap() = false;
                    }
                    SurfaceEvent::MouseWheel { x, y, delta_y, .. } => {
                        let zoom_factor = if delta_y > 0.0 { 1.1 } else { 0.9 };
                        let w = *cw_event.lock().unwrap();
                        let h = *ch_event.lock().unwrap();
                        {
                            let mut app = as_event.lock().unwrap();
                            let n = app.repos.len();
                            let repo_idx = render::cell_index_at(x, y, n, w, h).unwrap_or(0);
                            if repo_idx < app.repos.len() {
                                let gs = &mut app.repos[repo_idx].graph;
                                let old_zoom = gs.zoom;
                                gs.zoom = (gs.zoom * zoom_factor).clamp(0.1, 5.0);
                                let new_zoom = gs.zoom;
                                // For multi-repo, adjust x/y relative to cell origin
                                // For single repo, cell origin is (0,0) so this is identity
                                gs.offset_x = x - (x - gs.offset_x) * new_zoom / old_zoom;
                                gs.offset_y = y - (y - gs.offset_y) * new_zoom / old_zoom;
                                app.dirty = true;
                                zoom_level.set(new_zoom);
                            }
                        }
                    }
                    _ => {}
                }
            });

            // Build UI
            let load_repo_btn = load_repo.clone();

            rsx! {
                div { style: "display: flex; flex-direction: column; height: 100vh; color: #ccc; font-family: monospace;",
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
                        span { style: "margin-left: auto; font-size: 12px; color: #888;",
                            {|| format!("Zoom: {:.0}%", zoom_level.get() * 100.0)}
                        }
                    }
                    // Canvas
                    div { style: "flex: 1; min-width: 0; position: relative;",
                        div { style: "position: absolute; top: 0; left: 0; right: 0; bottom: 0;",
                            RenderSurface { surface: Some(surface) }
                        }
                    }
                    // Status bar
                    div { style: "padding: 4px 12px; background: #2d2d2d; border-top: 1px solid #444; font-size: 11px; color: #888; flex-shrink: 0;",
                        {|| status_msg.get()}
                    }
                    // Commit detail side panel (no Drawer — custom panel to avoid blocking events)
                    div { style: {move || format!(
                            "position: fixed; top: 0; right: 0; bottom: 0; width: 380px; \
                             background: #1e1e1e; border-left: 1px solid #444; \
                             z-index: 200; overflow-y: auto; \
                             transition: transform 300ms ease; \
                             transform: translateX({});",
                            if drawer_open.get() { "0" } else { "100%" }
                        )},
                        // Header
                        div { style: "display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; border-bottom: 1px solid #444;",
                            span { style: "font-size: 16px; font-weight: 600; color: #ccc;", "Commit Details" }
                            Button {
                                onclick: move || drawer_open.set(false),
                                "X"
                            }
                        }
                        // Body
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
