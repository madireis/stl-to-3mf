//! Topological face adjacency graph and Disjoint Set Union (DSU) component analysis.

use std::collections::HashMap;

use crate::mesh::UndirectedEdge;

/// Disjoint Set Union (DSU) / Union-Find data structure with path compression and union-by-rank.
#[derive(Debug, Clone)]
pub struct DisjointSetUnion {
    parent: Vec<usize>,
    rank: Vec<usize>,
    set_count: usize,
}

impl DisjointSetUnion {
    /// Creates a new DSU structure with `size` elements, each in its own set.
    pub fn new(size: usize) -> Self {
        let parent = (0..size).collect();
        let rank = vec![0; size];
        Self {
            parent,
            rank,
            set_count: size,
        }
    }

    /// Finds the representative root of the set containing `x` with two-pass path compression.
    pub fn find(&mut self, mut x: usize) -> usize {
        let mut root = x;
        while root != self.parent[root] {
            root = self.parent[root];
        }
        // Path compression: point all traversed nodes directly to root
        while x != root {
            let next = self.parent[x];
            self.parent[x] = root;
            x = next;
        }
        root
    }

    /// Unites the sets containing elements `x` and `y` using union-by-rank.
    /// Returns `true` if `x` and `y` were in different sets, or `false` if already connected.
    pub fn union(&mut self, x: usize, y: usize) -> bool {
        let root_x = self.find(x);
        let root_y = self.find(y);

        if root_x == root_y {
            return false;
        }

        match self.rank[root_x].cmp(&self.rank[root_y]) {
            std::cmp::Ordering::Less => {
                self.parent[root_x] = root_y;
            }
            std::cmp::Ordering::Greater => {
                self.parent[root_y] = root_x;
            }
            std::cmp::Ordering::Equal => {
                self.parent[root_y] = root_x;
                self.rank[root_x] += 1;
            }
        }

        self.set_count -= 1;
        true
    }

    /// Checks if elements `x` and `y` belong to the same connected component.
    pub fn connected(&mut self, x: usize, y: usize) -> bool {
        self.find(x) == self.find(y)
    }

    /// Returns the number of disjoint sets.
    #[inline(always)]
    pub fn set_count(&self) -> usize {
        self.set_count
    }

    /// Returns the total number of elements managed by the DSU.
    #[inline(always)]
    pub fn element_count(&self) -> usize {
        self.parent.len()
    }
}

/// Dual graph of triangular faces linked by shared undirected edges.
#[derive(Debug, Clone)]
pub struct FaceAdjacencyGraph {
    /// Total number of faces in the mesh.
    face_count: usize,
    /// Mapping from each undirected edge to all incident face indices.
    edge_to_faces: HashMap<UndirectedEdge, Vec<usize>>,
}

impl FaceAdjacencyGraph {
    /// Builds the face adjacency graph from triangular face indices.
    pub fn build(faces: &[[u32; 3]]) -> Self {
        let mut edge_to_faces: HashMap<UndirectedEdge, Vec<usize>> =
            HashMap::with_capacity(faces.len() * 3 / 2);

        for (f_idx, &[v0, v1, v2]) in faces.iter().enumerate() {
            edge_to_faces
                .entry(UndirectedEdge::new(v0, v1))
                .or_default()
                .push(f_idx);
            edge_to_faces
                .entry(UndirectedEdge::new(v1, v2))
                .or_default()
                .push(f_idx);
            edge_to_faces
                .entry(UndirectedEdge::new(v2, v0))
                .or_default()
                .push(f_idx);
        }

        Self {
            face_count: faces.len(),
            edge_to_faces,
        }
    }

    /// Returns the total number of faces.
    #[inline(always)]
    pub fn face_count(&self) -> usize {
        self.face_count
    }

    /// Extracts connected components of faces using Disjoint Set Union (DSU).
    ///
    /// Each returned `Vec<usize>` contains the face indices belonging to one connected shell.
    /// Components are returned in deterministic order, sorted by the minimum face index in each component.
    pub fn extract_connected_components(&self) -> Vec<Vec<usize>> {
        if self.face_count == 0 {
            return Vec::new();
        }

        let mut dsu = DisjointSetUnion::new(self.face_count);

        for incident_faces in self.edge_to_faces.values() {
            if incident_faces.len() > 1 {
                let first = incident_faces[0];
                for &other in &incident_faces[1..] {
                    dsu.union(first, other);
                }
            }
        }

        // Group faces by their representative root in DSU
        let mut component_map: HashMap<usize, Vec<usize>> = HashMap::new();
        for f_idx in 0..self.face_count {
            let root = dsu.find(f_idx);
            component_map.entry(root).or_default().push(f_idx);
        }

        // Sort components deterministically by minimum face index
        let mut components: Vec<Vec<usize>> = component_map.into_values().collect();
        components.sort_by_key(|comp| comp[0]);

        components
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsu_basic_ops() {
        let mut dsu = DisjointSetUnion::new(5);
        assert_eq!(dsu.set_count(), 5);
        assert_eq!(dsu.element_count(), 5);

        assert!(dsu.union(0, 1));
        assert_eq!(dsu.set_count(), 4);
        assert!(dsu.connected(0, 1));
        assert!(!dsu.connected(0, 2));

        assert!(!dsu.union(0, 1)); // redundant union
        assert_eq!(dsu.set_count(), 4);

        dsu.union(2, 3);
        dsu.union(1, 3);
        assert_eq!(dsu.set_count(), 2);
        assert!(dsu.connected(0, 2));
        assert!(dsu.connected(1, 3));
        assert!(!dsu.connected(0, 4));
    }

    #[test]
    fn test_face_adjacency_connected_and_disjoint() {
        // Face 0: (0, 1, 2)
        // Face 1: (1, 3, 2) - shares edge (1, 2) with Face 0
        // Face 2: (10, 11, 12) - completely disjoint
        let faces = vec![[0, 1, 2], [1, 3, 2], [10, 11, 12]];

        let graph = FaceAdjacencyGraph::build(&faces);
        assert_eq!(graph.face_count(), 3);

        let components = graph.extract_connected_components();
        assert_eq!(components.len(), 2);

        // Component 0 contains face 0 and face 1
        assert_eq!(components[0], vec![0, 1]);
        // Component 1 contains face 2
        assert_eq!(components[1], vec![2]);
    }

    #[test]
    fn test_face_adjacency_empty() {
        let graph = FaceAdjacencyGraph::build(&[]);
        assert_eq!(graph.face_count(), 0);
        let components = graph.extract_connected_components();
        assert!(components.is_empty());
    }
}
