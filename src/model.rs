//! Core data structures shared by all loaders and the renderer.

use glam::{Mat4, Vec3};

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub color: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    /// Optional per-vertex normals (same length as `positions` when present).
    pub normals: Vec<[f32; 3]>,
    /// Triangle list (3 indices per triangle).
    pub indices: Vec<u32>,
    pub color: [f32; 3],
    pub material: Option<String>,
    /// Loaded but not shown at first (the source file marked it as hidden).
    pub hidden: bool,
}

impl Mesh {
    pub fn new(name: impl Into<String>) -> Self {
        Mesh {
            name: name.into(),
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            color: [0.72, 0.74, 0.78],
            material: None,
            hidden: false,
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn is_valid(&self) -> bool {
        !self.positions.is_empty() && self.indices.len() >= 3
    }

    /// Applies a transform to positions (and normals).
    pub fn transform(&mut self, m: Mat4) {
        for p in &mut self.positions {
            *p = m.transform_point3(Vec3::from(*p)).to_array();
        }
        if !self.normals.is_empty() {
            let nm = glam::Mat3::from_mat4(m).inverse().transpose();
            for n in &mut self.normals {
                let v = (nm * Vec3::from(*n)).normalize_or_zero();
                *n = v.to_array();
            }
        }
        // Mirroring transforms flip the winding order.
        if m.determinant() < 0.0 {
            for t in self.indices.chunks_exact_mut(3) {
                t.swap(1, 2);
            }
        }
    }

    /// Drops triangles that reference vertices outside the array.
    pub fn sanitize(&mut self) {
        let n = self.positions.len() as u32;
        let mut out = Vec::with_capacity(self.indices.len());
        for t in self.indices.chunks_exact(3) {
            if t[0] < n && t[1] < n && t[2] < n {
                out.extend_from_slice(t);
            }
        }
        self.indices = out;
        if self.normals.len() != self.positions.len() {
            self.normals.clear();
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub format: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bounds {
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub vertices: usize,
    pub triangles: usize,
    pub size: [f32; 3],
    pub volume: f64,
    pub surface: f64,
    pub bounds: Option<(Vec3, Vec3)>,
}

pub fn mesh_bounds(m: &Mesh) -> Option<Bounds> {
    if m.positions.is_empty() {
        return None;
    }
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for p in &m.positions {
        let v = Vec3::from(*p);
        min = min.min(v);
        max = max.max(v);
    }
    Some(Bounds { min, max })
}

pub fn mesh_stats(m: &Mesh) -> Stats {
    let mut s = Stats {
        vertices: m.positions.len(),
        triangles: m.triangle_count(),
        ..Default::default()
    };
    if let Some(b) = mesh_bounds(m) {
        s.size = b.size().to_array();
        s.bounds = Some((b.min, b.max));
    }
    for t in m.indices.chunks_exact(3) {
        let a = Vec3::from(m.positions[t[0] as usize]).as_dvec3();
        let b = Vec3::from(m.positions[t[1] as usize]).as_dvec3();
        let c = Vec3::from(m.positions[t[2] as usize]).as_dvec3();
        s.surface += (b - a).cross(c - a).length() * 0.5;
        s.volume += a.dot(b.cross(c)) / 6.0;
    }
    s
}

impl Model {
    pub fn bounds(&self) -> Option<Bounds> {
        let mut acc: Option<Bounds> = None;
        for m in &self.meshes {
            if let Some(b) = mesh_bounds(m) {
                acc = Some(match acc {
                    None => b,
                    Some(a) => Bounds { min: a.min.min(b.min), max: a.max.max(b.max) },
                });
            }
        }
        acc
    }

    pub fn stats(&self) -> Stats {
        let mut total = Stats::default();
        for m in &self.meshes {
            let s = mesh_stats(m);
            total.vertices += s.vertices;
            total.triangles += s.triangles;
            total.volume += s.volume;
            total.surface += s.surface;
        }
        if let Some(b) = self.bounds() {
            total.size = b.size().to_array();
            total.bounds = Some((b.min, b.max));
        }
        total
    }

    /// Final clean-up: sanitize meshes, drop empties, collect materials.
    pub fn finish(mut self) -> Model {
        for m in &mut self.meshes {
            m.sanitize();
        }
        self.meshes.retain(|m| m.is_valid());
        for m in &self.meshes {
            if let Some(name) = &m.material {
                if !self.materials.iter().any(|x| &x.name == name) {
                    self.materials.push(Material { name: name.clone(), color: m.color });
                }
            }
        }
        self
    }
}

/// Fan-triangulates a polygon given as a list of vertex indices.
pub fn fan(poly: &[u32], out: &mut Vec<u32>) {
    if poly.len() < 3 {
        return;
    }
    for i in 1..poly.len() - 1 {
        out.extend_from_slice(&[poly[0], poly[i], poly[i + 1]]);
    }
}
