//! Boundary edge cycle extraction and loop detection.

use std::collections::HashMap;

use crate::mesh::DirectedEdge;

/// Extracts closed boundary loops from a collection of boundary directed edges.
///
/// For a boundary edge `u -> v` on an existing face, the open hole boundary step
/// that needs to be filled is `v -> u`. This function chains these opposite steps
/// into closed simple cycles `[v0, v1, ..., vk]`.
pub fn extract_boundary_loops(boundary_edges: &[DirectedEdge]) -> Vec<Vec<u32>> {
    if boundary_edges.is_empty() {
        return Vec::new();
    }

    // Map outgoing boundary steps: v -> Vec<u> (hole edge direction is edge.v -> edge.u)
    let mut outgoing: HashMap<u32, Vec<u32>> = HashMap::new();
    for edge in boundary_edges {
        outgoing.entry(edge.v).or_default().push(edge.u);
    }

    let mut loops = Vec::new();

    // Iterate through all vertices that have outgoing boundary edges
    while let Some(&start_node) = outgoing.keys().next() {
        let mut path = Vec::new();
        let mut node_to_pos: HashMap<u32, usize> = HashMap::new();

        let mut curr = start_node;

        while let Some(targets) = outgoing.get_mut(&curr) {
            if targets.is_empty() {
                break;
            }
            let next_node = targets.pop().unwrap();

            // If this node has no more outgoing edges, remove its entry
            if targets.is_empty() {
                outgoing.remove(&curr);
            }

            path.push(curr);
            node_to_pos.insert(curr, path.len() - 1);

            // Check if next_node closes a cycle within the current path
            if let Some(&cycle_start_idx) = node_to_pos.get(&next_node) {
                // Extract simple cycle: path[cycle_start_idx..]
                let cycle: Vec<u32> = path[cycle_start_idx..].to_vec();
                if cycle.len() >= 3 {
                    loops.push(cycle);
                }

                // Truncate path to cycle_start_idx and cleanup node_to_pos
                for &v in &path[cycle_start_idx..] {
                    node_to_pos.remove(&v);
                }
                path.truncate(cycle_start_idx);

                // If path is empty, pick a new start node
                if path.is_empty() {
                    break;
                }
                curr = *path.last().unwrap();
            } else {
                curr = next_node;
            }
        }

        // Clean up empty entries
        outgoing.retain(|_, targets| !targets.is_empty());
    }

    loops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_boundary_loop() {
        // A single triangular hole missing facet (1, 2, 3)
        // Existing boundary edges around the hole: (3, 2), (2, 1), (1, 3)
        // Open hole steps to fill: 2 -> 3, 1 -> 2, 3 -> 1
        let edges = vec![
            DirectedEdge::new(3, 2),
            DirectedEdge::new(2, 1),
            DirectedEdge::new(1, 3),
        ];

        let loops = extract_boundary_loops(&edges);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].len(), 3);
    }
}
