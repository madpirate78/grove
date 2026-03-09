use egui::Ui;
use grove_core::drawing::drawing::Drawing;
use grove_core::drawing::templates::DrawingTemplate;
use grove_core::project::Project;
use uuid::Uuid;

pub enum DrawingManagerAction {
    OpenDrawing(Uuid),
    CreateFromTemplate(DrawingTemplate),
    RemoveDrawing(Uuid),
}

pub fn draw_drawing_manager(
    ui: &mut Ui,
    project: &Project,
) -> Option<DrawingManagerAction> {
    let mut action = None;

    ui.heading("Drawings");
    ui.separator();

    if project.drawings.is_empty() {
        ui.label("No drawings yet.");
    } else {
        for drawing in &project.drawings {
            ui.horizontal(|ui| {
                let label = format!(
                    "{} — {} ({})",
                    drawing.full_sheet_ref(),
                    drawing.title,
                    format_paper_size(drawing),
                );
                if ui.button(&label).clicked() {
                    action = Some(DrawingManagerAction::OpenDrawing(drawing.id));
                }
                if ui.small_button("X").on_hover_text("Remove drawing").clicked() {
                    action = Some(DrawingManagerAction::RemoveDrawing(drawing.id));
                }
            });
        }
    }

    ui.add_space(8.0);
    ui.menu_button("New Drawing...", |ui| {
        for template in DrawingTemplate::all() {
            if ui.button(template.label()).clicked() {
                action = Some(DrawingManagerAction::CreateFromTemplate(*template));
                ui.close_menu();
            }
        }
    });

    action
}

fn format_paper_size(drawing: &Drawing) -> String {
    let size = match drawing.paper_size {
        grove_core::drawing::layout::PaperSize::A4 => "A4",
        grove_core::drawing::layout::PaperSize::A3 => "A3",
        grove_core::drawing::layout::PaperSize::A2 => "A2",
        grove_core::drawing::layout::PaperSize::A1 => "A1",
        grove_core::drawing::layout::PaperSize::A0 => "A0",
    };
    let orient = match drawing.orientation {
        grove_core::drawing::layout::Orientation::Landscape => "L",
        grove_core::drawing::layout::Orientation::Portrait => "P",
    };
    format!("{} {}", size, orient)
}
