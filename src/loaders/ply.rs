use crate::model::{fan, Mesh, Model};
use ply_rs::parser::Parser;
use ply_rs::ply::{DefaultElement, Property};
use std::{fs::File, io::BufReader, path::Path};

fn num(p: &Property) -> Option<f64> {
    Some(match p {
        Property::Char(v) => *v as f64,
        Property::UChar(v) => *v as f64,
        Property::Short(v) => *v as f64,
        Property::UShort(v) => *v as f64,
        Property::Int(v) => *v as f64,
        Property::UInt(v) => *v as f64,
        Property::Float(v) => *v as f64,
        Property::Double(v) => *v,
        _ => return None,
    })
}

fn list(p: &Property) -> Option<Vec<u32>> {
    Some(match p {
        Property::ListChar(v) => v.iter().map(|&x| x as u32).collect(),
        Property::ListUChar(v) => v.iter().map(|&x| x as u32).collect(),
        Property::ListShort(v) => v.iter().map(|&x| x as u32).collect(),
        Property::ListUShort(v) => v.iter().map(|&x| x as u32).collect(),
        Property::ListInt(v) => v.iter().map(|&x| x as u32).collect(),
        Property::ListUInt(v) => v.clone(),
        _ => return None,
    })
}

fn get(e: &DefaultElement, k: &str) -> f32 {
    e.get(k).and_then(num).unwrap_or(0.0) as f32
}

pub fn load(path: &Path) -> Result<Model, String> {
    let mut reader = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let ply = Parser::<DefaultElement>::new()
        .read_ply(&mut reader)
        .map_err(|e| format!("PLY error: {e}"))?;
    let verts = ply.payload.get("vertex").ok_or("PLY has no vertex element")?;
    let mut mesh = Mesh::new(super::file_stem(path));
    let mut has_normals = false;
    for v in verts {
        mesh.positions.push([get(v, "x"), get(v, "y"), get(v, "z")]);
        if v.contains_key("nx") {
            has_normals = true;
            mesh.normals.push([get(v, "nx"), get(v, "ny"), get(v, "nz")]);
        }
    }
    if !has_normals {
        mesh.normals.clear();
    }
    if let Some(faces) = ply.payload.get("face") {
        for f in faces {
            let idx = f
                .get("vertex_indices")
                .or_else(|| f.get("vertex_index"))
                .and_then(list);
            if let Some(idx) = idx {
                fan(&idx, &mut mesh.indices);
            }
        }
    }
    Ok(Model { meshes: vec![mesh], ..Default::default() })
}
