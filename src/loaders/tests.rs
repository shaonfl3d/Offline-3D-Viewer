//! Loader self-tests: every format is checked against a unit cube (volume 1, area 6).

use super::load;
use std::io::Write;

const V: [[f32; 3]; 8] = [
    [0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.],
    [0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.],
];
const QUADS: [[u32; 4]; 6] = [
    [0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [3, 7, 6, 2], [0, 4, 7, 3], [1, 2, 6, 5],
];

fn tris() -> Vec<[u32; 3]> {
    QUADS.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect()
}

fn tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("viewer3d_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn check(name: &str, bytes: &[u8]) {
    let p = tmp(name, bytes);
    let m = load(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
    let s = m.stats();
    assert_eq!(s.triangles, 12, "{name}: triangles");
    assert!((s.volume.abs() - 1.0).abs() < 1e-4, "{name}: volume {}", s.volume);
    assert!((s.surface - 6.0).abs() < 1e-4, "{name}: surface {}", s.surface);
}

fn verts_text(prefix: &str) -> String {
    V.iter().map(|v| format!("{prefix}{} {} {}\n", v[0], v[1], v[2])).collect()
}

#[test]
fn off() {
    let mut s = String::from("OFF\n8 6 0\n");
    s += &verts_text("");
    for q in QUADS { s += &format!("4 {} {} {} {}\n", q[0], q[1], q[2], q[3]); }
    check("cube.off", s.as_bytes());
}

#[test]
fn obj() {
    let mut s = verts_text("v ");
    for q in QUADS { s += &format!("f {} {} {} {}\n", q[0] + 1, q[1] + 1, q[2] + 1, q[3] + 1); }
    check("cube.obj", s.as_bytes());
}

#[test]
fn ply_ascii() {
    let mut s = String::from("ply\nformat ascii 1.0\nelement vertex 8\nproperty float x\nproperty float y\nproperty float z\nelement face 6\nproperty list uchar int vertex_indices\nend_header\n");
    s += &verts_text("");
    for q in QUADS { s += &format!("4 {} {} {} {}\n", q[0], q[1], q[2], q[3]); }
    check("cube.ply", s.as_bytes());
}

#[test]
fn stl_ascii_and_binary() {
    let mut a = String::from("solid cube\n");
    let mut b = vec![0u8; 80];
    b.extend_from_slice(&12u32.to_le_bytes());
    for t in tris() {
        a += "facet normal 0 0 0\nouter loop\n";
        b.extend_from_slice(&[0u8; 12]);
        for i in t {
            let v = V[i as usize];
            a += &format!("vertex {} {} {}\n", v[0], v[1], v[2]);
            for c in v { b.extend_from_slice(&c.to_le_bytes()); }
        }
        a += "endloop\nendfacet\n";
        b.extend_from_slice(&[0u8; 2]);
    }
    a += "endsolid cube\n";
    check("cube_ascii.stl", a.as_bytes());
    check("cube_bin.stl", &b);
}

#[test]
fn threemf() {
    let mut xml = String::from("<?xml version=\"1.0\"?><model xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\"><resources><object id=\"1\" type=\"model\"><mesh><vertices>");
    for v in V { xml += &format!("<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>", v[0], v[1], v[2]); }
    xml += "</vertices><triangles>";
    for t in tris() { xml += &format!("<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>", t[0], t[1], t[2]); }
    xml += "</triangles></mesh></object></resources><build><item objectid=\"1\" transform=\"1 0 0 0 1 0 0 0 1 5 5 5\"/></build></model>";
    let mut buf = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        z.start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(xml.as_bytes()).unwrap();
        z.finish().unwrap();
    }
    check("cube.3mf", &buf);
}

#[test]
fn amf() {
    let mut xml = String::from("<?xml version=\"1.0\"?><amf><object id=\"0\"><mesh><vertices>");
    for v in V { xml += &format!("<vertex><coordinates><x>{}</x><y>{}</y><z>{}</z></coordinates></vertex>", v[0], v[1], v[2]); }
    xml += "</vertices><volume>";
    for t in tris() { xml += &format!("<triangle><v1>{}</v1><v2>{}</v2><v3>{}</v3></triangle>", t[0], t[1], t[2]); }
    xml += "</volume></mesh></object></amf>";
    check("cube.amf", xml.as_bytes());
}

#[test]
fn collada() {
    let pos: String = V.iter().map(|v| format!("{} {} {} ", v[0], v[1], v[2])).collect();
    let vcount = "4 ".repeat(6);
    let p: String = QUADS.iter().flatten().map(|i| format!("{i} ")).collect();
    let xml = format!(r##"<?xml version="1.0"?><COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
<asset><up_axis>Y_UP</up_axis></asset>
<library_geometries><geometry id="g" name="cube"><mesh>
<source id="s"><float_array id="a" count="24">{pos}</float_array><technique_common><accessor source="#a" count="8" stride="3"/></technique_common></source>
<vertices id="v"><input semantic="POSITION" source="#s"/></vertices>
<polylist count="6"><input semantic="VERTEX" source="#v" offset="0"/><vcount>{vcount}</vcount><p>{p}</p></polylist>
</mesh></geometry></library_geometries>
<library_visual_scenes><visual_scene id="vs"><node><translate>2 0 0</translate><instance_geometry url="#g"/></node></visual_scene></library_visual_scenes>
</COLLADA>"##);
    check("cube.dae", xml.as_bytes());
}

#[test]
fn vrml() {
    let pts: String = V.iter().map(|v| format!("{} {} {},\n", v[0], v[1], v[2])).collect();
    let idx: String = QUADS.iter().map(|q| format!("{},{},{},{},-1,\n", q[0], q[1], q[2], q[3])).collect();
    let s = format!("#VRML V2.0 utf8\nShape {{ geometry IndexedFaceSet {{ coord Coordinate {{ point [ {pts} ] }} coordIndex [ {idx} ] }} }}");
    check("cube.wrl", s.as_bytes());
}

#[test]
fn bim() {
    let coords: Vec<f32> = V.iter().flatten().copied().collect();
    let idx: Vec<u32> = tris().iter().flatten().copied().collect();
    let j = serde_json::json!({
        "schema_version": "1.0.0",
        "meshes": [{"mesh_id": 0, "coordinates": coords, "indices": idx}],
        "elements": [{"mesh_id": 0, "vector": {"x": 3.0, "y": 0.0, "z": 0.0},
                      "rotation": {"qx": 0.0, "qy": 0.0, "qz": 0.0, "qw": 1.0},
                      "type": "Wall", "color": {"r": 200, "g": 100, "b": 50, "a": 255}}]
    });
    check("cube.bim", j.to_string().as_bytes());
}

#[test]
fn glb() {
    let mut bin = Vec::new();
    for v in V { for c in v { bin.extend_from_slice(&c.to_le_bytes()); } }
    for t in tris() { for i in t { bin.extend_from_slice(&(i as u16).to_le_bytes()); } }
    while bin.len() % 4 != 0 { bin.push(0); }
    let json = serde_json::json!({
        "asset": {"version": "2.0"},
        "scene": 0, "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "translation": [1.0, 2.0, 3.0]}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1}]}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": 96},
            {"buffer": 0, "byteOffset": 96, "byteLength": 72}],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 8, "type": "VEC3",
             "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0]},
            {"bufferView": 1, "componentType": 5123, "count": 36, "type": "SCALAR"}]
    });
    let mut js = json.to_string().into_bytes();
    while js.len() % 4 != 0 { js.push(b' '); }
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let mut g = Vec::new();
    g.extend_from_slice(b"glTF");
    g.extend_from_slice(&2u32.to_le_bytes());
    g.extend_from_slice(&(total as u32).to_le_bytes());
    g.extend_from_slice(&(js.len() as u32).to_le_bytes());
    g.extend_from_slice(b"JSON");
    g.extend_from_slice(&js);
    g.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    g.extend_from_slice(b"BIN\0");
    g.extend_from_slice(&bin);
    check("cube.glb", &g);
}

