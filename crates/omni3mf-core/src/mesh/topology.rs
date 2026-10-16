//! DirectedEdge and Half-Edge topology representation and adjacency queries.

use std::collections::HashMap;

/// A directed edge pointing from vertex `u` to vertex `v`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DirectedEdge {
    pub u: u32,
    pub v: u32,
}

impl DirectedEdge {
    #[inline(always)]
    pub const fn new(u: u32, v: u32) -> Self {
        Self { u, v }
    }

    /// Returns the opposite (twin) directed edge `v -> u`.
    #[inline(always)]
    pub const fn twin(self) -> Self {
        Self {
            u: self.v,
            v: self.u,
        }
    }
}

/// An undirected edge connecting vertices `u` and `v` with canonical ordering `u <= v`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UndirectedEdge {
    pub u: u32,
    pub v: u32,
}

impl UndirectedEdge {
    #[inline(always)]
    pub fn new(a: u32, b: u32) -> Self {
        if a <= b {
            Self { u: a, v: b }
        } else {
            Self { u: b, v: a }
        }
    }
}

/// Mesh topological adjacency graph built over face indices.
#[derive(Debug, Clone)]
pub struct EdgeTopology {
    /// Directed edge `(u, v)` -> `(face_index, local_edge_index: 0..3)`.
    /// Assumes orientable 2-manifold input; in case of non-manifold duplicate directed edges, the last face is retained.
    pub directed_edges: HashMap<DirectedEdge, (u32, u8)>,
    /// Undirected edge -> incident `Vec<(face_index, local_edge_index)>`.
    pub undirected_edges: HashMap<UndirectedEdge, Vec<(u32, u8)>>,
    /// Number of faces in the mesh.
    pub face_count: usize,
}

impl EdgeTopology {
    /// Builds topological connectivity maps from triangle face indices.
    pub fn build(faces: &[[u32; 3]]) -> Self {
        let mut directed = HashMap::with_capacity(faces.len() * 3);
        let mut undirected: HashMap<UndirectedEdge, Vec<(u32, u8)>> =
            HashMap::with_capacity(faces.len() * 3 / 2);

        for (f_idx, &[v0, v1, v2]) in faces.iter().enumerate() {
            let f = f_idx as u32;
            let edges = [
                (DirectedEdge::new(v0, v1), UndirectedEdge::new(v0, v1), 0u8),
                (DirectedEdge::new(v1, v2), UndirectedEdge::new(v1, v2), 1u8),
                (DirectedEdge::new(v2, v0), UndirectedEdge::new(v2, v0), 2u8),
            ];

            for (dir, undir, local) in edges {
                directed.insert(dir, (f, local));
                undirected.entry(undir).or_default().push((f, local));
            }
        }

        Self {
            directed_edges: directed,
            undirected_edges: undirected,
            face_count: faces.len(),
        }
    }

    /// Finds all boundary directed edges (directed edges whose twin does not exist in the mesh).
    pub fn find_boundary_directed_edges(&self) -> Vec<DirectedEdge> {
        let mut boundary = Vec::new();
        for &dir in self.directed_edges.keys() {
            let twin = dir.twin();
            if !self.directed_edges.contains_key(&twin) {
                boundary.push(dir);
            }
        }
        boundary
    }

    /// Returns the number of boundary directed edges.
    #[inline(always)]
    pub fn boundary_edge_count(&self) -> usize {
        self.find_boundary_directed_edges().len()
    }

    /// Returns non-manifold undirected edges shared by more than 2 faces.
    pub fn non_manifold_edges(&self) -> Vec<UndirectedEdge> {
        let mut non_manifold = Vec::new();
        for (&edge, faces) in &self.undirected_edges {
            if faces.len() > 2 {
                non_manifold.push(edge);
            }
        }
        non_manifold
    }

    /// Checks if the mesh is 2-manifold (every edge has at most 2 incident faces).
    pub fn is_2_manifold(&self) -> bool {
        self.undirected_edges.values().all(|faces| faces.len() <= 2)
    }

    /// Checks if the mesh is watertight (closed 2-manifold with zero boundary edges).
    pub fn is_watertight(&self) -> bool {
        self.is_2_manifold() && self.boundary_edge_count() == 0
    }

    /// Computes the Euler characteristic: Chi = V - E + F.
    pub fn euler_characteristic(&self, vertex_count: usize) -> i32 {
        let v = vertex_count as i32;
        let e = self.undirected_edges.len() as i32;
        let f = self.face_count as i32;
        v - e + f
    }
}
