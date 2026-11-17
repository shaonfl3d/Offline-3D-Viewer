//! Modern look for the app UI (dark and light).

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, TextStyle};

pub const ACCENT: Color32 = Color32::from_rgb(79, 124, 255);

/// Uses the system Segoe UI font when available (Windows); otherwise egui's default font.
pub fn install_fonts(ctx: &egui::Context) {
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert("segoe".to_owned(), FontData::from_owned(bytes));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe".to_owned());
        }
        ctx.set_fonts(fonts);
    }
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let (panel, widget, hover, text, line, deep) = if dark {
        (
            Color32::from_rgb(24, 26, 32),
            Color32::from_rgb(40, 43, 52),
            Color32::from_rgb(55, 60, 74),
            Color32::from_rgb(226, 229, 236),
            Color32::from_rgb(48, 52, 62),
            Color32::from_rgb(16, 17, 22),
        )
    } else {
        (
            Color32::from_rgb(247, 248, 250),
            Color32::from_rgb(234, 237, 243),
            Color32::from_rgb(221, 226, 236),
            Color32::from_rgb(30, 33, 40),
            Color32::from_rgb(214, 218, 226),
            Color32::WHITE,
        )
    };
    let r = Rounding::same(8.0);
    v.panel_fill = panel;
    v.window_fill = panel;
    v.extreme_bg_color = deep;
    v.faint_bg_color = widget;
    v.window_rounding = Rounding::same(12.0);
    v.menu_rounding = Rounding::same(10.0);
    v.window_stroke = Stroke::new(1.0_f32, line);

    v.widgets.noninteractive.bg_fill = panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, text);
    v.widgets.noninteractive.rounding = r;

    v.widgets.inactive.bg_fill = widget;
    v.widgets.inactive.weak_bg_fill = widget;
    // A thin outline keeps empty checkboxes and quiet buttons visible in both themes.
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, line);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, text);
    v.widgets.inactive.rounding = r;

    v.widgets.hovered.bg_fill = hover;
    v.widgets.hovered.weak_bg_fill = hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, text);
    v.widgets.hovered.rounding = r;

    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.weak_bg_fill = ACCENT;
    v.widgets.active.bg_stroke = Stroke::NONE;
    // egui draws "strong" text (section titles) in this colour, so it must contrast with the panel.
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, if dark { Color32::WHITE } else { Color32::from_rgb(12, 14, 20) });
    v.widgets.active.rounding = r;

    v.widgets.open.bg_fill = hover;
    v.widgets.open.weak_bg_fill = hover;
    v.widgets.open.rounding = r;

    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);

    ctx.style_mut(|s| {
        s.visuals = v;
        s.spacing.item_spacing = egui::vec2(8.0, 7.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 30.0;
        s.spacing.slider_width = 150.0;
        s.text_styles.insert(TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Body, FontId::new(14.5, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(14.5, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Small, FontId::new(12.0, FontFamily::Proportional));
    });
}
