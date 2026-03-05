use std::collections::HashMap;

use crate::state::{CommitNode, GraphEdge, GraphState};

/// Assign lanes (columns) to commits using a greedy algorithm.
/// Each active lane tracks which OID it's following. First parent continues
/// the same lane; merge parents get new lanes.
pub fn assign_lanes(commits: &mut Vec<CommitNode>) -> usize {
    // active_lanes[lane_index] = oid that this lane is currently tracking
    let mut active_lanes: Vec<Option<String>> = Vec::new();
    let mut max_lanes: usize = 0;

    // Map from oid to which lane is expecting it
    let mut reserved: HashMap<String, usize> = HashMap::new();

    for i in 0..commits.len() {
        commits[i].row = i;

        let oid = commits[i].oid.clone();

        // Check if a lane is already reserved for this commit
        let lane = if let Some(&l) = reserved.get(&oid) {
            // This lane was waiting for us
            active_lanes[l] = None;
            reserved.remove(&oid);
            l
        } else {
            // Find a free lane or allocate a new one
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

        // Reserve lanes for parents
        let parent_oids = commits[i].parent_oids.clone();
        for (pi, parent_oid) in parent_oids.iter().enumerate() {
            if reserved.contains_key(parent_oid) {
                // Already reserved by an earlier child
                continue;
            }
            if pi == 0 {
                // First parent: continue on the same lane
                active_lanes[lane] = Some(parent_oid.clone());
                reserved.insert(parent_oid.clone(), lane);
            } else {
                // Merge parent: allocate a new lane
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

/// Build a complete GraphState from raw commits.
pub fn build_graph_state(mut commits: Vec<CommitNode>) -> GraphState {
    let max_lanes = assign_lanes(&mut commits);

    let oid_to_index: HashMap<String, usize> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| (c.oid.clone(), i))
        .collect();

    let edges = build_edges(&commits, &oid_to_index);

    GraphState {
        commits,
        edges,
        oid_to_index,
        max_lanes,
        offset_x: 50.0,
        offset_y: 30.0,
        zoom: 1.0,
        width: 0,
        height: 0,
        selected_oid: None,
        dirty: true,
    }
}
