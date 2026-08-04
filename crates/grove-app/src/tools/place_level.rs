use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{DomainData, Entity, Geometry, LineData, PolylineData};
use grove_core::garden::levels::*;
use grove_core::snap::{self, SnapResult};

use super::draw_line::draw_snap_indicator;
use super::{collect_snap_centers, collect_snap_segments, ortho_constrain, Tool, ToolContext, ToolResult};
use crate::view::viewport::ViewTransform;

// ---------------------------------------------------------------------------
// Sub-type enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelSubType {
    SpotHeight,
    Contour,
    SlopeArrow,
    DrainRun,
}

impl LevelSubType {
    pub const ALL: &'static [Self] = &[
        Self::SpotHeight,
        Self::Contour,
        Self::SlopeArrow,
        Self::DrainRun,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::SpotHeight => "Spot Height",
            Self::Contour => "Contour",
            Self::SlopeArrow => "Slope Arrow",
            Self::DrainRun => "Drain Run",
        }
    }

    pub fn target_layer_name(&self) -> &'static str {
        match self {
            Self::SpotHeight | Self::Contour | Self::SlopeArrow => "GD_LEVELS",
            Self::DrainRun => "GD_DRAINAGE",
        }
    }

    pub fn default_colour(&self) -> [u8; 4] {
        match self {
            Self::SpotHeight => [180, 80, 80, 255],
            Self::Contour => [140, 120, 80, 255],
            Self::SlopeArrow => [180, 100, 60, 255],
            Self::DrainRun => [60, 120, 180, 255],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LevelDrawingMode {
    Point,
    Polyline,
    TwoClickLine,
}

impl LevelSubType {
    fn drawing_mode(&self) -> LevelDrawingMode {
        match self {
            Self::SpotHeight => LevelDrawingMode::Point,
            Self::Contour | Self::DrainRun => LevelDrawingMode::Polyline,
            Self::SlopeArrow => LevelDrawingMode::TwoClickLine,
        }
    }
}

// ---------------------------------------------------------------------------
// Tool struct
// ---------------------------------------------------------------------------

pub struct PlaceLevelTool {
    pub sub_type: LevelSubType,
    pub spot_height: SpotHeightData,
    pub contour: ContourData,
    pub slope_arrow: SlopeArrowData,
    pub drain_run: DrainRunData,
    // Drawing state
    vertices: Vec<Vec2>,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    pub layer_id: uuid::Uuid,
}

impl PlaceLevelTool {
    pub fn new() -> Self {
        Self {
            sub_type: LevelSubType::SpotHeight,
            spot_height: SpotHeightData::default(),
            contour: ContourData::default(),
            slope_arrow: SlopeArrowData::default(),
            drain_run: DrainRunData::default(),
            vertices: Vec::new(),
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
        }
    }

    pub fn target_layer_name(&self) -> &str {
        self.sub_type.target_layer_name()
    }

    fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        let constrained = if ctx.shift_held && !self.vertices.is_empty() {
            ortho_constrain(*self.vertices.last().unwrap(), raw)
        } else {
            raw
        };
        if ctx.snap_config.enabled {
            let (segs, centers) = self.snap_data(ctx);
            let mut candidates = snap::snap_points(constrained, &segs, &centers, ctx.snap_config);
            if !ctx.smart_points.is_empty() {
                candidates.extend(super::extension_snap_candidates(constrained, ctx.smart_points));
            }
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }
        constrained
    }

    fn snap_data(&self, ctx: &ToolContext) -> (Vec<(Vec2, Vec2)>, Vec<snap::CenterPoint>) {
        let mut segs = Vec::new();
        let mut centers = Vec::new();
        for entity in ctx.entities {
            collect_snap_segments(&entity.geometry, &mut segs);
            collect_snap_centers(&entity.geometry, &mut centers);
        }
        for w in self.vertices.windows(2) {
            segs.push((w[0], w[1]));
        }
        (segs, centers)
    }

    fn colour(&self) -> [u8; 4] {
        self.sub_type.default_colour()
    }

    fn finish_shape(&mut self) -> ToolResult {
        if let Some(entity) = self.make_entity() {
            let cmd = Box::new(AddEntityCommand::new(entity));
            self.vertices.clear();
            ToolResult::EmitAndContinue(cmd)
        } else {
            ToolResult::Continue
        }
    }

    fn make_entity(&self) -> Option<Entity> {
        let mode = self.sub_type.drawing_mode();
        let geom = match mode {
            LevelDrawingMode::Point => {
                if self.vertices.is_empty() {
                    return None;
                }
                Geometry::Point(self.vertices[0])
            }
            LevelDrawingMode::TwoClickLine => {
                if self.vertices.len() < 2 {
                    return None;
                }
                Geometry::Line(LineData {
                    start: self.vertices[0],
                    end: self.vertices[1],
                })
            }
            LevelDrawingMode::Polyline => {
                if self.vertices.len() < 2 {
                    return None;
                }
                if self.vertices.len() == 2 {
                    Geometry::Line(LineData {
                        start: self.vertices[0],
                        end: self.vertices[1],
                    })
                } else {
                    Geometry::Polyline(PolylineData {
                        vertices: self.vertices.clone(),
                        closed: false,
                    })
                }
            }
        };

        let domain = match self.sub_type {
            LevelSubType::SpotHeight => DomainData::SpotHeight(self.spot_height.clone()),
            LevelSubType::Contour => DomainData::Contour(self.contour.clone()),
            LevelSubType::SlopeArrow => DomainData::SlopeArrow(self.slope_arrow.clone()),
            LevelSubType::DrainRun => DomainData::DrainRun(self.drain_run.clone()),
        };

        let mut entity = Entity::new(self.sub_type.label(), geom, domain);
        entity.layer_id = self.layer_id;
        entity.colour = self.colour();
        Some(entity)
    }
}

// ---------------------------------------------------------------------------
// Tool trait
// ---------------------------------------------------------------------------

impl Tool for PlaceLevelTool {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = if self.layer_id.is_nil() {
            ctx.active_layer_id
        } else {
            self.layer_id
        };
        let effective = self.effective_pos(pos, ctx);
        let mode = self.sub_type.drawing_mode();

        match mode {
            LevelDrawingMode::Point => {
                self.vertices = vec![effective];
                self.finish_shape()
            }
            LevelDrawingMode::TwoClickLine => {
                self.vertices.push(effective);
                if self.vertices.len() >= 2 {
                    self.finish_shape()
                } else {
                    ToolResult::Continue
                }
            }
            LevelDrawingMode::Polyline => {
                self.vertices.push(effective);
                ToolResult::Continue
            }
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        let effective = self.effective_pos(pos, ctx);
        self.current_pos = effective;

        if ctx.snap_config.enabled {
            let constrained = if ctx.shift_held && !self.vertices.is_empty() {
                ortho_constrain(*self.vertices.last().unwrap(), pos)
            } else {
                pos
            };
            let (segments, centers) = self.snap_data(ctx);
            let mut candidates = snap::snap_points(constrained, &segments, &centers, ctx.snap_config);
            if !ctx.smart_points.is_empty() {
                candidates.extend(super::extension_snap_candidates(constrained, ctx.smart_points));
            }
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }
    }

    fn on_key_enter(&mut self) -> ToolResult {
        self.finish_shape()
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.vertices.is_empty() {
            ToolResult::Finished
        } else {
            self.vertices.clear();
            ToolResult::Continue
        }
    }

    fn on_double_click(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        let mode = self.sub_type.drawing_mode();
        if mode == LevelDrawingMode::Polyline {
            let effective = self.effective_pos(pos, ctx);
            self.vertices.push(effective);
            self.finish_shape()
        } else {
            ToolResult::Continue
        }
    }

    fn on_right_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        if self.vertices.len() > 1 {
            self.vertices.pop();
            ToolResult::Continue
        } else if !self.vertices.is_empty() {
            self.vertices.clear();
            ToolResult::Continue
        } else {
            ToolResult::Finished
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let col = self.colour();
        let stroke_colour = egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 180);
        let fill_colour = egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 40);
        let stroke = egui::Stroke::new(1.5, stroke_colour);
        let preview_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 120));
        let dot_colour = egui::Color32::from_rgb(col[0], col[1], col[2]);
        let mode = self.sub_type.drawing_mode();

        match mode {
            LevelDrawingMode::Point => {
                let screen = viewport.world_to_screen(self.current_pos);
                // Cross marker for spot heights
                let size = 6.0;
                painter.line_segment(
                    [
                        egui::pos2(screen.x - size, screen.y),
                        egui::pos2(screen.x + size, screen.y),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(screen.x, screen.y - size),
                        egui::pos2(screen.x, screen.y + size),
                    ],
                    stroke,
                );
                // Elevation label
                let elev_m = self.spot_height.elevation_mm / 1000.0;
                painter.text(
                    egui::pos2(screen.x + 8.0, screen.y - 8.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{:.2}m", elev_m),
                    egui::FontId::proportional(10.0),
                    stroke_colour,
                );
            }
            LevelDrawingMode::TwoClickLine => {
                if !self.vertices.is_empty() {
                    let start = viewport.world_to_screen(self.vertices[0]);
                    let end = viewport.world_to_screen(self.current_pos);
                    painter.line_segment([start, end], preview_stroke);
                    painter.circle_filled(start, 3.0, dot_colour);
                    // Arrow head at end
                    let dir = egui::vec2(end.x - start.x, end.y - start.y);
                    let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
                    if len > 10.0 {
                        let norm = egui::vec2(dir.x / len, dir.y / len);
                        let perp = egui::vec2(-norm.y, norm.x);
                        let arrow_size = 8.0;
                        let tip = end;
                        let left = egui::pos2(
                            tip.x - norm.x * arrow_size + perp.x * arrow_size * 0.4,
                            tip.y - norm.y * arrow_size + perp.y * arrow_size * 0.4,
                        );
                        let right = egui::pos2(
                            tip.x - norm.x * arrow_size - perp.x * arrow_size * 0.4,
                            tip.y - norm.y * arrow_size - perp.y * arrow_size * 0.4,
                        );
                        painter.add(egui::Shape::convex_polygon(
                            vec![tip, left, right],
                            fill_colour,
                            stroke,
                        ));
                    }
                }
            }
            LevelDrawingMode::Polyline => {
                if !self.vertices.is_empty() {
                    for w in self.vertices.windows(2) {
                        let a = viewport.world_to_screen(w[0]);
                        let b = viewport.world_to_screen(w[1]);
                        painter.line_segment([a, b], stroke);
                    }
                    let last = viewport.world_to_screen(*self.vertices.last().unwrap());
                    let cursor = viewport.world_to_screen(self.current_pos);
                    painter.line_segment([last, cursor], preview_stroke);

                    for v in &self.vertices {
                        let screen = viewport.world_to_screen(*v);
                        painter.circle_filled(screen, 3.0, dot_colour);
                    }
                }
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