#[test]
fn threeds() {
    fn chunk(id: u16, body: Vec<u8>) -> Vec<u8> {
        let mut c = id.to_le_bytes().to_vec();
        c.extend_from_slice(&((body.len() + 6) as u32).to_le_bytes());
        c.extend(body);
        c
    }
    let mut verts = (8u16).to_le_bytes().to_vec();
    for v in V { for c in v { verts.extend_from_slice(&c.to_le_bytes()); } }
    let mut faces = (12u16).to_le_bytes().to_vec();
    for t in tris() {
        for i in t { faces.extend_from_slice(&(i as u16).to_le_bytes()); }
        faces.extend_from_slice(&0u16.to_le_bytes());
    }
    let mut tri = chunk(0x4110, verts);
    tri.extend(chunk(0x4120, faces));
    let mut obj = b"Cube\0".to_vec();
    obj.extend(chunk(0x4100, tri));
    let file = chunk(0x4D4D, chunk(0x3D3D, chunk(0x4000, obj)));
    check("cube.3ds", &file);
}

#[test]
fn unsupported_and_planned_report_clean_errors() {
    let p = tmp("x.step", b"ISO-10303-21;");
    assert!(!load(&p).unwrap_err().is_empty());
    let p = tmp("x.xyz", b"");
    assert!(load(&p).unwrap_err().contains("Unsupported"));
}

