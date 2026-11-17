use crate::model::{Mesh, Model};
use glam::{Mat4, Quat, Vec3};
use roxmltree::{Document, Node};
use std::{collections::HashMap, fs, path::Path};

fn kids<'a, 'i>(n: Node<'a, 'i>, name: &'static str) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(move |c| c.is_element() && c.tag_name().name() == name)
}
fn kid<'a, 'i>(n: Node<'a, 'i>, name: &'static str) -> Option<Node<'a, 'i>> {
    kids(n, name).next()
}
fn floats(s: &str) -> Vec<f32> {
    s.split_whitespace().filter_map(|t| t.parse().ok()).collect()
}
fn ints(s: &str) -> Vec<u32> {
    s.split_whitespace().filter_map(|t| t.parse().ok()).collect()
}
fn strip(s: &str) -> &str {
    s.trim_start_matches('#')
}

struct Source {
    data: Vec<f32>,
    stride: usize,
}

fn parse_geometry(g: Node) -> Vec<Mesh> {
    let mut out = Vec::new();
    let Some(mesh) = kid(g, "mesh") else { return out };
    let gname = g.attribute("name").or(g.attribute("id")).unwrap_or("geometry").to_string();

    let mut sources: HashMap<&str, Source> = HashMap::new();
    for s in kids(mesh, "source") {
        let Some(id) = s.attribute("id") else { continue };
        let Some(fa) = kid(s, "float_array") else { continue };
        let stride = kid(s, "technique_common")
            .and_then(|t| kid(t, "accessor"))
            .and_then(|a| a.attribute("stride"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);
        sources.insert(id, Source { data: floats(fa.text().unwrap_or("")), stride });
    }
    // <vertices id> -> POSITION source id
    let mut vertex_pos: HashMap<&str, &str> = HashMap::new();
    for v in kids(mesh, "vertices") {
        if let Some(id) = v.attribute("id") {
            for i in kids(v, "input") {
                if i.attribute("semantic") == Some("POSITION") {
                    if let Some(src) = i.attribute("source") {
                        vertex_pos.insert(id, strip(src));
                    }
                }
            }
        }
    }

    for (pi, prim) in mesh
        .children()
        .filter(|c| c.is_element() && matches!(c.tag_name().name(), "triangles" | "polylist" | "polygons"))
        .enumerate()
    {
        let kind = prim.tag_name().name();
        struct Inp<'a> {
            sem: &'a str,
            off: usize,
            src: &'a str,
        }
        let mut inputs = Vec::new();
        for i in kids(prim, "input") {
            inputs.push(Inp {
                sem: i.attribute("semantic").unwrap_or(""),
                off: i.attribute("offset").and_then(|v| v.parse().ok()).unwrap_or(0),
                src: strip(i.attribute("source").unwrap_or("")),
            });
        }
        let stride = inputs.iter().map(|i| i.off).max().unwrap_or(0) + 1;
        let Some(vin) = inputs.iter().find(|i| i.sem == "VERTEX") else { continue };
        let Some(pos_src) = vertex_pos.get(vin.src).and_then(|id| sources.get(id)) else { continue };
        let nin = inputs.iter().find(|i| i.sem == "NORMAL");
        let nsrc = nin.and_then(|i| sources.get(i.src));

        // Gather polygons as lists of "corner" index groups.
        let mut polys: Vec<Vec<u32>> = Vec::new(); // each is flat list of stride-sized groups
        match kind {
            "triangles" => {
                let p: Vec<u32> = kid(prim, "p").map(|n| ints(n.text().unwrap_or(""))).unwrap_or_default();
                for tri in p.chunks_exact(stride * 3) {
                    polys.push(tri.to_vec());
                }
            }
            "polylist" => {
                let vc: Vec<u32> = kid(prim, "vcount").map(|n| ints(n.text().unwrap_or(""))).unwrap_or_default();
                let p: Vec<u32> = kid(prim, "p").map(|n| ints(n.text().unwrap_or(""))).unwrap_or_default();
                let mut at = 0usize;
                for c in vc {
                    let len = c as usize * stride;
                    if at + len > p.len() {
                        break;
                    }
                    polys.push(p[at..at + len].to_vec());
                    at += len;
                }
            }
            _ => {
                for p in kids(prim, "p") {
                    polys.push(ints(p.text().unwrap_or("")));
                }
            }
        }

        let mut m = Mesh::new(if pi == 0 { gname.clone() } else { format!("{gname}_{pi}") });
        m.material = prim.attribute("material").map(str::to_string);
        let mut normals = Vec::new();
        for poly in polys {
            let corners = poly.len() / stride;
            if corners < 3 {
                continue;
            }
            let base = m.positions.len() as u32;
            for c in 0..corners {
                let g = &poly[c * stride..(c + 1) * stride];
                let pi = g[vin.off] as usize * pos_src.stride;
                if pi + 2 >= pos_src.data.len() {
                    m.positions.push([0.0; 3]);
                } else {
                    m.positions.push([pos_src.data[pi], pos_src.data[pi + 1], pos_src.data[pi + 2]]);
                }
                if let (Some(ni), Some(ns)) = (nin, nsrc) {
                    let k = g[ni.off] as usize * ns.stride;
                    if k + 2 < ns.data.len() {
                        normals.push([ns.data[k], ns.data[k + 1], ns.data[k + 2]]);
                    } else {
                        normals.push([0.0, 1.0, 0.0]);
                    }
                }
            }
            let idx: Vec<u32> = (0..corners as u32).map(|i| base + i).collect();
            crate::model::fan(&idx, &mut m.indices);
        }
        if normals.len() == m.positions.len() {
            m.normals = normals;
        }
        out.push(m);
    }
    out
}

