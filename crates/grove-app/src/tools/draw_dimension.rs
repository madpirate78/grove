use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::drawing::dimension::DimensionData;
use grove_core::entity::{Entity, Geometry};
use grove_core::snap::{self, SnapResult};

use super::{collect_snap_centers, collect_snap_segments, Tool, ToolContext, ToolResult};
use super::draw_line::draw_snap_indicator;
use crate::view::viewport::ViewTransform;

/// State machine for the dimension tool.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DimState {
    /// Waiting for first point.
    Start,
    /// First point placed, waiting for second.
    SecondPoint,
    /// Both points placed, dragging offset.
    Offset,
}

/// Draw linear dimensions by clicking two points, then dragging to set the offset.
pub struct DrawDimensionTool {
    state: DimState,
    start: Vec2,
    end: Vec2,
    offset: f32,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    layer_id: uuid::Uuid,
}

impl DrawDimensionTool {
    pub fn new() -> Self {
        Self {
            state: DimState::Start,
            start: Vec2::ZERO,
            end: Vec2::ZERO,
            offset: 0.0,
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
        }
    }

    fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        if ctx.snap_config.enabled {
            let mut segs = Vec::new();
            let mut centers = Vec::new();
            for entity in ctx.entities {
                collect_snap_segments(&entity.geometry, &mut segs);
                collect_snap_centers(&entity.geometry, &mut centers);
            }
            let mut candidates = snap::snap_points(raw, &segs, &centers, ctx.snap_config);
            if !ctx.smart_points.is_empty() {
                candidates.extend(super::extension_snap_candidates(raw, ctx.smart_points));
            }
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }
        raw
    }

    fn update_snap(&mut self, pos: Vec2, ctx: &ToolContext) {
        if ctx.snap_config.enabled {
            let mut segs = Vec::new();
            let mut centers = Vec::new();
            for entity in ctx.entities {
                collect_snap_segments(&entity.geometry, &mut segs);
                collect_snap_centers(&entity.geometry, &mut centers);
            }
            let mut candidates = snap::snap_points(pos, &segs, &centers, ctx.snap_config);
            if !ctx.smart_points.is_empty() {
                candidates.extend(super::extension_snap_candidates(pos, ctx.smart_points));
            }
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }
    }

    /// Calculate offset from cursor position (perpendicular distance from the dimension line).
    fn calc_offset(&self, cursor: Vec2) -> f32 {
        let dir = self.end - self.start;
        let len = dir.length();
        if len < f32::EPSILON {
            return 0.0;
        }
        let normal = Vec2::new(-dir.y, dir.x) / len;
        (cursor - self.start).dot(normal)
    }

    fn finish_dimension(&mut self) -> ToolResult {
        let entity = {
            let mut e = Entity::cad(
                "Dimension",
                Geometry::Dimension(DimensionData::Linear {
                    start: self.start,
                    end: self.end,
                    offset: self.offset,
                    text_override: None,
                }),
            );
            e.layer_id = self.layer_id;
            e.colour = [180, 180, 190, 255];
            e
        };

        self.state = DimState::Start;
        ToolResult::EmitAndContinue(Box::new(AddEntityCommand::new(entity)))
    }
}

impl Tool for DrawDimensionTool {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = ctx.active_layer_id;
        match self.state {
            DimState::Start => {
                self.start = self.effective_pos(pos, ctx);
                self.state = DimState::SecondPoint;
                ToolResult::Continue
            }
            DimState::SecondPoint => {
                self.end = self.effective_pos(pos, ctx);
                self.offset = 40.0; // Default offset
                self.state = DimState::Offset;
                ToolResult::Continue
            }
            DimState::Offset => {
                self.offset = self.calc_offset(pos);
                self.finish_dimension()
            }
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        self.current_pos = pos;
        match self.state {
            DimState::Start | DimState::SecondPoint => {
                self.update_snap(pos, ctx);
            }
            DimState::Offset => {
                self.offset = self.calc_offset(pos);
                self.snap = None;
            }
        }
    }

