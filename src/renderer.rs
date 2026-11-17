//! OpenGL renderer (via glow) used inside an egui paint callback.

use crate::model::Model;
use eframe::glow::{self, HasContext};
use glam::Mat4;
use std::collections::HashMap;

const VERT: &str = r#"#version 330 core
layout(location=0) in vec3 a_pos;
layout(location=1) in vec3 a_nrm;
layout(location=2) in vec3 a_col;
uniform mat4 u_mvp;
uniform mat4 u_view_model;
out vec3 v_pos;
out vec3 v_nrm;
out vec3 v_col;
void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
    v_pos = (u_view_model * vec4(a_pos, 1.0)).xyz;
    v_nrm = mat3(u_view_model) * a_nrm;
    v_col = a_col;
}
"#;

const FRAG: &str = r#"#version 330 core
in vec3 v_pos;
in vec3 v_nrm;
in vec3 v_col;
uniform int u_flat;
uniform int u_wire;
uniform vec3 u_tint;
uniform int u_shader;     // 0 standard, 1 clay, 2 toon, 3 matcap, 4 normals, 5 unlit
uniform int u_matcap;     // 0..5 procedural presets, 100 = custom image
uniform int u_matcap_tex;
uniform sampler2D u_tex;
uniform vec3 u_ldir;      // view-space direction towards the light
uniform vec3 u_lcol;      // light colour * intensity
uniform float u_ambient;
uniform float u_spec;
uniform float u_gloss;
uniform int u_override;
uniform vec3 u_ocol;
out vec4 frag;

vec3 env(vec3 r) {
    float h = r.y;
    vec3 sky = mix(vec3(0.75, 0.82, 0.95), vec3(1.0), smoothstep(0.0, 1.0, h));
    vec3 ground = mix(vec3(0.07, 0.07, 0.08), vec3(0.26, 0.23, 0.21), smoothstep(-1.0, 0.0, h));
    vec3 c = mix(ground, sky, smoothstep(-0.04, 0.06, h));
    c += vec3(2.0) * pow(max(dot(r, normalize(vec3(0.5, 0.7, 0.6))), 0.0), 60.0);
    return c;
}

vec3 matcap_proc(int id, vec3 n) {
    vec3 R = reflect(vec3(0.0, 0.0, -1.0), n);
    float nz = clamp(n.z, 0.0, 1.0);
    float fres = pow(1.0 - nz, 3.0);
    vec3 L = normalize(vec3(0.4, 0.6, 0.7));
    float d = max(dot(n, L), 0.0);
    float sp = max(dot(R, L), 0.0);
    if (id == 0) return vec3(0.78, 0.74, 0.70) * (0.25 + 0.85 * d) + vec3(0.08) * fres;
    if (id == 1) return env(R) * 0.95;
    if (id == 2) return env(R) * vec3(1.0, 0.78, 0.34) * 1.1;
    if (id == 3) return vec3(0.75, 0.08, 0.07) * (0.2 + 0.8 * d) + vec3(1.0) * pow(sp, 40.0) * 0.7 + vec3(0.3, 0.05, 0.05) * fres;
    if (id == 4) return vec3(0.18, 0.55, 0.38) * (0.25 + 0.8 * d) + vec3(0.9, 1.0, 0.95) * pow(sp, 24.0) * 0.35 + vec3(0.1, 0.3, 0.2) * fres;
    return vec3(0.93, 0.93, 0.97) * (0.35 + 0.7 * d) + vec3(0.6, 0.7, 1.0) * fres * 0.5 + vec3(1.0) * pow(sp, 80.0) * 0.5;
}

