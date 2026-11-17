use crate::model::{Mesh, Model};
use glam::{Mat4, Vec3};
use std::{collections::HashMap, fs::File, io::Read, path::Path};

pub(crate) fn parse_transform(s: Option<&str>) -> Mat4 {
    // 3MF stores a row-vector 4x3 matrix: m00 m01 m02 m10 m11 m12 m20 m21 m22 m30 m31 m32
    let Some(s) = s else { return Mat4::IDENTITY };
    let v: Vec<f32> = s.split_whitespace().filter_map(|t| t.parse().ok()).collect();
    if v.len() != 12 {
        return Mat4::IDENTITY;
    }
    Mat4::from_cols_array(&[
        v[0], v[1], v[2], 0.0, v[3], v[4], v[5], 0.0, v[6], v[7], v[8], 0.0, v[9], v[10], v[11], 1.0,
    ])
}

struct Obj {
    name: String,
    mesh: Option<(Vec<[f32; 3]>, Vec<u32>)>,
    components: Vec<(String, Mat4)>,
}

pub fn load(path: &Path) -> Result<Model, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("3MF is not a valid zip: {e}"))?;
    let mut model_name = None;
    for i in 0..zip.len() {
        let name = zip.by_index(i).map_err(|e| e.to_string())?.name().to_string();
        if name.to_ascii_lowercase().ends_with(".model") {
            model_name = Some(name);
            break;
        }
    }
    let model_name = model_name.ok_or("3MF has no .model file")?;
    let mut xml = String::new();
    zip.by_name(&model_name)
        .map_err(|e| e.to_string())?
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;
    let doc = roxmltree::Document::parse(&xml).map_err(|e| format!("3MF XML error: {e}"))?;

    let mut objects: HashMap<String, Obj> = HashMap::new();
    for o in doc.descendants().filter(|n| n.has_tag_name("object")) {
        let id = o.attribute("id").unwrap_or("").to_string();
        let name = o.attribute("name").map(str::to_string).unwrap_or_else(|| format!("object_{id}"));
        let mut obj = Obj { name, mesh: None, components: vec![] };
        if let Some(mesh) = o.children().find(|n| n.has_tag_name("mesh")) {
            let mut pos = Vec::new();
            let mut idx = Vec::new();
            for v in mesh.descendants().filter(|n| n.has_tag_name("vertex")) {
                let g = |k: &str| v.attribute(k).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                pos.push([g("x"), g("y"), g("z")]);
            }
            for t in mesh.descendants().filter(|n| n.has_tag_name("triangle")) {
                for k in ["v1", "v2", "v3"] {
                    idx.push(t.attribute(k).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0));
                }
            }
            obj.mesh = Some((pos, idx));
        }
        for c in o.descendants().filter(|n| n.has_tag_name("component")) {
            obj.components.push((
                c.attribute("objectid").unwrap_or("").to_string(),
                parse_transform(c.attribute("transform")),
            ));
        }
        objects.insert(id, obj);
    }

    fn emit(id: &str, m: Mat4, objs: &HashMap<String, Obj>, out: &mut Model, depth: u32) {
        if depth > 16 {
            return;
        }
        let Some(o) = objs.get(id) else { return };
        if let Some((p, i)) = &o.mesh {
            let mut mesh = Mesh::new(o.name.clone());
            mesh.positions = p.clone();
            mesh.indices = i.clone();
            mesh.transform(m);
            out.meshes.push(mesh);
        }
        for (cid, cm) in &o.components {
            emit(cid, m * *cm, objs, out, depth + 1);
        }
    }

    let mut out = Model::default();
    let items: Vec<_> = doc.descendants().filter(|n| n.has_tag_name("item")).collect();
    if items.is_empty() {
        let mut ids: Vec<_> = objects.keys().cloned().collect();
        ids.sort();
        for id in ids {
            emit(&id, Mat4::IDENTITY, &objects, &mut out, 0);
        }
    } else {
        for it in items {
            let m = parse_transform(it.attribute("transform"));
            emit(it.attribute("objectid").unwrap_or(""), m, &objects, &mut out, 0);
        }
    }
    let _ = Vec3::ZERO;
    Ok(out)
}
