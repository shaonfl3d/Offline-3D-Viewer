//! End-to-end tests of the OpenCASCADE shim on real STEP / IGES / BREP files.
//! The sample files in `tests/data` were written by OpenCASCADE itself.

use cad_sys::{load, Kind, Part};
use std::path::PathBuf;

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(name)
}

/// Signed volume (positive when triangles face outwards) and surface area.
fn measure(parts: &[Part]) -> (f64, f64) {
    let (mut vol, mut area) = (0.0f64, 0.0f64);
    for p in parts {
        assert_eq!(p.indices.len() % 3, 0);
        assert_eq!(p.normals.len(), p.positions.len(), "{}: one normal per vertex", p.name);
        for t in p.indices.chunks_exact(3) {
            let v = |i: u32| {
                let q = p.positions[i as usize];
                [q[0] as f64, q[1] as f64, q[2] as f64]
            };
            let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
            area += 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            vol += (a[0] * n[0] + a[1] * n[1] + a[2] * n[2]) / 6.0;
        }
    }
    (vol, area)
}

fn close(actual: f64, expected: f64, rel: f64) -> bool {
    (actual - expected).abs() <= expected.abs() * rel
}

const PI: f64 = std::f64::consts::PI;

#[test]
fn step_box_is_exact() {
    let parts = load(&data("box.step"), Kind::Step, 0.001, 0.5).unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].name, "Solid 1");
    assert_eq!(parts[0].indices.len() / 3, 12);
    let (vol, area) = measure(&parts);
    assert!(close(vol, 6000.0, 1e-6), "volume {vol}");
    assert!(close(area, 2200.0, 1e-6), "area {area}");
    // Flat faces: every normal is axis-aligned, so the box keeps sharp edges.
    for n in &parts[0].normals {
        let m = n[0].abs().max(n[1].abs()).max(n[2].abs());
        assert!(m > 0.999, "normal {n:?}");
    }
}

#[test]
fn step_assembly_gives_one_part_per_solid() {
    let parts = load(&data("assembly.step"), Kind::Step, 0.0005, 0.2).unwrap();
    assert_eq!(parts.len(), 3, "{:?}", parts.iter().map(|p| &p.name).collect::<Vec<_>>());
    let mut vols: Vec<f64> = parts.iter().map(|p| measure(std::slice::from_ref(p)).0).collect();
    vols.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(close(vols[0], 4.0 / 3.0 * PI * 512.0, 0.02), "sphere {}", vols[0]);
    assert!(close(vols[1], PI * 25.0 * 40.0, 0.02), "cylinder {}", vols[1]);
    assert!(close(vols[2], 6000.0, 1e-6), "box {}", vols[2]);
    // The cylinder was moved by +50 in X inside the file: placement must be applied.
    let cyl = parts.iter().find(|p| close(measure(std::slice::from_ref(p)).0, PI * 1000.0, 0.02)).unwrap();
    let min_x = cyl.positions.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
    let max_x = cyl.positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
    assert!((min_x - 45.0).abs() < 0.05 && (max_x - 55.0).abs() < 0.05, "x range {min_x}..{max_x}");
}

#[test]
fn surfaces_outside_solids_are_kept() {
    let parts = load(&data("solid_and_surface.step"), Kind::Step, 0.001, 0.5).unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[1].name, "Loose surfaces");
    let (_, area) = measure(&parts[1..]);
    assert!(close(area, 100.0, 1e-6), "plane area {area}");
}

#[test]
fn iges_cylinder_has_smooth_outward_normals() {
    let parts = load(&data("cylinder.iges"), Kind::Iges, 0.0005, 0.2).unwrap();
    let (vol, area) = measure(&parts);
    assert!(close(vol, PI * 25.0 * 40.0, 0.02), "volume {vol}");
    assert!(close(area, 2.0 * PI * 5.0 * 40.0 + 2.0 * PI * 25.0, 0.02), "area {area}");
    let mut side = 0;
    for p in &parts {
        for (pos, n) in p.positions.iter().zip(&p.normals) {
            if n[2].abs() < 0.5 {
                // Side wall: the normal must point straight away from the axis.
                let r = (pos[0] * pos[0] + pos[1] * pos[1]).sqrt();
                let dot = (pos[0] * n[0] + pos[1] * n[1]) / r;
                // Exact surface normals: radial to within rounding, not just roughly.
                assert!(dot > 0.9999, "side normal {n:?} at {pos:?}");
                side += 1;
            }
        }
    }
    assert!(side > 8);
}

#[test]
fn brep_text_and_binary() {
    let text = load(&data("box.brep"), Kind::Brep, 0.001, 0.5).unwrap();
    assert!(close(measure(&text).0, 6000.0, 1e-6));
    let bin = load(&data("cylinder_bin.brep"), Kind::Brep, 0.0005, 0.2).unwrap();
    assert!(close(measure(&bin).0, PI * 1000.0, 0.02));
}

#[test]
fn finer_setting_gives_more_triangles() {
    let coarse = load(&data("assembly.step"), Kind::Step, 0.004, 0.6).unwrap();
    let fine = load(&data("assembly.step"), Kind::Step, 0.00025, 0.2).unwrap();
    let count = |p: &[Part]| p.iter().map(|x| x.indices.len() / 3).sum::<usize>();
    assert!(count(&fine) > count(&coarse) * 2, "{} vs {}", count(&fine), count(&coarse));
}

#[test]
fn bad_input_is_an_error_not_a_crash() {
    assert!(load(&data("does_not_exist.step"), Kind::Step, 0.001, 0.5).is_err());
    let junk = std::env::temp_dir().join("viewer3d_junk_test.step");
    std::fs::write(&junk, b"this is not a step file").unwrap();
    assert!(load(&junk, Kind::Step, 0.001, 0.5).is_err());
    assert!(load(&junk, Kind::Iges, 0.001, 0.5).is_err());
    assert!(load(&junk, Kind::Brep, 0.001, 0.5).is_err());
}
