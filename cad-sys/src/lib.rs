//! Safe Rust wrapper around the C++ OpenCASCADE shim (`cpp/shim.cpp`).

use std::ffi::{c_char, c_double, c_int, c_void, CStr, CString};
use std::path::Path;

extern "C" {
    fn vcad_load(
        path: *const c_char,
        kind: c_int,
        linear_rel: c_double,
        angular: c_double,
        err: *mut c_char,
        err_len: c_int,
    ) -> *mut c_void;
    fn vcad_part_count(handle: *const c_void) -> usize;
    fn vcad_part_name(handle: *const c_void, index: usize) -> *const c_char;
    fn vcad_part_vertex_count(handle: *const c_void, index: usize) -> usize;
    fn vcad_part_index_count(handle: *const c_void, index: usize) -> usize;
    fn vcad_part_positions(handle: *const c_void, index: usize) -> *const f32;
    fn vcad_part_normals(handle: *const c_void, index: usize) -> *const f32;
    fn vcad_part_indices(handle: *const c_void, index: usize) -> *const u32;
    fn vcad_free(handle: *mut c_void);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Step = 0,
    Iges = 1,
    Brep = 2,
}

/// One solid (or the loose surfaces) of a CAD file as a triangle mesh.
#[derive(Clone, Debug, Default)]
pub struct Part {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

struct Handle(*mut c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { vcad_free(self.0) }
    }
}

/// Reads a CAD file and tessellates it.
///
/// `linear_rel` is the allowed surface deviation as a fraction of the model's diagonal
/// (0.001 is a good default); `angular` is the allowed angle between facets in radians.
pub fn load(path: &Path, kind: Kind, linear_rel: f64, angular: f64) -> Result<Vec<Part>, String> {
    let text = path.to_str().ok_or_else(|| "The file path is not valid Unicode.".to_string())?;
    let c_path = CString::new(text).map_err(|_| "The file path contains a NUL character.".to_string())?;
    let mut err = [0 as c_char; 512];
    let raw = unsafe {
        vcad_load(c_path.as_ptr(), kind as c_int, linear_rel, angular, err.as_mut_ptr(), err.len() as c_int)
    };
    if raw.is_null() {
        let msg = unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned();
        return Err(if msg.is_empty() { "Could not read the CAD file.".to_string() } else { msg });
    }
    let handle = Handle(raw);
    let count = unsafe { vcad_part_count(handle.0) };
    let mut parts = Vec::with_capacity(count);
    for i in 0..count {
        unsafe {
            let nv = vcad_part_vertex_count(handle.0, i);
            let ni = vcad_part_index_count(handle.0, i);
            let pos = vcad_part_positions(handle.0, i);
            let nrm = vcad_part_normals(handle.0, i);
            let idx = vcad_part_indices(handle.0, i);
            if nv == 0 || ni == 0 || pos.is_null() || idx.is_null() {
                continue;
            }
            let triples = |p: *const f32| -> Vec<[f32; 3]> {
                std::slice::from_raw_parts(p, nv * 3).chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
            };
            let name_ptr = vcad_part_name(handle.0, i);
            let name = if name_ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(name_ptr).to_string_lossy().into_owned()
            };
            parts.push(Part {
                name,
                positions: triples(pos),
                normals: if nrm.is_null() { Vec::new() } else { triples(nrm) },
                indices: std::slice::from_raw_parts(idx, ni).to_vec(),
            });
        }
    }
    if parts.is_empty() {
        return Err("The CAD file contains no solid or surface geometry.".to_string());
    }
    Ok(parts)
}
