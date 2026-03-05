use std::collections::HashMap;

/// A single commit in the graph.
#[derive(Clone, Debug)]
pub struct CommitNode {
    pub oid: String,
    pub short_oid: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
    pub parent_oids: Vec<String>,
    pub branch_labels: Vec<String>,
    pub is_head: bool,
    /// Assigned lane (column) index.
    pub lane: usize,
    /// Row index (0 = newest).
    pub row: usize,
}

/// An edge connecting two commits in the graph.
#[derive(Clone, Debug)]
pub struct GraphEdge {
    pub from_lane: usize,
    pub to_lane: usize,
    pub from_row: usize,
    pub to_row: usize,
    pub color_index: usize,
}

/// Full graph state, shared with the render thread.
#[derive(Clone, Debug)]
pub struct GraphState {
    pub commits: Vec<CommitNode>,
    pub edges: Vec<GraphEdge>,
    pub oid_to_index: HashMap<String, usize>,
    #[allow(dead_code)]
    pub max_lanes: usize,
    // View transform
    pub offset_x: f32,
    pub offset_y: f32,
    pub zoom: f32,
    pub width: u32,
    pub height: u32,
    pub selected_oid: Option<String>,
    #[allow(dead_code)]
    pub dirty: bool,
}

// Layout constants
pub const NODE_RADIUS: f32 = 8.0;
pub const LANE_WIDTH: f32 = 30.0;
pub const ROW_HEIGHT: f32 = 40.0;

/// Convert a commit's lane/row to pixel position (before view transform).
pub fn commit_position(lane: usize, row: usize) -> (f32, f32) {
    let x = lane as f32 * LANE_WIDTH;
    let y = row as f32 * ROW_HEIGHT;
    (x, y)
}

/// Apply view transform (zoom + pan) to a world coordinate.
pub fn world_to_screen(wx: f32, wy: f32, state: &GraphState) -> (f32, f32) {
    let sx = wx * state.zoom + state.offset_x;
    let sy = wy * state.zoom + state.offset_y;
    (sx, sy)
}

/// Inverse: screen coordinate to world coordinate.
#[allow(dead_code)]
pub fn screen_to_world(sx: f32, sy: f32, state: &GraphState) -> (f32, f32) {
    let wx = (sx - state.offset_x) / state.zoom;
    let wy = (sy - state.offset_y) / state.zoom;
    (wx, wy)
}

/// A single repo view with its own graph state.
#[derive(Clone, Debug)]
pub struct RepoView {
    #[allow(dead_code)]
    pub path: String,
    pub name: String,
    pub graph: GraphState,
}

/// Application state holding multiple repo views.
#[derive(Clone, Debug)]
pub struct AppState {
    pub repos: Vec<RepoView>,
    pub dirty: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            repos: Vec::new(),
            dirty: true,
        }
    }
}

/// Lane colors (for branches).
pub const LANE_COLORS: &[(u8, u8, u8)] = &[
    (97, 175, 239),   // blue
    (152, 195, 121),  // green
    (229, 192, 123),  // yellow
    (224, 108, 117),  // red
    (198, 120, 221),  // purple
    (86, 182, 194),   // cyan
    (209, 154, 102),  // orange
    (190, 80, 70),    // dark red
    (126, 154, 206),  // light blue
    (106, 153, 85),   // dark green
];
