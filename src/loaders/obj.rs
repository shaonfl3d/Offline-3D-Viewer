use crate::model::{Mesh, Model};
use std::path::Path;

pub fn load(path: &Path) -> Result<Model, String> {
    let opts = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ignore_lines: true,
        ignore_points: true,
    };
    let (models, mats) = tobj::load_obj(path, &opts).map_err(|e| format!("OBJ error: {e}"))?;
    let mats = mats.unwrap_or_default(); // a missing .mtl is not fatal
    let mut out = Model::default();
    for m in models {
        let src = &m.mesh;
        let mut mesh = Mesh::new(m.name);
        mesh.positions = src.positions.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
        if src.normals.len() == src.positions.len() {
            mesh.normals = src.normals.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
        }
        mesh.indices = src.indices.clone();
        if let Some(id) = src.material_id {
            if let Some(mat) = mats.get(id) {
                mesh.material = Some(mat.name.clone());
                if let Some(d) = mat.diffuse {
                    mesh.color = d;
                }
            }
        }
        out.meshes.push(mesh);
    }
    Ok(out)
}
