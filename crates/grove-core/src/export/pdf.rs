use std::path::Path;

use crate::drawing::drawing::Drawing;
use crate::project::Project;

use super::svg::SvgExportOptions;

/// Export a drawing to a PDF file via SVG→PDF conversion.
///
/// This reuses the full SVG drawing export (viewports, clipping, title blocks,
/// scale bars, north arrows, all geometry types, width-aware rendering) and
/// converts the result to PDF using `usvg` + `svg2pdf`.
pub fn export_drawing_pdf(
    project: &Project,
    drawing: &Drawing,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let pdf_bytes = export_drawing_pdf_bytes(project, drawing)?;
    std::fs::write(path, pdf_bytes)?;
    Ok(())
}

/// Export a drawing to PDF bytes (for testing or in-memory use).
pub fn export_drawing_pdf_bytes(
    project: &Project,
    drawing: &Drawing,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let svg_string = super::svg::export_drawing_svg(project, drawing, &SvgExportOptions::default())
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let tree = usvg::Tree::from_str(&svg_string, &usvg::Options::default())?;
    let pdf_bytes = svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    );
    let pdf_bytes = pdf_bytes.map_err(|e| format!("SVG to PDF conversion failed: {:?}", e))?;
    Ok(pdf_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::drawing::{Drawing, DrawingViewport};
    use crate::entity::{Entity, Geometry, PolylineData};
    use crate::garden::hardscape::WallData;
    use crate::project::Project;
    use glam::Vec2;

    #[test]
    fn test_pdf_export_creates_valid_pdf() {
        let mut project = Project::new();
        let layer_id = project.layers.layers[0].id;

        // Add a wall entity
        let mut wall = Entity::cad(
            "wall1",
            Geometry::Polyline(PolylineData {
                vertices: vec![
                    Vec2::new(0.0, 0.0),
                    Vec2::new(2000.0, 0.0),
                    Vec2::new(2000.0, 1000.0),
                ],
                closed: false,
            }),
        );
        wall.domain = crate::entity::DomainData::Wall(WallData {
            width_mm: 215,
            ..Default::default()
        });
        wall.layer_id = layer_id;
        project.add_entity(wall);

        let mut drawing = Drawing::new("01", "Test Plan");
        drawing.paper_size = crate::drawing::layout::PaperSize::A3;
        let mut vp = DrawingViewport::new("Main", 100);
        vp.model_center = [1000.0, 500.0];
        drawing.viewports.push(vp);

        let tmp = std::env::temp_dir().join("grove_test_export.pdf");
        let result = export_drawing_pdf(&project, &drawing, &tmp);
        assert!(result.is_ok(), "PDF export failed: {:?}", result.err());

        // Verify file exists and starts with PDF magic
        let bytes = std::fs::read(&tmp).unwrap();
        assert!(bytes.len() > 100, "PDF too small");
        assert_eq!(&bytes[0..5], b"%PDF-", "Not a valid PDF");

        // Clean up
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_pdf_bytes_export() {
        let project = Project::new();
        let drawing = Drawing::new("01", "Empty Drawing");

        let result = export_drawing_pdf_bytes(&project, &drawing);
        assert!(result.is_ok(), "PDF bytes export failed: {:?}", result.err());

        let bytes = result.unwrap();
        assert!(bytes.len() > 100, "PDF too small");
        assert_eq!(&bytes[0..5], b"%PDF-", "Not a valid PDF");
    }
}
