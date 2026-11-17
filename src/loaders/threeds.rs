use crate::model::{Mesh, Model};
use std::{collections::HashMap, fs, path::Path};

fn u16le(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}
fn u32le(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}
fn f32le(d: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}
fn cstr(d: &[u8], at: usize) -> (String, usize) {
    let mut e = at;
    while e < d.len() && d[e] != 0 {
        e += 1;
    }
    (String::from_utf8_lossy(&d[at..e]).into_owned(), e + 1)
}

struct Ctx {
    out: Model,
    mats: HashMap<String, [f32; 3]>,
}

fn walk(d: &[u8], mut at: usize, end: usize, ctx: &mut Ctx, obj_name: &str, cur_mat: &mut String, mesh: &mut Option<Mesh>) {
    while at + 6 <= end && at + 6 <= d.len() {
        let id = u16le(d, at);
        let len = u32le(d, at + 2) as usize;
        if len < 6 {
            break;
        }
        let body = at + 6;
        let next = (at + len).min(end);
        match id {
            0x4D4D | 0x3D3D | 0xAFFF | 0xA020 => {
                // 0xAFFF material block: reset current material name.
                if id == 0xAFFF {
                    cur_mat.clear();
                }
                walk(d, body, next, ctx, obj_name, cur_mat, mesh);
            }
            0xA000 => {
                *cur_mat = cstr(d, body).0;
            }
            0x0010 if body + 12 <= d.len() => {
                if !cur_mat.is_empty() {
                    ctx.mats.insert(cur_mat.clone(), [f32le(d, body), f32le(d, body + 4), f32le(d, body + 8)]);
                }
            }
            0x0011 if body + 3 <= d.len() => {
                if !cur_mat.is_empty() {
                    ctx.mats.insert(
                        cur_mat.clone(),
                        [d[body] as f32 / 255.0, d[body + 1] as f32 / 255.0, d[body + 2] as f32 / 255.0],
                    );
                }
            }
            0x4000 => {
                let (name, after) = cstr(d, body);
                let mut m = None;
                walk(d, after, next, ctx, &name, cur_mat, &mut m);
                if let Some(m) = m {
                    ctx.out.meshes.push(m);
                }
            }
            0x4100 => {
                let mut m = Mesh::new(obj_name.to_string());
                let mut inner = Some(m.clone());
                walk(d, body, next, ctx, obj_name, cur_mat, &mut inner);
                m = inner.unwrap_or(m);
                *mesh = Some(m);
            }
            0x4110 if body + 2 <= d.len() => {
                let n = u16le(d, body) as usize;
                if let Some(m) = mesh {
                    for i in 0..n {
                        let o = body + 2 + i * 12;
                        if o + 12 > d.len() {
                            break;
                        }
                        m.positions.push([f32le(d, o), f32le(d, o + 4), f32le(d, o + 8)]);
                    }
                }
            }
            0x4120 if body + 2 <= d.len() => {
                let n = u16le(d, body) as usize;
                let mut sub = body + 2 + n * 8;
                if let Some(m) = mesh {
                    for i in 0..n {
                        let o = body + 2 + i * 8;
                        if o + 8 > d.len() {
                            break;
                        }
                        m.indices.extend_from_slice(&[u16le(d, o) as u32, u16le(d, o + 2) as u32, u16le(d, o + 4) as u32]);
                    }
                }
                // Sub-chunks of the face list (material assignment).
                while sub + 6 <= next {
                    let sid = u16le(d, sub);
                    let slen = u32le(d, sub + 2) as usize;
                    if slen < 6 {
                        break;
                    }
                    if sid == 0x4130 {
                        let (mname, _) = cstr(d, sub + 6);
                        if let Some(m) = mesh {
                            if m.material.is_none() {
                                m.material = Some(mname);
                            }
                        }
                    }
                    sub += slen;
                }
            }
            _ => {}
        }
        at += len;
    }
}

pub fn load(path: &Path) -> Result<Model, String> {
    let d = fs::read(path).map_err(|e| e.to_string())?;
    if d.len() < 6 || u16le(&d, 0) != 0x4D4D {
        return Err("Not a valid 3DS file".into());
    }
    let mut ctx = Ctx { out: Model::default(), mats: HashMap::new() };
    let mut cur = String::new();
    let mut none = None;
    walk(&d, 0, d.len(), &mut ctx, "", &mut cur, &mut none);
    let mats = ctx.mats;
    let mut out = ctx.out;
    for m in &mut out.meshes {
        if let Some(name) = &m.material {
            if let Some(c) = mats.get(name) {
                m.color = *c;
            }
        }
    }
    Ok(out)
}
