use crate::model::{Mesh, Model};
use std::{fs, io::Read, path::Path};

pub fn load(path: &Path) -> Result<Model, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    // AMF may be a zip archive containing a single .amf XML file.
    let xml = if data.starts_with(b"PK") {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| e.to_string())?;
        let mut s = String::new();
        zip.by_index(0)
            .map_err(|e| e.to_string())?
            .read_to_string(&mut s)
            .map_err(|e| e.to_string())?;
        s
    } else {
        String::from_utf8_lossy(&data).into_owned()
    };
    let doc = roxmltree::Document::parse(&xml).map_err(|e| format!("AMF XML error: {e}"))?;
    let mut out = Model::default();
    for (oi, o) in doc.descendants().filter(|n| n.has_tag_name("object")).enumerate() {
        let Some(mesh_node) = o.children().find(|n| n.has_tag_name("mesh")) else { continue };
        let Some(verts) = mesh_node.children().find(|n| n.has_tag_name("vertices")) else { continue };
        let mut pos = Vec::new();
        for v in verts.children().filter(|n| n.has_tag_name("vertex")) {
            let Some(c) = v.children().find(|n| n.has_tag_name("coordinates")) else { continue };
            let g = |k: &str| {
                c.children()
                    .find(|n| n.has_tag_name(k))
                    .and_then(|n| n.text())
                    .and_then(|t| t.trim().parse::<f32>().ok())
                    .unwrap_or(0.0)
            };
            pos.push([g("x"), g("y"), g("z")]);
        }
        for (vi, vol) in mesh_node.children().filter(|n| n.has_tag_name("volume")).enumerate() {
            let mut mesh = Mesh::new(format!("object_{oi}_volume_{vi}"));
            mesh.positions = pos.clone();
            for t in vol.children().filter(|n| n.has_tag_name("triangle")) {
                for k in ["v1", "v2", "v3"] {
                    let i = t
                        .children()
                        .find(|n| n.has_tag_name(k))
                        .and_then(|n| n.text())
                        .and_then(|s| s.trim().parse::<u32>().ok())
                        .unwrap_or(0);
                    mesh.indices.push(i);
                }
            }
            out.meshes.push(mesh);
        }
    }
    Ok(out)
}
