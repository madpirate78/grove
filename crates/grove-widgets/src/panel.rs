use egui::{Color32, CornerRadius, Frame, Margin, Ui};

/// A styled panel with consistent appearance.
pub struct Panel {
    pub title: String,
}

impl Panel {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }

    pub fn show(&self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
        Frame::new()
            .inner_margin(Margin::same(8))
            .corner_radius(CornerRadius::same(4))
            .fill(Color32::from_rgb(35, 35, 40))
            .show(ui, |ui| {
                if !self.title.is_empty() {
                    ui.heading(&self.title);
                    ui.separator();
                }
                add_contents(ui);
            });
    }
}