void main() {
    if (u_wire == 1) { frag = vec4(u_tint, 1.0); return; }
    vec3 n = (u_flat == 1) ? normalize(cross(dFdx(v_pos), dFdy(v_pos))) : normalize(v_nrm);
    n = faceforward(n, v_pos, n);
    vec3 base = (u_override == 1) ? u_ocol : v_col;
    vec3 V = normalize(-v_pos);
    vec3 L = normalize(u_ldir);
    float ndl = max(dot(n, L), 0.0);
    vec3 col;
    if (u_shader == 0) {
        vec3 H = normalize(L + V);
        float s = pow(max(dot(n, H), 0.0), u_gloss) * u_spec * step(0.0, dot(n, L));
        float amb = u_ambient * mix(0.6, 1.0, n.y * 0.5 + 0.5);
        // Soft fill from the camera, scaled by the Ambient slider, so faces turned away from
        // the main light keep their shape instead of going black.
        float fill = u_ambient * 0.9 * max(dot(n, V), 0.0);
        col = base * (vec3(amb + fill) + u_lcol * ndl) + u_lcol * s;
    } else if (u_shader == 1) {
        float w = pow(dot(n, L) * 0.5 + 0.5, 2.0);
        col = vec3(0.80) * (u_ambient * 0.8 + u_lcol * w * 0.95);
    } else if (u_shader == 2) {
        float band = ndl > 0.75 ? 1.0 : (ndl > 0.45 ? 0.75 : (ndl > 0.15 ? 0.52 : 0.32));
        col = base * band * max(u_lcol, vec3(0.35));
        if (dot(n, V) < 0.22) col = vec3(0.03);
    } else if (u_shader == 3) {
        if (u_matcap == 100 && u_matcap_tex == 1) {
            col = texture(u_tex, vec2(n.x * 0.5 + 0.5, 0.5 - n.y * 0.5)).rgb;
        } else {
            col = matcap_proc(u_matcap, n);
        }
    } else if (u_shader == 4) {
        col = n * 0.5 + 0.5;
    } else {
        col = base;
    }
    col = mix(col, u_tint, 0.35 * float(u_tint.x >= 0.0));
    frag = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

struct GpuMesh {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    count: i32,
}

pub struct Renderer {
    program: glow::Program,
    meshes: Vec<GpuMesh>,
    matcap_tex: Option<glow::Texture>,
}

/// Everything the shader needs to know about the "look" (light, shader, material override).
#[derive(Clone, Copy)]
pub struct Style {
    pub shader: i32,
    pub matcap: i32,
    pub matcap_tex: bool,
    pub light_dir: [f32; 3],
    pub light_color: [f32; 3],
    pub ambient: f32,
    pub spec: f32,
    pub gloss: f32,
    pub override_on: bool,
    pub override_col: [f32; 3],
}

#[derive(Clone)]
pub struct DrawParams {
    pub style: Style,
    pub mvp: Mat4,
    pub view_model: Mat4,
    pub flat: bool,
    /// 0 = solid, 1 = wireframe, 2 = solid + edges
    pub mode: u8,
    pub visible: Vec<bool>,
    pub selected: Option<usize>,
}

fn as_bytes<T>(v: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

impl Renderer {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        unsafe {
            let program = gl.create_program()?;
            let mut shaders = Vec::new();
            for (kind, src) in [(glow::VERTEX_SHADER, VERT), (glow::FRAGMENT_SHADER, FRAG)] {
                let sh = gl.create_shader(kind)?;
                gl.shader_source(sh, src);
                gl.compile_shader(sh);
                if !gl.get_shader_compile_status(sh) {
                    return Err(format!("Shader error: {}", gl.get_shader_info_log(sh)));
                }
                gl.attach_shader(program, sh);
                shaders.push(sh);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(format!("Program link error: {}", gl.get_program_info_log(program)));
            }
            for sh in shaders {
                gl.detach_shader(program, sh);
                gl.delete_shader(sh);
            }
            Ok(Renderer { program, meshes: Vec::new(), matcap_tex: None })
        }
    }

    /// Releases everything (used on exit).
    pub fn free_all(&mut self, gl: &glow::Context) {
        unsafe {
            if let Some(t) = self.matcap_tex.take() {
                gl.delete_texture(t);
            }
        }
        self.clear(gl);
    }

    pub fn clear(&mut self, gl: &glow::Context) {
        unsafe {
            for m in self.meshes.drain(..) {
                gl.delete_vertex_array(m.vao);
                gl.delete_buffer(m.vbo);
                gl.delete_buffer(m.ebo);
            }
        }
    }

    /// Uploads a custom matcap image (RGBA8).
    pub fn set_matcap(&mut self, gl: &glow::Context, rgba: &[u8], w: u32, h: u32) {
        unsafe {
            if let Some(t) = self.matcap_tex.take() {
                gl.delete_texture(t);
            }
            let tex = gl.create_texture().unwrap();
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                w as i32,
                h as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(rgba),
            );
            for (k, v) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, k, v as i32);
            }
            gl.bind_texture(glow::TEXTURE_2D, None);
            self.matcap_tex = Some(tex);
        }
    }

    pub fn upload(&mut self, gl: &glow::Context, model: &Model) {
        self.clear(gl);
        for mesh in &model.meshes {
            let normals = if mesh.normals.len() == mesh.positions.len() {
                mesh.normals.clone()
            } else {
                smooth_normals(&mesh.positions, &mesh.indices)
            };
            let mut data: Vec<f32> = Vec::with_capacity(mesh.positions.len() * 9);
            for (i, p) in mesh.positions.iter().enumerate() {
                data.extend_from_slice(p);
                data.extend_from_slice(&normals[i]);
                data.extend_from_slice(&mesh.color);
            }
            unsafe {
                let vao = gl.create_vertex_array().unwrap();
                let vbo = gl.create_buffer().unwrap();
                let ebo = gl.create_buffer().unwrap();
                gl.bind_vertex_array(Some(vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_bytes(&data), glow::STATIC_DRAW);
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
                gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, as_bytes(&mesh.indices), glow::STATIC_DRAW);
                let stride = 9 * 4;
                for (loc, off) in [(0u32, 0i32), (1, 12), (2, 24)] {
                    gl.enable_vertex_attrib_array(loc);
                    gl.vertex_attrib_pointer_f32(loc, 3, glow::FLOAT, false, stride, off);
                }
                gl.bind_vertex_array(None);
                self.meshes.push(GpuMesh { vao, vbo, ebo, count: mesh.indices.len() as i32 });
            }
        }
    }

    pub fn draw(&self, gl: &glow::Context, p: &DrawParams) {
        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.depth_mask(true);
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::BLEND);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.use_program(Some(self.program));

            let loc = |n: &str| gl.get_uniform_location(self.program, n);
            gl.uniform_matrix_4_f32_slice(loc("u_mvp").as_ref(), false, &p.mvp.to_cols_array());
            gl.uniform_matrix_4_f32_slice(loc("u_view_model").as_ref(), false, &p.view_model.to_cols_array());
            gl.uniform_1_i32(loc("u_flat").as_ref(), p.flat as i32);
            let st = &p.style;
            gl.uniform_1_i32(loc("u_shader").as_ref(), st.shader);
            gl.uniform_1_i32(loc("u_matcap").as_ref(), st.matcap);
            let use_tex = st.matcap_tex && self.matcap_tex.is_some();
            gl.uniform_1_i32(loc("u_matcap_tex").as_ref(), use_tex as i32);
            if use_tex {
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, self.matcap_tex);
                gl.uniform_1_i32(loc("u_tex").as_ref(), 0);
            }
            gl.uniform_3_f32_slice(loc("u_ldir").as_ref(), &st.light_dir);
            gl.uniform_3_f32_slice(loc("u_lcol").as_ref(), &st.light_color);
            gl.uniform_1_f32(loc("u_ambient").as_ref(), st.ambient);
            gl.uniform_1_f32(loc("u_spec").as_ref(), st.spec);
            gl.uniform_1_f32(loc("u_gloss").as_ref(), st.gloss);
            gl.uniform_1_i32(loc("u_override").as_ref(), st.override_on as i32);
            gl.uniform_3_f32_slice(loc("u_ocol").as_ref(), &st.override_col);
            let u_wire = loc("u_wire");
            let u_tint = loc("u_tint");

            let draw_all = |wire: bool| {
                for (i, m) in self.meshes.iter().enumerate() {
                    if !p.visible.get(i).copied().unwrap_or(true) {
                        continue;
                    }
                    gl.uniform_1_i32(u_wire.as_ref(), wire as i32);
                    let sel = p.selected == Some(i);
                    if wire {
                        let c = if sel { [1.0, 0.75, 0.1] } else { [0.05, 0.07, 0.10] };
                        gl.uniform_3_f32(u_tint.as_ref(), c[0], c[1], c[2]);
                    } else if sel {
                        gl.uniform_3_f32(u_tint.as_ref(), 1.0, 0.75, 0.1);
                    } else {
                        gl.uniform_3_f32(u_tint.as_ref(), -1.0, 0.0, 0.0);
                    }
                    gl.bind_vertex_array(Some(m.vao));
                    gl.draw_elements(glow::TRIANGLES, m.count, glow::UNSIGNED_INT, 0);
                }
            };

            match p.mode {
                1 => {
                    gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
                    draw_all(true);
                }
                2 => {
                    gl.enable(glow::POLYGON_OFFSET_FILL);
                    gl.polygon_offset(1.0, 1.0);
                    draw_all(false);
                    gl.disable(glow::POLYGON_OFFSET_FILL);
                    gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
                    draw_all(true);
                }
                _ => draw_all(false),
            }

            // Restore state for egui.
            gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            gl.bind_vertex_array(None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
        }
    }
}

/// Area-weighted smooth normals, welding vertices that share an exact position.
fn smooth_normals(pos: &[[f32; 3]], idx: &[u32]) -> Vec<[f32; 3]> {
    use glam::Vec3;
    let key = |p: &[f32; 3]| [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
    let mut group: HashMap<[u32; 3], usize> = HashMap::with_capacity(pos.len());
    let mut group_of = Vec::with_capacity(pos.len());
    for p in pos {
        let next = group.len();
        group_of.push(*group.entry(key(p)).or_insert(next));
    }
    let mut acc = vec![Vec3::ZERO; group.len()];
    for t in idx.chunks_exact(3) {
        let a = Vec3::from(pos[t[0] as usize]);
        let b = Vec3::from(pos[t[1] as usize]);
        let c = Vec3::from(pos[t[2] as usize]);
        let n = (b - a).cross(c - a);
        for &i in t {
            acc[group_of[i as usize]] += n;
        }
    }
    group_of
        .iter()
        .map(|&g| {
            let n = acc[g].normalize_or_zero();
            if n == Vec3::ZERO { [0.0, 1.0, 0.0] } else { n.to_array() }
        })
        .collect()
}