    fn on_key_escape(&mut self) -> ToolResult {
        match self.state {
            DimState::Start => ToolResult::Finished,
            DimState::SecondPoint => {
                self.state = DimState::Start;
                ToolResult::Continue
            }
            DimState::Offset => {
                self.state = DimState::Start;
                ToolResult::Continue
            }
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let accent = egui::Color32::from_rgb(180, 180, 190);
        let dim_stroke = egui::Stroke::new(1.0, accent);
        let preview_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(180, 180, 190, 120));

        match self.state {
            DimState::Start => {
                // Just show snap indicator
            }
            DimState::SecondPoint => {
                // Show line from start to cursor
                let a = viewport.world_to_screen(self.start);
                let b = viewport.world_to_screen(self.current_pos);
                painter.line_segment([a, b], preview_stroke);
                painter.circle_filled(a, 3.0, accent);
            }
            DimState::Offset => {
                // Show full dimension preview
                draw_dimension_preview(
                    painter, viewport,
                    self.start, self.end, self.offset,
                    accent, dim_stroke,
                );
            }
        }

        if let Some(ref snap) = self.snap {
            draw_snap_indicator(painter, viewport, snap);
        }
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        self.snap.as_ref()
    }
}

/// Draw a linear dimension annotation (used by both preview and canvas rendering).
pub fn draw_dimension_preview(
    painter: &egui::Painter,
    viewport: &ViewTransform,
    start: Vec2,
    end: Vec2,
    offset: f32,
    colour: egui::Color32,
    stroke: egui::Stroke,
) {
    let dir = end - start;
    let len = dir.length();
    if len < f32::EPSILON {
        return;
    }
    let normal = Vec2::new(-dir.y, dir.x) / len;
    let offset_vec = normal * offset;

    let p1 = start + offset_vec;
    let p2 = end + offset_vec;

    let s_p1 = viewport.world_to_screen(p1);
    let s_p2 = viewport.world_to_screen(p2);

    // Dimension line
    painter.line_segment([s_p1, s_p2], stroke);

    // Extension lines (from points to dimension line, with small gap)
    let gap = normal * offset.signum() * 4.0;
    let ext_overshoot = normal * offset.signum() * 8.0;

    let s_start = viewport.world_to_screen(start + gap);
    let s_start_ext = viewport.world_to_screen(start + offset_vec + ext_overshoot);
    let s_end = viewport.world_to_screen(end + gap);
    let s_end_ext = viewport.world_to_screen(end + offset_vec + ext_overshoot);

    let ext_stroke = egui::Stroke::new(0.8, colour);
    painter.line_segment([s_start, s_start_ext], ext_stroke);
    painter.line_segment([s_end, s_end_ext], ext_stroke);

    // Arrowheads (small ticks at 45 degrees)
    let arrow_len = 6.0; // screen pixels
    let dim_dir_screen = (s_p2 - s_p1).normalized();
    let arrow_perp = egui::vec2(-dim_dir_screen.y, dim_dir_screen.x);

    // Left arrow
    let arr1a = s_p1 + dim_dir_screen * arrow_len + arrow_perp * arrow_len * 0.3;
    let arr1b = s_p1 + dim_dir_screen * arrow_len - arrow_perp * arrow_len * 0.3;
    painter.line_segment([s_p1, arr1a], stroke);
    painter.line_segment([s_p1, arr1b], stroke);

    // Right arrow
    let arr2a = s_p2 - dim_dir_screen * arrow_len + arrow_perp * arrow_len * 0.3;
    let arr2b = s_p2 - dim_dir_screen * arrow_len - arrow_perp * arrow_len * 0.3;
    painter.line_segment([s_p2, arr2a], stroke);
    painter.line_segment([s_p2, arr2b], stroke);

    // Measurement text
    let mid = (s_p1 + s_p2.to_vec2()) * 0.5;
    let measurement = len;
    let text = if measurement >= 1000.0 {
        format!("{:.2}m", measurement / 1000.0)
    } else {
        format!("{:.0}mm", measurement)
    };
    let text_offset = arrow_perp * 12.0;
    painter.text(
        egui::pos2(mid.x + text_offset.x, mid.y + text_offset.y),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(11.0),
        colour,
    );
}
