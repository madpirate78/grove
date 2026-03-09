mod app;
mod panels;
mod theme;
mod tools;
mod view;

use app::GroveApp;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Grove — Garden Design")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Grove",
        options,
        Box::new(|cc| Ok(Box::new(GroveApp::new(cc)))),
    )
}
