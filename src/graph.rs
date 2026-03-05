use std::collections::HashMap;

use crate::state::{CommitNode, GraphEdge, GraphState, LaneSegment, RowGraphData};

/// Assign lanes (columns) to commits using a greedy algorithm.
/// Each active lane tracks which OID it's following. First parent continues
/// the same lane; merge parents get new lanes.
pub fn assign_lanes(commits: &mut Vec<CommitNode>) -> usize {
    let mut active_lanes: Vec<Option<String>> = Vec::new();
    let mut max_lanes: usize = 0;
    let mut reserved: HashMap<String, usize> = HashMap::new();

    for i in 0..commits.len() {
        commits[i].row = i;
        let oid = commits[i].oid.clone();

        let lane = if let Some(&l) = reserved.get(&oid) {
            active_lanes[l] = None;
            reserved.remove(&oid);
            l
        } else {
            let free = active_lanes.iter().position(|l| l.is_none());
            match free {
                Some(l) => l,
                None => {
                    active_lanes.push(None);
                    active_lanes.len() - 1
                }
            }
        };

        commits[i].lane = lane;
        max_lanes = max_lanes.max(lane + 1);

        let parent_oids = commits[i].parent_oids.clone();
        for (pi, parent_oid) in parent_oids.iter().enumerate() {
            if reserved.contains_key(parent_oid) {
                continue;
            }
            if pi == 0 {
                active_lanes[lane] = Some(parent_oid.clone());
                reserved.insert(parent_oid.clone(), lane);
            } else {
                let merge_lane = active_lanes.iter().position(|l| l.is_none());
                let merge_lane = match merge_lane {
                    Some(l) => l,
                    None => {
                        active_lanes.push(None);
                        active_lanes.len() - 1
                    }
                };
                active_lanes[merge_lane] = Some(parent_oid.clone());
                reserved.insert(parent_oid.clone(), merge_lane);
                max_lanes = max_lanes.max(merge_lane + 1);
            }
        }
    }

    max_lanes
}

/// Build edges from parent-child relationships.
pub fn build_edges(commits: &[CommitNode], oid_to_index: &HashMap<String, usize>) -> Vec<GraphEdge> {
    let mut edges = Vec::new();

    for commit in commits {
        for parent_oid in &commit.parent_oids {
            if let Some(&parent_idx) = oid_to_index.get(parent_oid) {
                let parent = &commits[parent_idx];
                edges.push(GraphEdge {
                    from_lane: commit.lane,
                    to_lane: parent.lane,
                    from_row: commit.row,
                    to_row: parent.row,
                    color_index: commit.lane,
                });
            }
        }
    }

    edges
}

/// Compute per-row graph rendering data from edges.
pub fn compute_row_graph_data(
    commits: &[CommitNode],
    edges: &[GraphEdge],
    max_lanes: usize,
) -> Vec<RowGraphData> {
    let num_rows = commits.len();
    let max_lanes = max_lanes.max(1);

    let mut data: Vec<RowGraphData> = (0..num_rows)
        .map(|i| {
            let mut lanes = vec![LaneSegment::default(); max_lanes];
            let lane = commits[i].lane;
            lanes[lane].has_node = true;
            lanes[lane].color_index = lane;
            RowGraphData {
                lanes,
                connectors: vec![],
            }
        })
        .collect();

    for edge in edges {
        let color = edge.color_index;

        if edge.from_lane == edge.to_lane {
            let lane = edge.from_lane;
            // Child node sends line downward
            data[edge.from_row].lanes[lane].line_bottom = true;
            // Parent node receives line from above
            data[edge.to_row].lanes[lane].line_top = true;
            // Intermediate rows: full pass-through
            for row in (edge.from_row + 1)..edge.to_row {
                let seg = &mut data[row].lanes[lane];
                seg.line_top = true;
                seg.line_bottom = true;
                seg.color_index = color;
            }
        } else {
            // Cross-lane: connector at child row, vertical on target lane
            data[edge.from_row]
                .connectors
                .push((edge.from_lane, edge.to_lane, color));

            // Start vertical line at child row on the target lane
            let seg = &mut data[edge.from_row].lanes[edge.to_lane];
            seg.line_bottom = true;
            seg.color_index = color;

            // Intermediate rows on target lane
            for row in (edge.from_row + 1)..edge.to_row {
                let seg = &mut data[row].lanes[edge.to_lane];
                seg.line_top = true;
                seg.line_bottom = true;
                seg.color_index = color;
            }

            // Parent node receives from above
            data[edge.to_row].lanes[edge.to_lane].line_top = true;
        }
    }

    data
}

/// Build a complete GraphState from raw commits.
pub fn build_graph_state(mut commits: Vec<CommitNode>) -> GraphState {
    let max_lanes = assign_lanes(&mut commits);

    let oid_to_index: HashMap<String, usize> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| (c.oid.clone(), i))
        .collect();

    let edges = build_edges(&commits, &oid_to_index);
    let row_graph = compute_row_graph_data(&commits, &edges, max_lanes);

    GraphState {
        commits,
        max_lanes,
        row_graph,
    }
}
