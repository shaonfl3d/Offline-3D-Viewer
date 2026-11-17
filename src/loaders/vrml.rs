//! Minimal VRML97 (.wrl) reader: IndexedFaceSet geometry (Transform nodes are ignored).

use crate::model::{fan, Mesh, Model};
use std::{fs, path::Path};

/// Returns the text between the first '[' ... ']' that follows `key` in `s`.
fn bracket_after<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let at = s.find(key)?;
    let rest = &s[at + key.len()..];
    let open = rest.find('[')?;
    let close = rest[open..].find(']')? + open;
    Some(&rest[open + 1..close])
}

pub fn load(path: &Path) -> Result<Model, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes).replace(',', " ");
    // Strip comments.
    let text: String = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");

    let mut out = Model::default();
    let mut search = 0usize;
    let mut n = 0;
    while let Some(rel) = text[search..].find("IndexedFaceSet") {
        let start = search + rel;
        // Find the matching closing brace of the node body.
        let Some(open) = text[start..].find('{') else { break };
        let body_start = start + open;
        let mut depth = 0i32;
        let mut end = text.len();
        for (i, ch) in text[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = body_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = &text[body_start..end];
        search = end;

        let (Some(points), Some(index)) = (bracket_after(body, "point"), bracket_after(body, "coordIndex")) else {
            continue;
        };
        let nums: Vec<f32> = points.split_whitespace().filter_map(|t| t.parse().ok()).collect();
        let mut mesh = Mesh::new(format!("{}_{n}", super::file_stem(path)));
        mesh.positions = nums.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        let mut poly: Vec<u32> = Vec::new();
        for tok in index.split_whitespace() {
            match tok.parse::<i64>() {
                Ok(-1) => {
                    fan(&poly, &mut mesh.indices);
                    poly.clear();
                }
                Ok(v) if v >= 0 => poly.push(v as u32),
                _ => {}
            }
        }
        fan(&poly, &mut mesh.indices);
        out.meshes.push(mesh);
        n += 1;
    }
    if out.meshes.is_empty() {
        return Err("No IndexedFaceSet geometry found in this VRML file.".into());
    }
    Ok(out)
}