/// CAD formats: only in builds with the OpenCASCADE engine (`--features cad` / `cad-system`).
#[cfg(any(feature = "cad", feature = "cad-system"))]
mod cad {
    use super::{load, tmp};
    use std::io::Write;
    use std::path::PathBuf;

    fn sample(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cad-sys").join("tests").join("data").join(name)
    }

    #[test]
    fn step_assembly_opens_as_three_meshes() {
        let m = load(&sample("assembly.step")).unwrap();
        assert_eq!(m.format, "STEP");
        assert_eq!(m.meshes.len(), 3);
        assert!(m.meshes.iter().all(|x| x.normals.len() == x.positions.len()));
        let s = m.stats();
        let expected = 6000.0 + std::f64::consts::PI * 1000.0 + 4.0 / 3.0 * std::f64::consts::PI * 512.0;
        assert!((s.volume - expected).abs() < expected * 0.02, "volume {}", s.volume);
    }

    #[test]
    fn iges_and_brep_open() {
        assert!(load(&sample("cylinder.iges")).unwrap().stats().triangles > 20);
        let b = load(&sample("box.brep")).unwrap();
        assert!((b.stats().volume - 6000.0).abs() < 1e-2);
    }

    #[test]
    fn path_with_non_english_letters() {
        let bytes = std::fs::read(sample("box.step")).unwrap();
        let p = tmp("মডেল বাক্স ñ.step", &bytes);
        let m = load(&p).unwrap();
        assert_eq!(m.stats().triangles, 12);
    }

    #[test]
    fn freecad_document() {
        let brep = std::fs::read(sample("box.brep")).unwrap();
        let xml = r#"<?xml version='1.0' encoding='utf-8'?>
<Document SchemaVersion="4"><ObjectData Count="1">
<Object name="Box"><Properties Count="2">
<Property name="Label" type="App::PropertyString"><String value="My Box"/></Property>
<Property name="Shape" type="Part::PropertyPartShape"><Part file="PartShape.brp"/></Property>
</Properties></Object></ObjectData></Document>"#;
        let mut buf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("Document.xml", o).unwrap();
            z.write_all(xml.as_bytes()).unwrap();
            z.start_file("PartShape.brp", o).unwrap();
            z.write_all(&brep).unwrap();
            z.finish().unwrap();
        }
        let m = load(&tmp("doc.FCStd", &buf)).unwrap();
        assert_eq!(m.meshes.len(), 1);
        assert_eq!(m.meshes[0].name, "My Box");
        assert!((m.stats().volume - 6000.0).abs() < 1e-2);
    }
}

/// Opens every file in the folder named by the VIEWER3D_SAMPLES environment variable and prints
/// what was found. Use it to check a folder of your own models:
///   PowerShell:  $env:VIEWER3D_SAMPLES="D:\\models"; cargo test --release --features cad -- --ignored --nocapture samples
#[test]
#[ignore]
fn samples_folder() {
    let Some(dir) = std::env::var_os("VIEWER3D_SAMPLES") else {
        println!("VIEWER3D_SAMPLES is not set");
        return;
    };
    let mut failed = 0;
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries.into_iter().filter(|p| p.is_file()) {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let t = std::time::Instant::now();
        match load(&path) {
            Ok(m) => {
                let s = m.stats();
                println!(
                    "OK    {name}: {} meshes, {} triangles, size {:.1} x {:.1} x {:.1}, {} ms",
                    m.meshes.len(), s.triangles, s.size[0], s.size[1], s.size[2], t.elapsed().as_millis()
                );
                let hidden = m.meshes.iter().filter(|x| x.hidden).count();
                if hidden > 0 {
                    println!("        ({hidden} hidden at first)");
                }
                for mesh in m.meshes.iter().filter(|x| !x.hidden).take(6) {
                    println!("        - {}", mesh.name);
                }
            }
            Err(e) => {
                failed += 1;
                println!("FAIL  {name}: {e}");
            }
        }
    }
    assert_eq!(failed, 0, "{failed} file(s) could not be opened");
}
