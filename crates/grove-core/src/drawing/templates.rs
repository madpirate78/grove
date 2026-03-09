use crate::layer::LayerManager;

use super::drawing::{Drawing, DrawingViewport, LayerOverrides};
use super::layout::{Orientation, PaperSize};

/// Available drawing templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingTemplate {
    MasterPlan,
    PlantingPlan,
    SketchPlan,
    ConstructionDetail,
    LevelsDrawing,
    DrainagePlan,
    SiteSurvey,
}

impl DrawingTemplate {
    pub fn label(&self) -> &'static str {
        match self {
            Self::MasterPlan => "Master Plan",
            Self::PlantingPlan => "Planting Plan",
            Self::SketchPlan => "Sketch Plan",
            Self::ConstructionDetail => "Construction Detail",
            Self::LevelsDrawing => "Levels Drawing",
            Self::DrainagePlan => "Drainage Plan",
            Self::SiteSurvey => "Site Survey",
        }
    }

    pub fn all() -> &'static [DrawingTemplate] {
        &[
            Self::MasterPlan,
            Self::PlantingPlan,
            Self::SketchPlan,
            Self::ConstructionDetail,
            Self::LevelsDrawing,
            Self::DrainagePlan,
            Self::SiteSurvey,
        ]
    }
}

/// Create a drawing from a template, using the layer manager to look up layer IDs.
pub fn create_from_template(
    template: DrawingTemplate,
    number: &str,
    layers: &LayerManager,
) -> Drawing {
    let (title, scale, paper_size, orientation, visible_layers) = match template {
        DrawingTemplate::MasterPlan => (
            "Master Plan",
            100,
            PaperSize::A1,
            Orientation::Landscape,
            vec![], // all visible
        ),
        DrawingTemplate::PlantingPlan => (
            "Planting Plan",
            100,
            PaperSize::A1,
            Orientation::Landscape,
            vec![
                "GD_PLANTING",
                "GD_BOUNDARY",
                "GD_BUILDINGS",
                "GD_HARDSCAPE_SURFACES",
            ],
        ),
        DrawingTemplate::SketchPlan => (
            "Sketch Plan",
            200,
            PaperSize::A3,
            Orientation::Landscape,
            vec![], // all visible
        ),
        DrawingTemplate::ConstructionDetail => (
            "Construction Detail",
            20,
            PaperSize::A3,
            Orientation::Landscape,
            vec![
                "GD_HARDSCAPE_WALLS",
                "GD_HARDSCAPE_SURFACES",
                "GD_HARDSCAPE_STEPS",
                "GD_HARDSCAPE_STRUCTURES",
                "GD_HARDSCAPE_WATER",
                "GD_HARDSCAPE_EDGING",
                "GD_LEVELS",
                "GD_BOUNDARY",
                "GD_BUILDINGS",
            ],
        ),
        DrawingTemplate::LevelsDrawing => (
            "Levels Drawing",
            100,
            PaperSize::A1,
            Orientation::Landscape,
            vec![
                "GD_LEVELS",
                "GD_BOUNDARY",
                "GD_BUILDINGS",
                "GD_DRAINAGE",
            ],
        ),
        DrawingTemplate::DrainagePlan => (
            "Drainage Plan",
            100,
            PaperSize::A1,
            Orientation::Landscape,
            vec![
                "GD_DRAINAGE",
                "GD_LEVELS",
                "GD_BOUNDARY",
                "GD_BUILDINGS",
            ],
        ),
        DrawingTemplate::SiteSurvey => (
            "Site Survey",
            200,
            PaperSize::A1,
            Orientation::Landscape,
            vec![
                "GD_SURVEY",
                "GD_BOUNDARY",
                "GD_BUILDINGS",
            ],
        ),
    };

    let mut drawing = Drawing::new(number, title);
    drawing.paper_size = paper_size;
    drawing.orientation = orientation;

    // Build layer overrides: if visible_layers is non-empty, hide everything not listed
    if !visible_layers.is_empty() {
        let mut overrides = LayerOverrides::default();
        for layer in &layers.layers {
            let should_be_visible = visible_layers.contains(&layer.name.as_str());
            if !should_be_visible {
                overrides.visibility.insert(layer.id, false);
            }
        }
        let mut viewport = DrawingViewport::new("Main View", scale);
        // Size viewport to fill most of the printable area
        let (pw, ph) = drawing.paper_dimensions_mm();
        let m = &drawing.margins;
        viewport.sheet_position = [m[3], m[0]];
        viewport.sheet_size = [pw - m[1] - m[3], ph - m[0] - m[2]];
        viewport.layer_overrides = overrides;
        drawing.viewports.push(viewport);
    } else {
        let mut viewport = DrawingViewport::new("Main View", scale);
        let (pw, ph) = drawing.paper_dimensions_mm();
        let m = &drawing.margins;
        viewport.sheet_position = [m[3], m[0]];
        viewport.sheet_size = [pw - m[1] - m[3], ph - m[0] - m[2]];
        drawing.viewports.push(viewport);
    }

    drawing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_master_plan_template() {
        let layers = LayerManager::new();
        let d = create_from_template(DrawingTemplate::MasterPlan, "01", &layers);
        assert_eq!(d.title, "Master Plan");
        assert_eq!(d.viewports.len(), 1);
        assert_eq!(d.viewports[0].scale, 100);
        // Master plan has no overrides (all visible)
        assert!(d.viewports[0].layer_overrides.visibility.is_empty());
    }

    #[test]
    fn test_planting_plan_template() {
        let layers = LayerManager::new();
        let d = create_from_template(DrawingTemplate::PlantingPlan, "02", &layers);
        assert_eq!(d.title, "Planting Plan");
        let overrides = &d.viewports[0].layer_overrides;
        // Planting plan should hide layers not in the visible list
        // GD_DRAINAGE should be hidden
        let drainage = layers.find_by_name("GD_DRAINAGE").unwrap();
        assert_eq!(overrides.visibility.get(&drainage.id), Some(&false));
        // GD_PLANTING should NOT be overridden (inherits model-space visible)
        let planting = layers.find_by_name("GD_PLANTING").unwrap();
        assert!(!overrides.visibility.contains_key(&planting.id));
    }

    #[test]
    fn test_template_labels() {
        for t in DrawingTemplate::all() {
            assert!(!t.label().is_empty());
        }
    }

    #[test]
    fn test_construction_detail_scale() {
        let layers = LayerManager::new();
        let d = create_from_template(DrawingTemplate::ConstructionDetail, "03", &layers);
        assert_eq!(d.viewports[0].scale, 20);
        assert_eq!(d.paper_size, PaperSize::A3);
    }
}
