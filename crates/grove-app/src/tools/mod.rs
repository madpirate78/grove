pub mod select;
pub mod draw_line;
pub mod draw_rect;
pub mod draw_circle;
pub mod draw_arc;
pub mod draw_dimension;
pub mod draw_text;
pub mod polygon_drawer;
pub mod place_plant;
pub mod place_group;
pub mod place_hardscape;
pub mod place_level;
pub mod subtract_polygon;
pub mod modify;
pub mod array;

use grove_core::commands::Command;
use grove_core::entity::{Entity, Geometry};
use grove_core::snap::{CenterPoint, SnapConfig, SnapResult};
use glam::Vec2;

/// Context passed to tools each frame.
pub struct ToolContext<'a> {
    pub entities: &'a [Entity],
    pub snap_config: &'a SnapConfig,
    pub shift_held: bool,
    pub ctrl_held: bool,
    pub active_layer_id: uuid::Uuid,
    pub viewport_zoom: f32,
    pub selected_ids: &'a [uuid::Uuid],
}

/// What a tool returns after handling input.
pub enum ToolResult {
    /// Nothing happened, keep going.
    Continue,
    /// Tool produced a command to execute.
    Emit(Box<dyn Command>),
    /// Tool produced a command but stays active (e.g. continuous placement).
    EmitAndContinue(Box<dyn Command>),
    /// Tool is done, switch back to select.
    Finished,
}

/// The interactive tool protocol.
pub trait Tool {
    fn name(&self) -> &str;
    /// Downcast support for runtime tool mutation (e.g. updating plant selection).
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    /// Downcast support for runtime tool inspection.
    fn as_any_ref(&self) -> &dyn std::any::Any { &() }
    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult;
    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext);
    fn on_mouse_up(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        ToolResult::Continue
    }
    fn on_key_escape(&mut self) -> ToolResult {
        ToolResult::Finished
    }
    fn on_key_enter(&mut self) -> ToolResult {
        ToolResult::Continue
    }
    fn on_double_click(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        let _ = (pos, ctx);
        ToolResult::Continue
    }
    fn on_right_click(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        let _ = (pos, ctx);
        ToolResult::Continue
    }
    /// Handle a generic key press. Tools can override to handle tool-specific keys.
    fn on_key_pressed(&mut self, _key: egui::Key) -> ToolResult {
        ToolResult::Continue
    }
    /// Handle text input (for text tool). Returns true if consumed.
    fn on_text_input(&mut self, _text: &str) -> bool {
        false
    }
    fn draw_preview(&self, painter: &egui::Painter, viewport: &crate::view::viewport::ViewTransform);
    /// Current snap result for display.
    fn snap_result(&self) -> Option<&SnapResult> {
        None
    }
}

/// Collect line segments from an entity's geometry for snapping.
pub fn collect_snap_segments(geom: &Geometry, segs: &mut Vec<(Vec2, Vec2)>) {
    match geom {
        Geometry::Line(l) => segs.push((l.start, l.end)),
        Geometry::Polyline(pl) => {
            for w in pl.vertices.windows(2) {
                segs.push((w[0], w[1]));
            }
            if pl.closed && pl.vertices.len() >= 2 {
                segs.push((*pl.vertices.last().unwrap(), pl.vertices[0]));
            }
        }
        Geometry::Polygon(pg) => {
            for w in pg.vertices.windows(2) {
                segs.push((w[0], w[1]));
            }
            if pg.vertices.len() >= 2 {
                segs.push((*pg.vertices.last().unwrap(), pg.vertices[0]));
            }
            // Hole ring segments are also valid snap targets
            for hole in &pg.holes {
                for w in hole.windows(2) {
                    segs.push((w[0], w[1]));
                }
                if hole.len() >= 2 {
                    segs.push((*hole.last().unwrap(), hole[0]));
                }
            }
        }
        Geometry::Rectangle(r) => {
            let half = r.half_size;
            let corners = if r.rotation.abs() < f32::EPSILON {
                [
                    r.center + Vec2::new(-half.x, -half.y),
                    r.center + Vec2::new(half.x, -half.y),
                    r.center + Vec2::new(half.x, half.y),
                    r.center + Vec2::new(-half.x, half.y),
                ]
            } else {
                let (sin, cos) = r.rotation.sin_cos();
                let local = [
                    Vec2::new(-half.x, -half.y),
                    Vec2::new(half.x, -half.y),
                    Vec2::new(half.x, half.y),
                    Vec2::new(-half.x, half.y),
                ];
                [
                    r.center + Vec2::new(local[0].x * cos - local[0].y * sin, local[0].x * sin + local[0].y * cos),
                    r.center + Vec2::new(local[1].x * cos - local[1].y * sin, local[1].x * sin + local[1].y * cos),
                    r.center + Vec2::new(local[2].x * cos - local[2].y * sin, local[2].x * sin + local[2].y * cos),
                    r.center + Vec2::new(local[3].x * cos - local[3].y * sin, local[3].x * sin + local[3].y * cos),
                ]
            };
            for i in 0..4 {
                segs.push((corners[i], corners[(i + 1) % 4]));
            }
        }
        _ => {}
    }
}

/// Collect center points from entities for center snapping.
pub fn collect_snap_centers(geom: &Geometry, centers: &mut Vec<CenterPoint>) {
    match geom {
        Geometry::Circle(c) => centers.push(CenterPoint { position: c.center }),
        Geometry::Arc(a) => centers.push(CenterPoint { position: a.center }),
        Geometry::Rectangle(r) => centers.push(CenterPoint { position: r.center }),
        _ => {}
    }
}

/// Gather all snap candidates from existing entities and grid.
pub fn snap_candidates_from_entities(
    cursor: Vec2,
    entities: &[Entity],
    config: &grove_core::snap::SnapConfig,
) -> Vec<SnapResult> {
    let mut segs = Vec::new();
    let mut centers = Vec::new();
    for entity in entities {
        collect_snap_segments(&entity.geometry, &mut segs);
        collect_snap_centers(&entity.geometry, &mut centers);
    }
    grove_core::snap::snap_points(cursor, &segs, &centers, config)
}

/// Apply ortho constraint: snap to horizontal or vertical from anchor.
pub fn ortho_constrain(anchor: Vec2, cursor: Vec2) -> Vec2 {
    let delta = cursor - anchor;
    if delta.x.abs() >= delta.y.abs() {
        Vec2::new(cursor.x, anchor.y)
    } else {
        Vec2::new(anchor.x, cursor.y)
    }
}
