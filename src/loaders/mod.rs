//! File-format loaders. Every loader returns a `Model`.

mod bim;
mod collada;
mod fbx;
mod gltf_loader;
mod obj;
mod off;
mod ply;
mod stl;
mod threedmf;
mod threeds;
mod vrml;
mod amf;
#[cfg(any(feature = "cad", feature = "cad-system"))]
pub mod cad;
#[cfg(test)]
mod tests;

use crate::model::Model;
use std::path::Path;

/// CAD extensions (need the OpenCASCADE engine: `--features cad`).
pub const CAD: &[&str] = &["step", "stp", "iges", "igs", "brep", "brp", "fcstd"];

/// Is the CAD engine compiled into this build?
pub const CAD_ENABLED: bool = cfg!(any(feature = "cad", feature = "cad-system"));

/// Extensions the viewer can open (lower-case, without dot).
#[cfg(any(feature = "cad", feature = "cad-system"))]
pub const SUPPORTED: &[&str] = &[
    "stl", "obj", "ply", "off", "gltf", "glb", "3mf", "amf", "dae", "wrl", "3ds", "bim", "fbx",
    "step", "stp", "iges", "igs", "brep", "brp", "fcstd",
];
#[cfg(not(any(feature = "cad", feature = "cad-system")))]
pub const SUPPORTED: &[&str] = &[
    "stl", "obj", "ply", "off", "gltf", "glb", "3mf", "amf", "dae", "wrl", "3ds", "bim", "fbx",
];

/// Formats not available in this build (shown with a clear message).
#[cfg(any(feature = "cad", feature = "cad-system"))]
pub const PLANNED: &[&str] = &["ifc", "3dm"];
#[cfg(not(any(feature = "cad", feature = "cad-system")))]
pub const PLANNED: &[&str] = &["step", "stp", "iges", "igs", "brep", "brp", "fcstd", "ifc", "3dm"];

pub fn is_cad(path: &Path) -> bool {
    CAD.contains(&ext_of(path).as_str())
}

pub fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

pub fn load(path: &Path) -> Result<Model, String> {
    let ext = ext_of(path);
    let mut model = match ext.as_str() {
        "stl" => stl::load(path)?,
        "obj" => obj::load(path)?,
        "ply" => ply::load(path)?,
        "off" => off::load(path)?,
        "gltf" | "glb" => gltf_loader::load(path)?,
        "3mf" => threedmf::load(path)?,
        "amf" => amf::load(path)?,
        "dae" => collada::load(path)?,
        "wrl" => vrml::load(path)?,
        "3ds" => threeds::load(path)?,
        "bim" => bim::load(path)?,
        "fbx" => fbx::load(path)?,
        #[cfg(any(feature = "cad", feature = "cad-system"))]
        e if CAD.contains(&e) => cad::load(path, e)?,
        e if CAD.contains(&e) => {
            return Err(format!(
                ".{e} needs the CAD engine. Rebuild with:  cargo build --release --features cad"
            ))
        }
        e if PLANNED.contains(&e) => {
            return Err(format!(".{e} files are planned for Phase 3 of this app."))
        }
        other => return Err(format!("Unsupported file type: .{other}")),
    };
    model.format = ext.to_ascii_uppercase();
    let model = model.finish();
    if model.meshes.is_empty() {
        return Err("The file was read, but it contains no triangle geometry.".into());
    }
    Ok(model)
}

pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("mesh")
        .to_string()
}
