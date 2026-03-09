use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Paper sizes in mm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperSize {
    A4,
    A3,
    A2,
    A1,
    A0,
}

impl PaperSize {
    /// Returns (width, height) in mm for landscape orientation.
    pub fn dimensions_mm(&self) -> (f32, f32) {
        match self {
            PaperSize::A4 => (297.0, 210.0),
            PaperSize::A3 => (420.0, 297.0),
            PaperSize::A2 => (594.0, 420.0),
            PaperSize::A1 => (841.0, 594.0),
            PaperSize::A0 => (1189.0, 841.0),
        }
    }

    /// Short label, e.g. "A3".
    pub fn label(&self) -> &'static str {
        match self {
            PaperSize::A4 => "A4",
            PaperSize::A3 => "A3",
            PaperSize::A2 => "A2",
            PaperSize::A1 => "A1",
            PaperSize::A0 => "A0",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    Landscape,
    Portrait,
}

/// A viewport on a layout sheet — shows a portion of the design at a given scale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutViewport {
    pub position: Vec2,
    pub size: Vec2,
    pub center: Vec2,
    pub scale: f32,
}

/// Legend data for a layout sheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendData {
    pub position: Vec2,
    pub entries: Vec<LegendEntry>,
}

impl Default for LegendData {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendEntry {
    pub label: String,
    pub colour: [u8; 4],
    pub symbol: LegendSymbol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LegendSymbol {
    Circle,
    Square,
    Line,
    DashedLine,
}

/// A print layout sheet model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutSheet {
    pub paper_size: PaperSize,
    pub orientation: Orientation,
    pub viewports: Vec<LayoutViewport>,
    pub show_title_block: bool,
    pub show_north_arrow: bool,
    pub show_scale_bar: bool,
    pub show_legend: bool,
}

impl Default for LayoutSheet {
    fn default() -> Self {
        Self {
            paper_size: PaperSize::A1,
            orientation: Orientation::Landscape,
            viewports: Vec::new(),
            show_title_block: true,
            show_north_arrow: true,
            show_scale_bar: true,
            show_legend: true,
        }
    }
}

/// Check if a layer is visible within a viewport, considering overrides.
/// If the viewport has an override for this layer, use it; otherwise fall back
/// to the model-space visibility from the LayerManager.
pub fn is_layer_visible_in_viewport(
    layer_id: uuid::Uuid,
    viewport: &super::drawing::DrawingViewport,
    layers: &crate::layer::LayerManager,
) -> bool {
    viewport
        .layer_overrides
        .visibility
        .get(&layer_id)
        .copied()
        .unwrap_or_else(|| layers.is_visible(layer_id))
}
