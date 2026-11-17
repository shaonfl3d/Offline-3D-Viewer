#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod loaders;
mod look;
mod model;
mod renderer;
mod theme;

use eframe::egui::{self, Color32, Key, PointerButton, Sense};
use eframe::egui_glow;
use glam::{Mat4, Vec3};
use model::{mesh_stats, Model, Stats};
use look::{Look, SHADERS};
use renderer::{DrawParams, Renderer};
use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    time::Instant,
};

const FOV_Y: f32 = 45.0_f32 * std::f32::consts::PI / 180.0;

struct Camera {
    target: Vec3,
    yaw: f32,
    pitch: f32,
    dist: f32,
    radius: f32,
}

impl Camera {
    fn new() -> Self {
        Camera { target: Vec3::ZERO, yaw: 0.6, pitch: 0.4, dist: 5.0, radius: 1.0 }
    }
    fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.target + self.dist * Vec3::new(cp * sy, sp, cp * cy)
    }
    fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }
}

type LoadResult = (PathBuf, Result<Model, String>, u128);

struct App {
    renderer: Option<Arc<Mutex<Renderer>>>,
    gl_error: Option<String>,
    model: Option<Model>,
    stats: Stats,
    mesh_stats: Vec<Stats>,
    path: Option<PathBuf>,
    file_size: u64,
    load_ms: u128,
    error: Option<String>,
    rx: Option<mpsc::Receiver<LoadResult>>,
    loading: bool,
    cam: Camera,
    visible: Vec<bool>,
    selected: Option<usize>,
    mode: u8,
    flat: bool,
    z_up: bool,
    /// Z-up was switched on automatically for a CAD file (so it is switched off again for the next mesh file).
    z_up_auto: bool,
    dark: bool,
    view_rect: egui::Rect,
    want_screenshot: bool,
    look: Look,
    want_close: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
        let (renderer, gl_error) = match cc.gl.as_ref() {
            Some(gl) => match Renderer::new(gl) {
                Ok(r) => (Some(Arc::new(Mutex::new(r))), None),
                Err(e) => (None, Some(e)),
            },
            None => (None, Some("OpenGL is not available on this system.".to_string())),
        };
        theme::install_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, true);
        let mut app = App {
            renderer,
            gl_error,
            model: None,
            stats: Stats::default(),
            mesh_stats: vec![],
            path: None,
            file_size: 0,
            load_ms: 0,
            error: None,
            rx: None,
            loading: false,
            cam: Camera::new(),
            visible: vec![],
            selected: None,
            mode: 0,
            flat: false,
            z_up: false,
            z_up_auto: false,
            dark: true,
            view_rect: egui::Rect::NOTHING,
            want_screenshot: false,
            look: Look::new(),
            want_close: false,
        };
        if let Some(p) = initial {
            app.start_load(&cc.egui_ctx, p);
        }
        app
    }

    fn start_load(&mut self, ctx: &egui::Context, path: PathBuf) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.loading = true;
        self.error = None;
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            let p2 = path.clone();
            let result = std::panic::catch_unwind(move || loaders::load(&p2))
                .unwrap_or_else(|_| Err("The file parser crashed. The file may be corrupt.".to_string()));
            let _ = tx.send((path, result, t.elapsed().as_millis()));
            ctx2.request_repaint();
        });
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        let mut exts: Vec<&str> = loaders::SUPPORTED.to_vec();
        exts.extend_from_slice(loaders::PLANNED);
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("3D models", &exts)
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.start_load(ctx, p);
        }
    }

    fn load_matcap(&mut self, frame: &eframe::Frame) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Matcap image", &["png", "jpg", "jpeg"])
            .pick_file()
        else {
            return;
        };
        match image::open(&path) {
            Ok(img) => {
                let img = img.to_rgba8();
                let (w, h) = img.dimensions();
                // Keep textures small and square-ish friendly.
                let img = if w.max(h) > 1024 {
                    let k = 1024.0 / w.max(h) as f32;
                    image::imageops::resize(
                        &img,
                        ((w as f32 * k) as u32).max(1),
                        ((h as f32 * k) as u32).max(1),
                        image::imageops::FilterType::Triangle,
                    )
                } else {
                    img
                };
                let (w, h) = img.dimensions();
                if let (Some(gl), Some(r)) = (frame.gl(), self.renderer.as_ref()) {
                    r.lock().unwrap().set_matcap(gl, img.as_raw(), w, h);
                    let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("custom").to_string();
                    self.look.custom_name = Some(name);
                    self.look.matcap = look::MATCAPS.len();
                    self.look.shader = 3;
                }
            }
            Err(e) => self.error = Some(format!("Could not load matcap image: {e}")),
        }
    }

    /// Returns to the home page: frees the model and its GPU buffers.
    fn close_model(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if let (Some(gl), Some(r)) = (frame.gl(), self.renderer.as_ref()) {
            if let Ok(mut r) = r.lock() {
                r.clear(gl);
            }
        }
        self.model = None;
        self.stats = Stats::default();
        self.mesh_stats.clear();
        self.visible.clear();
        self.selected = None;
        self.path = None;
        self.error = None;
        self.loading = false;
        self.rx = None;
        self.file_size = 0;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("3D Viewer".to_string()));
    }

    fn model_matrix(&self) -> Mat4 {
        if self.z_up {
            Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
        } else {
            Mat4::IDENTITY
        }
    }

    fn fit(&mut self) {
        let Some(model) = self.model.as_ref() else { return };
        // Frame what is actually shown; hidden meshes are ignored unless everything is hidden.
        let mut shown: Option<model::Bounds> = None;
        for (i, m) in model.meshes.iter().enumerate() {
            if self.visible.get(i).copied().unwrap_or(true) {
                if let Some(b) = model::mesh_bounds(m) {
                    shown = Some(match shown {
                        None => b,
                        Some(a) => model::Bounds { min: a.min.min(b.min), max: a.max.max(b.max) },
                    });
                }
            }
        }
        let Some(b) = shown.or_else(|| model.bounds()) else { return };
        let mm = self.model_matrix();
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for i in 0..8 {
            let c = Vec3::new(
                if i & 1 == 0 { b.min.x } else { b.max.x },
                if i & 2 == 0 { b.min.y } else { b.max.y },
                if i & 4 == 0 { b.min.z } else { b.max.z },
            );
            let w = mm.transform_point3(c);
            min = min.min(w);
            max = max.max(w);
        }
        self.cam.target = (min + max) * 0.5;
        self.cam.radius = ((max - min).length() * 0.5).max(1e-6);
        self.cam.dist = self.cam.radius / (FOV_Y * 0.5).sin() * 1.15;
    }

    fn finish_load(&mut self, ctx: &egui::Context, frame: &eframe::Frame, path: PathBuf, model: Model, ms: u128) {
        if let (Some(gl), Some(r)) = (frame.gl(), self.renderer.as_ref()) {
            r.lock().unwrap().upload(gl, &model);
        }
        self.stats = model.stats();
        self.mesh_stats = model.meshes.iter().map(mesh_stats).collect();
        self.visible = model.meshes.iter().map(|m| !m.hidden).collect();
        // CAD files (STEP, IGES, BREP, FreeCAD) are Z-up by convention: stand them upright.
        if loaders::is_cad(&path) {
            if !self.z_up {
                self.z_up = true;
                self.z_up_auto = true;
            }
        } else if self.z_up_auto {
            self.z_up = false;
            self.z_up_auto = false;
        }
        self.selected = None;
        // Meshes without stored normals look better flat-shaded (typical for STL/CAD).
        self.flat = model.meshes.iter().any(|m| m.normals.is_empty());
        self.file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        self.load_ms = ms;
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("model").to_string();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{name} - 3D Viewer")));
        self.path = Some(path);
        self.model = Some(model);
        self.fit();
    }

    fn poll_loader(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(rx) = &self.rx else { return };
        if let Ok((path, result, ms)) = rx.try_recv() {
            self.rx = None;
            self.loading = false;
            match result {
                Ok(m) => self.finish_load(ctx, frame, path, m, ms),
                Err(e) => self.error = Some(format!("{}: {e}", path.display())),
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(p) = dropped.into_iter().find_map(|f| f.path) {
            self.start_load(ctx, p);
        }
        let (open, fit, wire) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(Key::O),
                i.key_pressed(Key::F) && !i.modifiers.command,
                i.key_pressed(Key::W) && !i.modifiers.command,
            )
        });
        if open {
            self.open_dialog(ctx);
        }
        let close = ctx.input(|i| i.key_pressed(Key::Escape) || (i.modifiers.command && i.key_pressed(Key::W)));
        if close && self.model.is_some() && !ctx.wants_keyboard_input() {
            self.want_close = true;
        }
        if fit && !ctx.wants_keyboard_input() {
            self.fit();
        }
        if wire && !ctx.wants_keyboard_input() {
            self.mode = (self.mode + 1) % 3;
        }
        // Screenshot result
        let shots: Vec<_> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
                .collect()
        });
        for img in shots {
            let ppp = ctx.pixels_per_point();
            let region = img.region(&self.view_rect, Some(ppp));
            let rgba: Vec<u8> = region.pixels.iter().flat_map(|c| c.to_array()).collect();
            let stem = self
                .path
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("model");
            if let Some(out) = rfd::FileDialog::new()
                .set_file_name(format!("{stem}.png"))
                .add_filter("PNG image", &["png"])
                .save_file()
            {
                if let Err(e) = image::save_buffer(
                    &out,
                    &rgba,
                    region.width() as u32,
                    region.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    self.error = Some(format!("Could not save screenshot: {e}"));
                }
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            let open_btn = egui::Button::new(egui::RichText::new("Open…").color(Color32::WHITE).strong())
                .fill(theme::ACCENT);
            if ui.add(open_btn).on_hover_text("Ctrl+O").clicked() {
                self.open_dialog(ctx);
            }
            if ui
                .add_enabled(self.model.is_some(), egui::Button::new("Close model"))
                .on_hover_text("Back to the home page (Esc or Ctrl+W)")
                .clicked()
            {
                self.want_close = true;
            }
            ui.separator();
            let has = self.model.is_some();
            ui.add_enabled_ui(has, |ui| {
                if ui.button("Fit").on_hover_text("F or double-click the view").clicked() {
                    self.fit();
                }
                ui.selectable_value(&mut self.mode, 0, "Solid");
                ui.selectable_value(&mut self.mode, 2, "Solid + edges");
                ui.selectable_value(&mut self.mode, 1, "Wireframe");
                ui.separator();
                egui::ComboBox::from_id_salt("toolbar_shader")
                    .selected_text(format!("Shader: {}", SHADERS[self.look.shader as usize]))
                    .show_ui(ui, |ui| {
                        for (i, n) in SHADERS.iter().enumerate() {
                            ui.selectable_value(&mut self.look.shader, i as u8, *n);
                        }
                    });
                #[cfg(any(feature = "cad", feature = "cad-system"))]
                {
                    let names = ["CAD detail: Draft", "CAD detail: Normal", "CAD detail: Fine"];
                    let mut q = loaders::cad::quality();
                    egui::ComboBox::from_id_salt("toolbar_cadq")
                        .selected_text(names[q as usize])
                        .show_ui(ui, |ui| {
                            for (i, n) in names.iter().enumerate() {
                                ui.selectable_value(&mut q, i as u8, *n);
                            }
                        });
                    if q != loaders::cad::quality() {
                        loaders::cad::set_quality(q);
                        if let Some(p) = self.path.clone() {
                            if loaders::is_cad(&p) {
                                self.start_load(ctx, p);
                            }
                        }
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.flat, "Flat shading");
                if ui.checkbox(&mut self.z_up, "Z-up model").changed() {
                    self.z_up_auto = false;
                    self.fit();
                }
                ui.separator();
                if ui.button("Save image").clicked() {
                    self.want_screenshot = true;
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if self.dark { "Light theme" } else { "Dark theme" };
                if ui.button(label).clicked() {
                    self.dark = !self.dark;
                    theme::apply(ctx, self.dark);
                }
            });
        });
    }

    fn mesh_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Meshes");
        let Some(model) = &self.model else {
            ui.weak("No model loaded");
            return;
        };
        ui.horizontal(|ui| {
            if ui.small_button("Show all").clicked() {
                self.visible.iter_mut().for_each(|v| *v = true);
            }
            if ui.small_button("Hide all").clicked() {
                self.visible.iter_mut().for_each(|v| *v = false);
            }
            if ui.small_button("Deselect").clicked() {
                self.selected = None;
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, m) in model.meshes.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.visible[i], "");
                    let sel = self.selected == Some(i);
                    if ui.selectable_label(sel, &m.name).clicked() {
                        self.selected = if sel { None } else { Some(i) };
                    }
                });
            }
        });
    }

    fn details_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Details");
        ui.separator();
        let Some(model) = &self.model else {
            ui.weak("Open a file to see its details.");
            return;
        };
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::CollapsingHeader::new("Shader & Lighting")
                .default_open(true)
                .show(ui, |ui| self.look.ui(ui));
            ui.add_space(6.0);
            ui.separator();
            let stat_grid = |ui: &mut egui::Ui, id: &str, s: &Stats| {
                egui::Grid::new(id).num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                    ui.label("Vertices:");
                    ui.label(fmt_int(s.vertices));
                    ui.end_row();
                    ui.label("Triangles:");
                    ui.label(fmt_int(s.triangles));
                    ui.end_row();
                    ui.label("Size X:");
                    ui.label(fmt_f(s.size[0] as f64));
                    ui.end_row();
                    ui.label("Size Y:");
                    ui.label(fmt_f(s.size[1] as f64));
                    ui.end_row();
                    ui.label("Size Z:");
                    ui.label(fmt_f(s.size[2] as f64));
                    ui.end_row();
                    ui.label("Volume:");
                    ui.label(fmt_f(s.volume.abs()));
                    ui.end_row();
                    ui.label("Surface:");
                    ui.label(fmt_f(s.surface));
                    ui.end_row();
                });
            };
            stat_grid(ui, "total", &self.stats);
            ui.weak("Volume is only meaningful for closed (watertight) meshes.");

            if let Some(i) = self.selected {
                ui.add_space(8.0);
                ui.strong(format!("Selected: {}", model.meshes[i].name));
                stat_grid(ui, "sel", &self.mesh_stats[i]);
                if let Some((mn, mx)) = self.mesh_stats[i].bounds {
                    ui.weak(format!(
                        "Min ({:.2}, {:.2}, {:.2})\nMax ({:.2}, {:.2}, {:.2})",
                        mn.x, mn.y, mn.z, mx.x, mx.y, mx.z
                    ));
                }
            }

            ui.add_space(10.0);
            ui.strong("File");
            egui::Grid::new("file").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                if let Some(p) = &self.path {
                    ui.label("Name:");
                    ui.label(p.file_name().and_then(|n| n.to_str()).unwrap_or(""));
                    ui.end_row();
                }
                ui.label("Format:");
                ui.label(&model.format);
                ui.end_row();
                ui.label("File size:");
                ui.label(fmt_bytes(self.file_size));
                ui.end_row();
                ui.label("Meshes:");
                ui.label(fmt_int(model.meshes.len()));
                ui.end_row();
                ui.label("Load time:");
                ui.label(format!("{} ms", self.load_ms));
                ui.end_row();
            });
            if let Some(p) = &self.path {
                ui.weak(p.display().to_string());
            }

            ui.add_space(10.0);
            ui.strong(format!("Materials ({})", model.materials.len()));
            for mat in &model.materials {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
                    let c = mat.color;
                    ui.painter().rect_filled(
                        r,
                        3.0,
                        Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8),
                    );
                    ui.label(&mat.name);
                });
            }
        });
    }

    fn viewport(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.available_rect_before_wrap();
        self.view_rect = rect;
        let bg = if self.look.bg_custom {
            let c = self.look.bg;
            Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)
        } else if self.dark {
            Color32::from_rgb(34, 37, 43)
        } else {
            Color32::from_rgb(244, 245, 247)
        };
        ui.painter().rect_filled(rect, 0.0, bg);
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());

        if self.model.is_none() {
            self.empty_state(ui, rect, ctx);
            return;
        }
        let Some(renderer) = self.renderer.clone() else { return };

        // Camera interaction
        let d = resp.drag_delta();
        let shift = ctx.input(|i| i.modifiers.shift);
        if resp.dragged_by(PointerButton::Primary) && shift {
            self.look.rotate_light(d.x, d.y);
        } else if resp.dragged_by(PointerButton::Primary) {
            self.cam.yaw -= d.x * 0.008;
            self.cam.pitch = (self.cam.pitch + d.y * 0.008).clamp(-1.55, 1.55);
        } else if resp.dragged_by(PointerButton::Secondary) || resp.dragged_by(PointerButton::Middle) {
            let inv = self.cam.view().inverse();
            let right = inv.x_axis.truncate();
            let up = inv.y_axis.truncate();
            let k = self.cam.dist * 2.0 * (FOV_Y * 0.5).tan() / rect.height().max(1.0);
            self.cam.target += (-right * d.x + up * d.y) * k;
        }
        if resp.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.cam.dist = (self.cam.dist * (1.0 - scroll * 0.0016)).clamp(self.cam.radius * 0.02, self.cam.radius * 80.0);
            }
        }
        if resp.double_clicked() {
            self.fit();
        }

        let aspect = (rect.width() / rect.height().max(1.0)).max(0.01);
        let r = self.cam.radius;
        let near = (self.cam.dist - r * 1.5).max(r * 0.01);
        let far = self.cam.dist + r * 3.0;
        let proj = Mat4::perspective_rh_gl(FOV_Y, aspect, near, far);
        let view = self.cam.view();
        let model_m = self.model_matrix();
        let params = DrawParams {
            mvp: proj * view * model_m,
            view_model: view * model_m,
            flat: self.flat,
            mode: self.mode,
            visible: self.visible.clone(),
            selected: self.selected,
            style: self.look.style(view),
        };
        let cb = egui_glow::CallbackFn::new(move |_info, painter| {
            if let Ok(r) = renderer.lock() {
                r.draw(painter.gl(), &params);
            }
        });
        ui.painter().add(egui::PaintCallback { rect, callback: Arc::new(cb) });

        // Hint text
        ui.painter().text(
            rect.left_bottom() + egui::vec2(10.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            "Left-drag: orbit   Shift+drag: light   Right/Middle-drag: pan   Wheel: zoom   Double-click: fit",
            egui::FontId::proportional(12.0),
            if self.dark { Color32::from_gray(140) } else { Color32::from_gray(110) },
        );
        if self.want_screenshot {
            self.want_screenshot = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
    }

    fn empty_state(&mut self, ui: &mut egui::Ui, rect: egui::Rect, ctx: &egui::Context) {
        let dark = self.dark;
        let muted = if dark { Color32::from_gray(140) } else { Color32::from_gray(105) };
        let strong = if dark { Color32::from_gray(232) } else { Color32::from_gray(30) };
        ui.painter().text(
            rect.center_bottom() + egui::vec2(0.0, -16.0),
            egui::Align2::CENTER_BOTTOM,
            "Developed by Azizul Haque Shaon",
            egui::FontId::proportional(13.0),
            muted,
        );

        let card_w = 580.0_f32.min((rect.width() - 48.0).max(260.0));
        let card_h = 420.0_f32.min((rect.height() - 70.0).max(260.0));
        let card = egui::Rect::from_center_size(rect.center() + egui::vec2(0.0, -12.0), egui::vec2(card_w, card_h));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(card));
        child.vertical_centered(|ui| {
            ui.label(egui::RichText::new("3D Viewer").size(42.0).strong().color(theme::ACCENT));
            ui.label(egui::RichText::new("Open any 3D model. Fully offline.").size(16.0).color(muted));
            ui.add_space(18.0);

            // Drop zone
            let (zone, _) = ui.allocate_exact_size(egui::vec2(card_w, 150.0), Sense::hover());
            let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
            let border = if hovering { theme::ACCENT } else if dark { Color32::from_gray(70) } else { Color32::from_gray(190) };
            let fill = if dark { Color32::from_rgb(30, 33, 41) } else { Color32::WHITE };
            ui.painter().rect(zone, 16.0, fill, egui::Stroke::new(1.6_f32, border));
            if self.loading {
                ui.painter().text(
                    zone.center(),
                    egui::Align2::CENTER_CENTER,
                    "Loading…",
                    egui::FontId::proportional(20.0),
                    strong,
                );
            } else {
                ui.painter().text(
                    zone.center() + egui::vec2(0.0, -30.0),
                    egui::Align2::CENTER_CENTER,
                    "Drag and drop a 3D file here",
                    egui::FontId::proportional(20.0),
                    strong,
                );
                ui.painter().text(
                    zone.center() + egui::vec2(0.0, -4.0),
                    egui::Align2::CENTER_CENTER,
                    "or",
                    egui::FontId::proportional(14.0),
                    muted,
                );
                let btn = egui::Button::new(egui::RichText::new("Open file…").color(Color32::WHITE).strong())
                    .fill(theme::ACCENT)
                    .min_size(egui::vec2(150.0, 34.0));
                let r = ui.put(egui::Rect::from_center_size(zone.center() + egui::vec2(0.0, 38.0), egui::vec2(150.0, 34.0)), btn);
                if r.clicked() {
                    self.open_dialog(ctx);
                }
            }

            ui.add_space(20.0);
            ui.label(egui::RichText::new("SUPPORTED FORMATS").size(11.5).color(muted));
            ui.label(
                egui::RichText::new(loaders::SUPPORTED.iter().map(|e| e.to_uppercase()).collect::<Vec<_>>().join("   "))
                    .size(13.5)
                    .color(strong),
            );
            ui.add_space(4.0);
            if loaders::CAD_ENABLED {
                ui.label(egui::RichText::new("Coming next: IFC   3DM").size(12.5).color(muted));
            } else {
                ui.label(
                    egui::RichText::new("STEP, IGES, BREP, FCSTD need the CAD build.  Coming later: IFC, 3DM")
                        .size(12.5)
                        .color(muted),
                );
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.poll_loader(ctx, frame);
        if self.look.request_matcap {
            self.look.request_matcap = false;
            self.load_matcap(frame);
        }
        self.handle_input(ctx);
        if self.want_close {
            self.want_close = false;
            self.close_model(ctx, frame);
        }

        let panel_fill = ctx.style().visuals.panel_fill;
        let edge = ctx.style().visuals.widgets.noninteractive.bg_stroke;
        let bar = |h: f32, v: f32| {
            egui::Frame::none()
                .fill(panel_fill)
                .stroke(egui::Stroke::new(0.0_f32, edge.color))
                .inner_margin(egui::Margin::symmetric(h, v))
        };
        egui::TopBottomPanel::top("toolbar").frame(bar(12.0, 8.0)).show(ctx, |ui| {
            self.toolbar(ui, ctx);
        });
        if self.model.is_some() {
            egui::SidePanel::left("meshes")
                .default_width(230.0)
                .frame(bar(12.0, 12.0))
                .show(ctx, |ui| self.mesh_panel(ui));
            egui::SidePanel::right("details")
                .default_width(280.0)
                .frame(bar(12.0, 12.0))
                .show(ctx, |ui| self.details_panel(ui));
        }
        egui::TopBottomPanel::bottom("status").frame(bar(14.0, 6.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.loading {
                    ui.spinner();
                    ui.label("Loading…");
                } else if let Some(e) = &self.error {
                    ui.colored_label(Color32::from_rgb(235, 100, 100), e);
                } else if let Some(e) = &self.gl_error {
                    ui.colored_label(Color32::from_rgb(235, 100, 100), e);
                } else if let Some(p) = &self.path {
                    ui.weak(p.display().to_string());
                } else {
                    ui.weak("Ready. Works fully offline.");
                }
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| self.viewport(ui, ctx));

        if !ctx.input(|i| i.raw.hovered_files.is_empty()) {
            let r = ctx.screen_rect();
            let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop")));
            p.rect_filled(r, 0.0, Color32::from_black_alpha(160));
            p.text(r.center(), egui::Align2::CENTER_CENTER, "Drop to open", egui::FontId::proportional(32.0), Color32::WHITE);
        }
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let (Some(gl), Some(r)) = (gl, self.renderer.as_ref()) {
            if let Ok(mut r) = r.lock() {
                r.free_all(gl);
            }
        }
    }
}

fn fmt_int(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn fmt_f(v: f64) -> String {
    let s = format!("{:.2}", v);
    let (int, frac) = s.split_once('.').unwrap_or((&s, "00"));
    let neg = int.starts_with('-');
    let digits = int.trim_start_matches('-');
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{}{}.{}", if neg { "-" } else { "" }, out, frac)
}

fn fmt_bytes(b: u64) -> String {
    const U: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < 3 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{b} B") } else { format!("{v:.1} {}", U[i]) }
}

fn load_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/icon.png")).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width: w, height: h })
}

fn main() -> eframe::Result<()> {
    // "Open with" / double-click passes the file path as the first argument.
    let initial = std::env::args_os().nth(1).map(PathBuf::from).filter(|p| Path::new(p).is_file());
    // Some graphics drivers (virtual machines, remote desktop, old GPUs) have no anti-aliased
    // display mode. If start-up fails, the app restarts itself once without anti-aliasing.
    let no_msaa = std::env::var_os("VIEWER3D_NO_MSAA").is_some();
    let mut vb = egui::ViewportBuilder::default()
        .with_inner_size([1360.0, 820.0])
        .with_min_inner_size([760.0, 480.0])
        .with_title("3D Viewer")
        .with_app_id("viewer3d")
        .with_drag_and_drop(true);
    if let Some(icon) = load_icon() {
        vb = vb.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport: vb,
        depth_buffer: 24,
        multisampling: if no_msaa { 0 } else { 4 },
        ..Default::default()
    };
    let result = eframe::run_native("3D Viewer", options, Box::new(move |cc| Ok(Box::new(App::new(cc, initial)))));
    if result.is_err() && !no_msaa {
        if let Ok(exe) = std::env::current_exe() {
            let retry = std::process::Command::new(exe)
                .args(std::env::args_os().skip(1))
                .env("VIEWER3D_NO_MSAA", "1")
                .spawn();
            if retry.is_ok() {
                return Ok(());
            }
        }
    }
    result
}
