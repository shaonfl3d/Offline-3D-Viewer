use crate::model::{Mesh, Model};
use glam::{Mat4, Quat, Vec3};
use serde_json::Value;
use std::{fs, path::Path};

fn f(v: &Value, k: &str, d: f32) -> f32 {
    v.get(k).and_then(|x| x.as_f64()).map(|x| x as f32).unwrap_or(d)
}

pub fn load(path: &Path) -> Result<Model, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let json: Value = serde_json::from_str(&text).map_err(|e| format!("BIM JSON error: {e}"))?;
    let meshes = json.get("meshes").and_then(|m| m.as_array()).ok_or("BIM: missing 'meshes'")?;
    let mut base: std::collections::HashMap<i64, Mesh> = Default::default();
    for m in meshes {
        let id = m.get("mesh_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let coords: Vec<f32> = m
            .get("coordinates")
            .and_then(|c| c.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect())
            .unwrap_or_default();
        let mut mesh = Mesh::new(format!("mesh_{id}"));
        mesh.positions = coords.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        mesh.indices = m
            .get("indices")
            .and_then(|c| c.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect())
            .unwrap_or_default();
        base.insert(id, mesh);
    }

    let mut out = Model::default();
    if let Some(elements) = json.get("elements").and_then(|e| e.as_array()) {
        for (i, el) in elements.iter().enumerate() {
            let id = el.get("mesh_id").and_then(|v| v.as_i64()).unwrap_or(-1);
            let Some(src) = base.get(&id) else { continue };
            let mut m = src.clone();
            let t = el.get("vector").map(|v| Vec3::new(f(v, "x", 0.0), f(v, "y", 0.0), f(v, "z", 0.0))).unwrap_or(Vec3::ZERO);
            let q = el
                .get("rotation")
                .map(|r| Quat::from_xyzw(f(r, "qx", 0.0), f(r, "qy", 0.0), f(r, "qz", 0.0), f(r, "qw", 1.0)))
                .unwrap_or(Quat::IDENTITY);
            m.transform(Mat4::from_rotation_translation(q.normalize(), t));
            let ty = el.get("type").and_then(|v| v.as_str()).unwrap_or("element");
            m.name = format!("{ty}_{i}");
            if let Some(c) = el.get("color") {
                m.color = [f(c, "r", 180.0) / 255.0, f(c, "g", 180.0) / 255.0, f(c, "b", 180.0) / 255.0];
            }
            out.meshes.push(m);
        }
    }
    if out.meshes.is_empty() {
        out.meshes.extend(base.into_values());
    }
    Ok(out)
}
