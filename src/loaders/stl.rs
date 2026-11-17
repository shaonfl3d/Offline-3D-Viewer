use crate::model::{Mesh, Model};
use std::{fs, io::Cursor, path::Path};

pub fn load(path: &Path) -> Result<Model, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    let stl = stl_io::read_stl(&mut Cursor::new(data)).map_err(|e| format!("STL error: {e}"))?;
    let mut mesh = Mesh::new(super::file_stem(path));
    mesh.positions = stl.vertices.iter().map(|v| [v[0], v[1], v[2]]).collect();
    for f in &stl.faces {
        for &i in &f.vertices {
            mesh.indices.push(i as u32);
        }
    }
    Ok(Model { meshes: vec![mesh], ..Default::default() })
}
