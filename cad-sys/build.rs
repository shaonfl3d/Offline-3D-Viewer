//! Builds the C++ shim and links OpenCASCADE.
//!
//! * feature `builtin`: OpenCASCADE is built from source (crate `occt-sys`) into `target/OCCT`
//!   and linked statically. This is what the Windows release build uses.
//! * feature `system`: an installed OpenCASCADE is used (Linux packages). Set `OCCT_INCLUDE_DIR`
//!   and `OCCT_LIB_DIR` if it is not in a standard place.

use std::path::{Path, PathBuf};

/// Toolkits in dependency order (a toolkit comes before the ones it needs). The order only
/// matters for static linking with GNU ld; MSVC does not care.
const LIBS_NEW: &[&str] = &[
    "TKDESTEP", "TKDEIGES", "TKDE", "TKXCAF", "TKVCAF", "TKV3d", "TKService", "TKHLR", "TKCAF", "TKLCAF",
    "TKCDF", "TKXSBase", "TKMesh", "TKShHealing", "TKBool", "TKBO", "TKPrim", "TKTopAlgo", "TKGeomAlgo",
    "TKBRep", "TKGeomBase", "TKG3d", "TKG2d", "TKMath", "TKernel",
];
/// Toolkit names used up to OpenCASCADE 7.6.
const LIBS_OLD: &[&str] = &[
    "TKSTEP", "TKSTEP209", "TKSTEPAttr", "TKSTEPBase", "TKIGES", "TKXSBase", "TKMesh", "TKShHealing",
    "TKTopAlgo", "TKGeomAlgo", "TKBRep", "TKGeomBase", "TKG3d", "TKG2d", "TKMath", "TKernel",
];

fn has_lib(dir: &Path, name: &str) -> bool {
    ["lib{}.so", "lib{}.a", "{}.lib", "lib{}.dylib"]
        .iter()
        .any(|pat| dir.join(pat.replace("{}", name)).exists())
}

#[cfg(feature = "builtin")]
fn builtin_occt() -> Option<(PathBuf, PathBuf)> {
    println!("cargo:rerun-if-env-changed=CAD_SYS_REBUILD_OCCT");
    // `target/OCCT`: the same place the `opencascade-sys` crate uses, so an OpenCASCADE that was
    // already built there is reused instead of being compiled again.
    let root = occt_sys::occt_path();
    let (include, lib) = (root.join("include"), root.join("lib"));
    let ready = include.join("Standard_Version.hxx").exists() && LIBS_NEW.iter().all(|l| has_lib(&lib, l));
    if !ready || std::env::var_os("CAD_SYS_REBUILD_OCCT").is_some() {
        // CMake 4 refuses OpenCASCADE's old `cmake_minimum_required` unless this is set.
        if std::env::var_os("CMAKE_POLICY_VERSION_MINIMUM").is_none() {
            std::env::set_var("CMAKE_POLICY_VERSION_MINIMUM", "3.5");
        }
        occt_sys::build_occt();
    }
    Some((include, lib))
}

#[cfg(not(feature = "builtin"))]
fn builtin_occt() -> Option<(PathBuf, PathBuf)> {
    None
}

fn system_occt() -> (PathBuf, PathBuf) {
    println!("cargo:rerun-if-env-changed=OCCT_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=OCCT_LIB_DIR");
    let include = std::env::var_os("OCCT_INCLUDE_DIR").map(PathBuf::from).or_else(|| {
        ["/usr/include/opencascade", "/usr/local/include/opencascade", "/usr/include/occt", "/opt/homebrew/include/opencascade"]
            .iter()
            .map(PathBuf::from)
            .find(|p| p.join("Standard_Version.hxx").exists())
    });
    let lib = std::env::var_os("OCCT_LIB_DIR").map(PathBuf::from).or_else(|| {
        ["/usr/lib/x86_64-linux-gnu", "/usr/lib/aarch64-linux-gnu", "/usr/lib64", "/usr/lib", "/usr/local/lib", "/opt/homebrew/lib"]
            .iter()
            .map(PathBuf::from)
            .find(|p| has_lib(p, "TKernel"))
    });
    match (include, lib) {
        (Some(i), Some(l)) => (i, l),
        _ => panic!(
            "OpenCASCADE was not found. Install its development packages (Debian/Ubuntu: \
             libocct-data-exchange-dev libocct-modeling-algorithms-dev) or set OCCT_INCLUDE_DIR and OCCT_LIB_DIR."
        ),
    }
}

fn main() {
    println!("cargo:rerun-if-changed=cpp/shim.cpp");
    println!("cargo:rerun-if-changed=build.rs");

    let target = std::env::var("TARGET").unwrap_or_default();
    let is_windows = target.contains("windows");
    let is_msvc = target.contains("msvc");

    let (include, lib_dir, is_static) = match builtin_occt() {
        Some((i, l)) => (i, l, true),
        None => {
            let (i, l) = system_occt();
            (i, l, false)
        }
    };

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/shim.cpp")
        .include(&include)
        .define("_USE_MATH_DEFINES", None)
        .warnings(false);
    if is_msvc {
        build.flag("/EHsc").flag("/std:c++14").flag("/utf-8");
    } else {
        build.flag_if_supported("-std=c++14");
    }
    if is_static && is_windows {
        // Without this the OpenCASCADE headers declare everything as DLL imports.
        build.define("OCCT_STATIC_BUILD", None);
    }
    build.compile("viewer3d_cad_shim");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    let libs = if has_lib(&lib_dir, "TKDESTEP") { LIBS_NEW } else { LIBS_OLD };
    // `-bundle`: the linker reads the big OpenCASCADE libraries directly instead of copying them
    // into this crate first.
    let kind = if is_static { "static:-bundle" } else { "dylib" };
    for lib in libs {
        if is_static || has_lib(&lib_dir, lib) {
            println!("cargo:rustc-link-lib={kind}={lib}");
        }
    }
    if is_windows {
        for sys in ["user32", "advapi32", "gdi32", "wsock32", "ws2_32", "psapi", "shell32", "winmm"] {
            println!("cargo:rustc-link-lib=dylib={sys}");
        }
    } else if is_static {
        println!("cargo:rustc-link-lib=dylib=pthread");
        println!("cargo:rustc-link-lib=dylib=dl");
    }
}
