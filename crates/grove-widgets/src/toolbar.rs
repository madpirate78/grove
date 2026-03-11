use egui::{Color32, CornerRadius, Frame, Margin, Response, Ui, Vec2, WidgetText};

/// A vertical toolbar button.
pub fn tool_button(ui: &mut Ui, label: impl Into<WidgetText>, active: bool) -> Response {
    tool_button_colored(ui, label, active, None)
}

/// A toolbar button with an optional custom highlight colour when active.
pub fn tool_button_colored(
    ui: &mut Ui,
    label: impl Into<WidgetText>,
    active: bool,
    active_colour: Option<Color32>,
) -> Response {
    let label = label.into();
    let fill = if active {
        active_colour.unwrap_or(Color32::from_rgb(60, 90, 140))
    } else {
        Color32::from_rgb(45, 45, 50)
    };
    let button = egui::Button::new(label)
        .fill(fill)
        .min_size(Vec2::new(48.0, 36.0));
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

/// Container frame for a two-column toolbar layout.
/// `col1` renders mode buttons (left), `col2` renders shape sub-tools (right).
pub fn toolbar_two_column(
    ui: &mut Ui,
    col1: impl FnOnce(&mut Ui),
    col2: impl FnOnce(&mut Ui),
) {
    Frame::new()
        .inner_margin(Margin::same(2))
        .corner_radius(CornerRadius::ZERO)
        .fill(Color32::from_rgb(30, 30, 35))
        .show(ui, |ui| {
            // Split available width into two equal columns with a small gap for separator
            let available = ui.available_width();
            let spacing = ui.spacing().item_spacing.x;
            let separator_width = 4.0;
            let col_width = ((available - separator_width - spacing * 2.0) / 2.0).max(48.0);

            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(col_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| { col1(ui); },
                );
                ui.separator();
                ui.allocate_ui_with_layout(
                    Vec2::new(col_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| { col2(ui); },
                );
            });
        });
}
