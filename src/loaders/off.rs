use crate::model::{fan, Mesh, Model};
use std::{fs, path::Path};

pub fn load(path: &Path) -> Result<Model, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut lines = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty());

    let first = lines.next().ok_or("Empty OFF file")?;
    let mut counts_line = first.to_string();
    let upper = first.to_ascii_uppercase();
    if upper.starts_with("OFF") || upper.starts_with("COFF") || upper.starts_with("NOFF") {
        let rest = first[first.find(|c: char| c.is_whitespace()).unwrap_or(first.len())..].trim();
        counts_line = if rest.is_empty() {
            lines.next().ok_or("Missing OFF counts")?.to_string()
        } else {
            rest.to_string()
        };
    }
    let counts: Vec<usize> = counts_line
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    if counts.len() < 2 {
        return Err("Invalid OFF header".into());
    }
    let (nv, nf) = (counts[0], counts[1]);
    let mut mesh = Mesh::new(super::file_stem(path));
    for _ in 0..nv {
        let l = lines.next().ok_or("OFF: not enough vertices")?;
        let v: Vec<f32> = l.split_whitespace().take(3).filter_map(|t| t.parse().ok()).collect();
        if v.len() < 3 {
            return Err("OFF: bad vertex line".into());
        }
        mesh.positions.push([v[0], v[1], v[2]]);
    }
    for _ in 0..nf {
        let l = lines.next().ok_or("OFF: not enough faces")?;
        let mut it = l.split_whitespace();
        let n: usize = it.next().and_then(|t| t.parse().ok()).ok_or("OFF: bad face")?;
        let idx: Vec<u32> = it.take(n).filter_map(|t| t.parse().ok()).collect();
        fan(&idx, &mut mesh.indices);
    }
    Ok(Model { meshes: vec![mesh], ..Default::default() })
}
