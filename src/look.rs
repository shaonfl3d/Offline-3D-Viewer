//! "Look" settings: shader, light, material override, background.

use crate::renderer::Style;
use eframe::egui;
use glam::{Mat4, Vec3};

pub const SHADERS: [&str; 6] = ["Standard", "Clay", "Toon", "Matcap", "Normals", "Unlit"];
pub const MATCAPS: [&str; 6] = ["Clay", "Chrome", "Gold", "Red ceramic", "Jade", "Pearl"];

pub struct Look {
    pub shader: u8,
    /// 0..MATCAPS.len()-1 = built-in presets, MATCAPS.len() = custom image
    pub matcap: usize,
    pub custom_name: Option<String>,
    pub request_matcap: bool,

    pub light_yaw: f32,   // degrees
    pub light_pitch: f32, // degrees
    pub light_intensity: f32,
    pub light_color: [f32; 3],
    pub ambient: f32,
    pub follow_camera: bool,

    pub spec: f32,
    pub gloss: f32,
    pub override_on: bool,
    pub override_col: [f32; 3],

    pub bg_custom: bool,
    pub bg: [f32; 3],
}

impl Look {
    pub fn new() -> Self {
        Look {
            shader: 0,
            matcap: 0,
            custom_name: None,
            request_matcap: false,
            light_yaw: 35.0,
            light_pitch: 40.0,
            light_intensity: 0.95,
            light_color: [1.0, 1.0, 1.0],
            ambient: 0.30,
            follow_camera: true,
            spec: 0.35,
            gloss: 48.0,
            override_on: false,
            override_col: [0.80, 0.80, 0.82],
            bg_custom: false,
            bg: [0.13, 0.145, 0.17],
        }
    }

    pub fn reset_light(&mut self) {
        let d = Look::new();
        self.light_yaw = d.light_yaw;
        self.light_pitch = d.light_pitch;
        self.light_intensity = d.light_intensity;
        self.light_color = d.light_color;
        self.ambient = d.ambient;
        self.follow_camera = d.follow_camera;
    }

    /// Shift + left-drag in the viewport.
    pub fn rotate_light(&mut self, dx: f32, dy: f32) {
        self.light_yaw = (self.light_yaw - dx * 0.5).rem_euclid(360.0);
        self.light_pitch = (self.light_pitch + dy * 0.5).clamp(-89.0, 89.0);
    }

    fn matcap_id(&self) -> (i32, bool) {
        if self.matcap >= MATCAPS.len() {
            (100, true)
        } else {
            (self.matcap as i32, false)
        }
    }

    /// Builds the shader parameters. `view` is the camera view matrix.
    pub fn style(&self, view: Mat4) -> Style {
        let (yaw, pitch) = (self.light_yaw.to_radians(), self.light_pitch.to_radians());
        let dir = Vec3::new(pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos());
        // Headlight mode: the light is fixed relative to the camera.
        // Otherwise it is fixed in the scene and turns with the model while you orbit.
        let view_dir = if self.follow_camera {
            dir
        } else {
            (glam::Mat3::from_mat4(view) * dir).normalize_or_zero()
        };
        let (matcap, tex) = self.matcap_id();
        Style {
            shader: self.shader as i32,
            matcap,
            matcap_tex: tex,
            light_dir: view_dir.to_array(),
            light_color: [
                self.light_color[0] * self.light_intensity,
                self.light_color[1] * self.light_intensity,
                self.light_color[2] * self.light_intensity,
            ],
            ambient: self.ambient,
            spec: self.spec,
            gloss: self.gloss,
            override_on: self.override_on,
            override_col: self.override_col,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Shader");
            egui::ComboBox::from_id_salt("look_shader")
                .selected_text(SHADERS[self.shader as usize])
                .show_ui(ui, |ui| {
                    for (i, n) in SHADERS.iter().enumerate() {
                        ui.selectable_value(&mut self.shader, i as u8, *n);
                    }
                });
        });

        if self.shader == 3 {
            ui.horizontal(|ui| {
                ui.label("Matcap");
                let current = if self.matcap >= MATCAPS.len() {
                    self.custom_name.clone().unwrap_or_else(|| "Custom image".into())
                } else {
                    MATCAPS[self.matcap].to_string()
                };
                egui::ComboBox::from_id_salt("look_matcap").selected_text(current).show_ui(ui, |ui| {
                    for (i, n) in MATCAPS.iter().enumerate() {
                        ui.selectable_value(&mut self.matcap, i, *n);
                    }
                    if let Some(name) = &self.custom_name {
                        ui.selectable_value(&mut self.matcap, MATCAPS.len(), name.as_str());
                    }
                });
            });
            if ui.button("Load matcap image…").on_hover_text("PNG or JPG sphere image").clicked() {
                self.request_matcap = true;
            }
        }

        if self.shader == 0 || self.shader == 1 || self.shader == 2 {
            ui.add_space(4.0);
            ui.strong("Light");
            ui.checkbox(&mut self.follow_camera, "Light follows camera");
            ui.add(egui::Slider::new(&mut self.light_yaw, 0.0..=360.0).text("Direction"));
            ui.add(egui::Slider::new(&mut self.light_pitch, -89.0..=89.0).text("Height"));
            ui.add(egui::Slider::new(&mut self.light_intensity, 0.0..=2.0).text("Intensity"));
            ui.add(egui::Slider::new(&mut self.ambient, 0.0..=1.0).text("Ambient"));
            ui.horizontal(|ui| {
                ui.label("Light colour");
                ui.color_edit_button_rgb(&mut self.light_color);
                if ui.small_button("Reset light").clicked() {
                    self.reset_light();
                }
            });
            ui.weak("Tip: Shift + left-drag in the view rotates the light.");
        }

        if self.shader == 0 {
            ui.add_space(4.0);
            ui.strong("Material");
            ui.checkbox(&mut self.override_on, "Override colour");
            if self.override_on {
                ui.horizontal(|ui| {
                    ui.label("Colour");
                    ui.color_edit_button_rgb(&mut self.override_col);
                });
            }
            ui.add(egui::Slider::new(&mut self.spec, 0.0..=1.5).text("Specular"));
            ui.add(egui::Slider::new(&mut self.gloss, 2.0..=256.0).logarithmic(true).text("Glossiness"));
        } else if self.shader == 2 || self.shader == 5 {
            ui.add_space(4.0);
            ui.checkbox(&mut self.override_on, "Override colour");
            if self.override_on {
                ui.color_edit_button_rgb(&mut self.override_col);
            }
        }

        ui.add_space(4.0);
        ui.strong("Background");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.bg_custom, "Custom");
            if self.bg_custom {
                ui.color_edit_button_rgb(&mut self.bg);
            }
        });
    }
}
