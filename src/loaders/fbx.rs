//! FBX (binary 7.x and ASCII) via the pure-Rust `fbxcel-dom` crate.
//! Geometry is read as stored (node transforms/animation are not applied).

use crate::model::{fan, Mesh, Model};
use fbxcel_dom::any::AnyDocument;
use fbxcel_dom::v7400::object::{geometry::TypedGeometryHandle, TypedObjectHandle};
use std::{fs::File, io::BufReader, path::Path};

pub fn load(path: &Path) -> Result<Model, String> {
    let reader = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let doc = match AnyDocument::from_seekable_reader(reader).map_err(|e| format!("FBX error: {e}"))? {
        AnyDocument::V7400(_ver, doc) => doc,
        _ => return Err("This FBX version is not supported (only 7.x is).".into()),
    };

    let mut out = Model::default();
    for (n, obj) in doc.objects().enumerate() {
        let TypedObjectHandle::Geometry(TypedGeometryHandle::Mesh(mesh)) = obj.get_typed() else {
            continue;
        };
        let pv = mesh.polygon_vertices().map_err(|e| format!("FBX mesh error: {e}"))?;
        let mut m = Mesh::new(
            mesh.models()
                .next()
                .and_then(|md| md.name().map(str::to_string))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("mesh_{n}")),
        );
        let pts = pv.raw_control_points().map_err(|e| format!("FBX points error: {e}"))?;
        m.positions = pts.map(|p| [p.x as f32, p.y as f32, p.z as f32]).collect();
        let mut poly: Vec<u32> = Vec::new();
        for &raw in pv.raw_polygon_vertices() {
            // A negative value marks the last vertex of a polygon (stored as !index).
            if raw < 0 {
                poly.push((!raw) as u32);
                fan(&poly, &mut m.indices);
                poly.clear();
            } else {
                poly.push(raw as u32);
            }
        }
        out.meshes.push(m);
    }
    Ok(out)
}