fn node_matrix(n: Node) -> Mat4 {
    let mut m = Mat4::IDENTITY;
    for c in n.children().filter(|c| c.is_element()) {
        let v = floats(c.text().unwrap_or(""));
        match c.tag_name().name() {
            "matrix" if v.len() == 16 => {
                // Collada matrices are row-major.
                m *= Mat4::from_cols_array(&v.clone().try_into().unwrap_or([0.0; 16])).transpose();
            }
            "translate" if v.len() >= 3 => m *= Mat4::from_translation(Vec3::new(v[0], v[1], v[2])),
            "scale" if v.len() >= 3 => m *= Mat4::from_scale(Vec3::new(v[0], v[1], v[2])),
            "rotate" if v.len() >= 4 => {
                let axis = Vec3::new(v[0], v[1], v[2]).normalize_or_zero();
                if axis != Vec3::ZERO {
                    m *= Mat4::from_quat(Quat::from_axis_angle(axis, v[3].to_radians()));
                }
            }
            _ => {}
        }
    }
    m
}

pub fn load(path: &Path) -> Result<Model, String> {
    let xml = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let opts = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = Document::parse_with_options(&xml, opts).map_err(|e| format!("DAE XML error: {e}"))?;
    let root = doc.root_element();

    let mut geoms: HashMap<String, Vec<Mesh>> = HashMap::new();
    let mut order = Vec::new();
    for g in root.descendants().filter(|n| n.has_tag_name("geometry")) {
        if let Some(id) = g.attribute("id") {
            geoms.insert(id.to_string(), parse_geometry(g));
            order.push(id.to_string());
        }
    }
    let mut nodes_by_id: HashMap<&str, Node> = HashMap::new();
    for n in root.descendants().filter(|n| n.has_tag_name("node")) {
        if let Some(id) = n.attribute("id") {
            nodes_by_id.insert(id, n);
        }
    }

    fn visit(n: Node, parent: Mat4, geoms: &HashMap<String, Vec<Mesh>>, nodes: &HashMap<&str, Node>, out: &mut Model, depth: u32) {
        if depth > 32 {
            return;
        }
        let world = parent * node_matrix(n);
        for ig in kids(n, "instance_geometry") {
            if let Some(list) = ig.attribute("url").and_then(|u| geoms.get(strip(u))) {
                for m in list {
                    let mut m = m.clone();
                    m.transform(world);
                    out.meshes.push(m);
                }
            }
        }
        for inode in kids(n, "instance_node") {
            if let Some(t) = inode.attribute("url").and_then(|u| nodes.get(strip(u))) {
                visit(*t, world, geoms, nodes, out, depth + 1);
            }
        }
        for c in kids(n, "node") {
            visit(c, world, geoms, nodes, out, depth + 1);
        }
    }

    let mut out = Model::default();
    if let Some(vs) = root.descendants().find(|n| n.has_tag_name("visual_scene")) {
        for n in kids(vs, "node") {
            visit(n, Mat4::IDENTITY, &geoms, &nodes_by_id, &mut out, 0);
        }
    }
    if out.meshes.is_empty() {
        for id in order {
            out.meshes.extend(geoms.remove(&id).unwrap_or_default());
        }
    }
    // Z-up files: rotate to Y-up so they open upright.
    let z_up = root
        .descendants()
        .find(|n| n.has_tag_name("up_axis"))
        .and_then(|n| n.text())
        .map(|t| t.trim() == "Z_UP")
        .unwrap_or(false);
    if z_up {
        let r = Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        for m in &mut out.meshes {
            m.transform(r);
        }
    }
    Ok(out)
}
