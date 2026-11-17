use crate::model::{Mesh, Model};
use glam::Mat4;
use std::path::Path;

pub fn load(path: &Path) -> Result<Model, String> {
    let gltf = gltf::Gltf::open(path).map_err(|e| format!("glTF error: {e}"))?;
    let buffers = gltf::import_buffers(&gltf.document, path.parent(), gltf.blob.clone())
        .map_err(|e| format!("glTF buffers error: {e}"))?;
    let mut out = Model::default();

    let scene = gltf
        .document
        .default_scene()
        .or_else(|| gltf.document.scenes().next())
        .ok_or("glTF has no scene")?;

    fn visit(node: gltf::Node, parent: Mat4, buffers: &[gltf::buffer::Data], out: &mut Model) {
        let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        if let Some(mesh) = node.mesh() {
            for (pi, prim) in mesh.primitives().enumerate() {
                if prim.mode() != gltf::mesh::Mode::Triangles {
                    continue;
                }
                let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
                let Some(pos) = reader.read_positions() else { continue };
                let mut m = Mesh::new(format!(
                    "{}{}",
                    node.name().or(mesh.name()).unwrap_or("mesh"),
                    if mesh.primitives().len() > 1 { format!("_{pi}") } else { String::new() }
                ));
                m.positions = pos.collect();
                if let Some(n) = reader.read_normals() {
                    m.normals = n.collect();
                }
                m.indices = match reader.read_indices() {
                    Some(i) => i.into_u32().collect(),
                    None => (0..m.positions.len() as u32).collect(),
                };
                let mat = prim.material();
                let c = mat.pbr_metallic_roughness().base_color_factor();
                m.color = [c[0], c[1], c[2]];
                m.material = Some(mat.name().unwrap_or("material").to_string());
                m.transform(world);
                out.meshes.push(m);
            }
        }
        for child in node.children() {
            visit(child, world, buffers, out);
        }
    }

    for node in scene.nodes() {
        visit(node, Mat4::IDENTITY, &buffers, &mut out);
    }
    Ok(out)
}
