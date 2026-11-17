//! CAD import through OpenCASCADE: STEP, IGES, BREP and FreeCAD (FCStd).
//! Compiled only with `--features cad` (or `cad-system`). The OpenCASCADE calls live in the
//! small `cad-sys` crate next to this project.

use crate::model::{Mesh, Model};
use cad_sys::Kind;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
};

/// 0 = draft, 1 = normal, 2 = fine
static QUALITY: AtomicU8 = AtomicU8::new(1);

pub fn quality() -> u8 {
    QUALITY.load(Ordering::Relaxed)
}
pub fn set_quality(q: u8) {
    QUALITY.store(q.min(2), Ordering::Relaxed);
}

/// (surface deviation as a fraction of the model size, facet angle in radians)
fn settings() -> (f64, f64) {
    match quality() {
        0 => (0.004, 0.6),
        1 => (0.001, 0.4),
        _ => (0.00025, 0.2),
    }
}

/// Deletes a temporary file when dropped.
struct TempFile(PathBuf);
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn temp_path(tag: &str, ext: &str) -> PathBuf {
    use std::sync::atomic::AtomicU32;
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("viewer3d_{}_{}_{}.{}", tag, std::process::id(), n, ext))
}

/// Reads one CAD file into meshes, one per solid.
fn read(path: &Path, kind: Kind, base_name: &str) -> Result<Vec<Mesh>, String> {
    // OpenCASCADE takes plain C strings. A path with non-English letters is copied to a
    // temporary file with a simple name first, so such folders and file names always work.
    let is_plain = path.to_str().map(|s| s.is_ascii()).unwrap_or(false);
    let mut _guard = None;
    let mut source = path.to_path_buf();
    if !is_plain {
        let ext = path.extension().and_then(|e| e.to_str()).filter(|e| e.is_ascii()).unwrap_or("cad");
        let tmp = temp_path("cad", ext);
        fs::copy(path, &tmp).map_err(|e| format!("Could not prepare the file for reading: {e}"))?;
        source = tmp.clone();
        _guard = Some(TempFile(tmp));
    }

    let (linear, angular) = settings();
    let parts = cad_sys::load(&source, kind, linear, angular)?;
    let single = parts.len() == 1;
    Ok(parts
        .into_iter()
        .map(|p| {
            let mut m = Mesh::new(if single { base_name.to_string() } else { format!("{base_name} / {}", p.name) });
            m.positions = p.positions;
            m.normals = p.normals;
            m.indices = p.indices;
            m
        })
        .collect())
}

/// FreeCAD files are zip archives: Document.xml names the objects, *.brp hold their shapes.
fn load_fcstd(path: &Path) -> Result<Model, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("FCStd is not a valid archive: {e}"))?;

    // Which shape file belongs to which object: (brp file, label, object name).
    let mut objects: Vec<(String, String, String)> = Vec::new();
    if let Ok(mut doc) = zip.by_name("Document.xml") {
        let mut xml = String::new();
        let _ = doc.read_to_string(&mut xml);
        if let Ok(d) = roxmltree::Document::parse(&xml) {
            for obj in d.descendants().filter(|n| n.has_tag_name("Object") && n.attribute("name").is_some()) {
                let name = obj.attribute("name").unwrap_or("part").to_string();
                let mut label = name.clone();
                let mut file = None;
                for prop in obj.descendants().filter(|n| n.has_tag_name("Property")) {
                    match prop.attribute("name") {
                        Some("Label") => {
                            if let Some(s) = prop.children().find(|c| c.has_tag_name("String")) {
                                if let Some(v) = s.attribute("value") {
                                    label = v.to_string();
                                }
                            }
                        }
                        Some("Shape") => {
                            if let Some(p) = prop.children().find(|c| c.has_tag_name("Part")) {
                                file = p.attribute("file").map(str::to_string);
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(f) = file {
                    if !f.is_empty() {
                        objects.push((f, label, name));
                    }
                }
            }
        }
    }
    if objects.is_empty() {
        // Fall back to every .brp in the archive.
        for i in 0..zip.len() {
            let n = zip.by_index(i).map_err(|e| e.to_string())?.name().to_string();
            if n.to_ascii_lowercase().ends_with(".brp") {
                objects.push((n.clone(), n.clone(), n));
            }
        }
    }

    // A FreeCAD document keeps every modelling step (pads, pockets, the raw billet ...) as its own
    // shape. GuiDocument.xml records which ones the author left visible.
    let mut shown: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    if let Ok(mut gui) = zip.by_name("GuiDocument.xml") {
        let mut xml = String::new();
        let _ = gui.read_to_string(&mut xml);
        if let Ok(d) = roxmltree::Document::parse(&xml) {
            for vp in d.descendants().filter(|n| n.has_tag_name("ViewProvider")) {
                let Some(name) = vp.attribute("name") else { continue };
                let visible = vp
                    .descendants()
                    .find(|n| n.has_tag_name("Property") && n.attribute("name") == Some("Visibility"))
                    .and_then(|p| p.children().find(|c| c.has_tag_name("Bool")))
                    .and_then(|b| b.attribute("value"))
                    .map(|v| v == "true");
                if let Some(v) = visible {
                    shown.insert(name.to_string(), v);
                }
            }
        }
    }

    let mut out = Model::default();
    for (file, label, name) in &objects {
        let mut data = Vec::new();
        match zip.by_name(file) {
            Ok(mut f) => {
                if f.read_to_end(&mut data).is_err() {
                    continue;
                }
            }
            Err(_) => continue,
        }
        let tmp = TempFile(temp_path("fcstd", "brp"));
        if fs::write(&tmp.0, &data).is_err() {
            continue;
        }
        // Helper objects without a usable shape are skipped.
        if let Ok(mut meshes) = read(&tmp.0, Kind::Brep, label) {
            let hidden = shown.get(name).map(|v| !*v).unwrap_or(false);
            for m in &mut meshes {
                m.hidden = hidden;
            }
            out.meshes.extend(meshes);
        }
    }
    if out.meshes.is_empty() {
        return Err("No renderable shapes were found in this FreeCAD file.".into());
    }
    // Never open to an empty view.
    if out.meshes.iter().all(|m| m.hidden) {
        for m in &mut out.meshes {
            m.hidden = false;
        }
    }
    Ok(out)
}

pub fn load(path: &Path, ext: &str) -> Result<Model, String> {
    let name = super::file_stem(path);
    let kind = match ext {
        "step" | "stp" => Kind::Step,
        "iges" | "igs" => Kind::Iges,
        "brep" | "brp" => Kind::Brep,
        "fcstd" => return load_fcstd(path),
        _ => return Err(format!("Unsupported CAD format: .{ext}")),
    };
    Ok(Model { meshes: read(path, kind, &name)?, ..Default::default() })
}
