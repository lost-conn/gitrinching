/// A single commit in the graph.
#[derive(Clone, Debug, PartialEq)]
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

/// Full graph state.
#[derive(Clone, Debug)]
pub struct GraphState {
    pub commits: Vec<CommitNode>,
    pub max_lanes: usize,
    pub row_graph: Vec<RowGraphData>,
}

/// Per-row rendering data for the graph column.
#[derive(Clone, Debug)]
pub struct RowGraphData {
    pub lanes: Vec<LaneSegment>,
    /// Horizontal connectors: (from_lane, to_lane, color_index)
    pub connectors: Vec<(usize, usize, usize)>,
}

/// What to draw in a single lane cell for one row.
#[derive(Clone, Debug, Default)]
pub struct LaneSegment {
    pub has_node: bool,
    pub line_top: bool,
    pub line_bottom: bool,
    pub color_index: usize,
}

impl LaneSegment {
    pub fn is_active(&self) -> bool {
        self.has_node || self.line_top || self.line_bottom
    }
}

// Layout constants
pub const LANE_WIDTH: f32 = 24.0;
pub const ROW_HEIGHT: f32 = 32.0;

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
}

impl AppState {
    pub fn new() -> Self {
        Self { repos: Vec::new() }
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

pub fn lane_color_hex(index: usize) -> String {
    let (r, g, b) = LANE_COLORS[index % LANE_COLORS.len()];
    format!("#{r:02x}{g:02x}{b:02x}")
}
