use std::collections::BTreeMap;
use std::path::Path;

use glam::Vec2;
use svg::node::element::{self, path::Data};
use svg::Document;
use uuid::Uuid;

use crate::drawing::dimension::DimensionData;
use crate::drawing::drawing::{Drawing, PlantStyle};
use crate::drawing::layout::is_layer_visible_in_viewport;
use crate::drawing::legend::{collect_legend_items, render_legend_svg};
use crate::drawing::north_arrow::{render_north_arrow_svg, render_scale_bar_svg};
use crate::drawing::title_block_render::render_title_block_svg;
use crate::entity::{CircleData, DomainData, Entity, Geometry};
use crate::garden::plant::PlantData;
use crate::garden::colour::{resolve_colour, rgba_to_hex};
use crate::geometry::{offset_polyline, offset_polyline_sides};
use crate::project::Project;

pub struct SvgExportOptions {
    pub visible_only: bool,
    pub margin_mm: f32,
    pub stroke_width_mm: f32,
    pub background: Option<[u8; 4]>,
}

impl Default for SvgExportOptions {
    fn default() -> Self {
        Self {
            visible_only: true,
            margin_mm: 50.0,
            stroke_width_mm: 1.0,
            background: None,
        }
    }
}

/// Compute a stroke weight (in mm on paper) based on the entity's domain data.
/// Taller elements get heavier lines, creating visual hierarchy in plan view.
/// House always gets the heaviest weight as the dominant structure.
pub fn stroke_weight_for_domain(domain: &DomainData) -> f32 {
    const MIN_WEIGHT: f32 = 0.18;
    const MAX_WEIGHT: f32 = 0.50;
    const DEFAULT_WEIGHT: f32 = 0.25;
    const MAX_HEIGHT: f32 = 5000.0;
    const HOUSE_WEIGHT: f32 = 0.70;

    if matches!(domain, DomainData::House(_)) {
        return HOUSE_WEIGHT;
    }
    if matches!(domain, DomainData::Entourage(_)) {
        return MIN_WEIGHT;
    }

    match domain.height_mm() {
        None => DEFAULT_WEIGHT,
        Some(h) => {
            let t = (h.max(0) as f32 / MAX_HEIGHT).clamp(0.0, 1.0);
            MIN_WEIGHT + t * (MAX_WEIGHT - MIN_WEIGHT)
        }
    }
}

mod drawing;
mod model;
mod plants;
mod primitives;
#[cfg(test)]
mod tests;

pub use drawing::{export_all_drawings_svg, export_drawing_svg, export_drawing_svg_to_file};
pub use model::{export_svg, export_svg_to_file};
