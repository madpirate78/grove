use egui::{Color32, FontFamily, FontId, Style, TextStyle, Visuals};

/// Colours for the Grove dark theme.
pub struct ThemeColours;

impl ThemeColours {
    pub const BG_DARKEST: Color32 = Color32::from_rgb(22, 22, 26);
    pub const BG_DARK: Color32 = Color32::from_rgb(30, 30, 35);
    pub const BG_MID: Color32 = Color32::from_rgb(40, 40, 46);
    pub const BG_PANEL: Color32 = Color32::from_rgb(35, 35, 40);
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 220, 225);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(150, 150, 158);
    pub const TEXT_DIM: Color32 = Color32::from_rgb(100, 100, 108);
    pub const ACCENT: Color32 = Color32::from_rgb(70, 130, 210);
    pub const GRID_MAJOR: Color32 = Color32::from_rgba_premultiplied(60, 60, 65, 80);
    pub const GRID_MINOR: Color32 = Color32::from_rgba_premultiplied(45, 45, 50, 60);
    pub const SELECTION: Color32 = Color32::from_rgb(70, 160, 255);
    pub const SNAP_INDICATOR: Color32 = Color32::from_rgb(255, 200, 50);
    pub const CANVAS_BG: Color32 = Color32::from_rgb(28, 28, 32);
    pub const STATUS_BAR: Color32 = Color32::from_rgb(25, 25, 30);
}

/// Apply the Grove dark theme to egui.
pub fn apply_theme(ctx: &egui::Context) {
    let mut style = Style {
        visuals: Visuals::dark(),
        ..Style::default()
    };
    style.visuals.panel_fill = ThemeColours::BG_DARK;
    style.visuals.window_fill = ThemeColours::BG_PANEL;
    style.visuals.extreme_bg_color = ThemeColours::BG_DARKEST;
    style.visuals.faint_bg_color = ThemeColours::BG_MID;
    style.visuals.override_text_color = Some(ThemeColours::TEXT_PRIMARY);

    // Widget rounding
    style.visuals.widgets.noninteractive.corner_radius = 3.0.into();
    style.visuals.widgets.inactive.corner_radius = 3.0.into();
    style.visuals.widgets.hovered.corner_radius = 3.0.into();
    style.visuals.widgets.active.corner_radius = 3.0.into();

    // Accent colour for selections — subtle so text remains readable
    style.visuals.selection.bg_fill = Color32::from_rgba_premultiplied(70, 130, 210, 60);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, ThemeColours::ACCENT);

    // Spacing
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(8);

    // Font sizes
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(16.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(12.0, FontFamily::Monospace),
    );

    ctx.set_global_style(style);
}
