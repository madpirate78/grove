use std::path::Path;
use std::sync::Arc;

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

    // Also write the SVG alongside the PDF for verification
    let svg_path = path.with_extension("svg");
    let svg_string = super::svg::export_drawing_svg(project, drawing, &SvgExportOptions::default())
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let _ = std::fs::write(&svg_path, &svg_string);

    Ok(())
}

/// Export a drawing to PDF bytes (for testing or in-memory use).
pub fn export_drawing_pdf_bytes(
    project: &Project,
    drawing: &Drawing,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let svg_string = super::svg::export_drawing_svg(project, drawing, &SvgExportOptions::default())
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    // Load system fonts so usvg can resolve <text> elements into glyph outlines.
    // Without this, all text is silently dropped from the PDF.
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts();
    let mut opts = usvg::Options::default();
    opts.font_family = "sans-serif".to_owned();
    opts.fontdb = Arc::new(fontdb);

    let tree = usvg::Tree::from_str(&svg_string, &opts)?;
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
    fn test_svg_viewport_content_preserved_in_usvg() {
        use crate::entity::{Entity, Geometry, LineData};

        let mut project = Project::new();
        let layer_id = project.layers.layers[0].id;
        let mut line = Entity::cad(
            "diag_line",
            Geometry::Line(LineData {
                start: Vec2::new(0.0, 0.0),
                end: Vec2::new(5000.0, 5000.0),
            }),
        );
        line.layer_id = layer_id;
        line.colour = [255, 0, 0, 255]; // Red so it's definitely visible
        project.add_entity(line);

        let mut drawing = Drawing::new("01", "Diag Test");
        drawing.paper_size = crate::drawing::layout::PaperSize::A3;
        let mut vp = DrawingViewport::new("Main", 100);
        vp.model_center = [2500.0, 2500.0];
        drawing.viewports.push(vp);

        let svg_string = super::super::svg::export_drawing_svg(
            &project, &drawing, &SvgExportOptions::default(),
        ).unwrap();

        // Verify the SVG contains our line
        assert!(svg_string.contains("<line"), "SVG should contain a <line> element");
        assert!(svg_string.contains("clip-path"), "SVG should have clip-path");

        // Parse with usvg and verify geometry is preserved through the pipeline
        let mut fontdb = usvg::fontdb::Database::new();
        fontdb.load_system_fonts();
        let mut opts = usvg::Options::default();
        opts.font_family = "sans-serif".to_owned();
        opts.fontdb = Arc::new(fontdb);

        let tree = usvg::Tree::from_str(&svg_string, &opts).unwrap();
        let processed_svg = tree.to_string(&usvg::WriteOptions::default());

        // usvg converts <line> to <path>, so check for path data
        assert!(
            processed_svg.contains("<path") || processed_svg.contains("<line"),
            "usvg tree should preserve viewport geometry"
        );
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
