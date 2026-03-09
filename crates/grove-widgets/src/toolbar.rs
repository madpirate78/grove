use egui::{Color32, CornerRadius, Frame, Margin, Response, Ui, Vec2, WidgetText};

/// A vertical toolbar button.
pub fn tool_button(ui: &mut Ui, label: impl Into<WidgetText>, active: bool) -> Response {
    let label = label.into();
    let button = if active {
        egui::Button::new(label)
            .fill(Color32::from_rgb(60, 90, 140))
            .min_size(Vec2::new(48.0, 36.0))
    } else {
        egui::Button::new(label)
            .fill(Color32::from_rgb(45, 45, 50))
            .min_size(Vec2::new(48.0, 36.0))
    };
    ui.add(button)
}

/// A toolbar separator line.
pub fn tool_separator(ui: &mut Ui) {
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);
}

/// Container frame for a vertical toolbar.
pub fn toolbar_frame(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    Frame::new()
        .inner_margin(Margin::same(4))
        .corner_radius(CornerRadius::ZERO)
        .fill(Color32::from_rgb(30, 30, 35))
        .show(ui, |ui| {
            ui.vertical(add_contents);
        });
}
