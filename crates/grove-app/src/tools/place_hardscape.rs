use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{
    DomainData, Entity, Geometry, LineData, PolygonData, PolylineData, RectData,
};
use grove_core::garden::hardscape::*;
use grove_core::geometry::would_self_intersect;
use grove_core::snap::{self, SnapResult};

use super::draw_line::draw_snap_indicator;
use super::{collect_snap_centers, collect_snap_segments, ortho_constrain, Tool, ToolContext, ToolResult};
use crate::view::viewport::ViewTransform;

// ---------------------------------------------------------------------------
// Sub-type enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardscapeSubType {
    Wall,
    Path,
    Patio,
    Step,
    Edging,
    RaisedBed,
    Fence,
    Pergola,
    Decking,
    WaterFeature,
}

impl HardscapeSubType {
    pub const ALL: &'static [Self] = &[
        Self::Wall,
        Self::Path,
        Self::Patio,
        Self::Step,
        Self::Edging,
        Self::RaisedBed,
        Self::Fence,
        Self::Pergola,
        Self::Decking,
        Self::WaterFeature,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Wall => "Wall",
            Self::Path => "Path",
            Self::Patio => "Patio",
            Self::Step => "Step",
            Self::Edging => "Edging",
            Self::RaisedBed => "Raised Bed",
            Self::Fence => "Fence",
            Self::Pergola => "Pergola",
            Self::Decking => "Decking",
            Self::WaterFeature => "Water Feature",
        }
    }

    pub fn drawing_mode(&self) -> DrawingMode {
        match self {
            Self::Wall | Self::Path | Self::Step | Self::Edging | Self::Fence => DrawingMode::Polyline,
            Self::Patio | Self::RaisedBed | Self::Decking => DrawingMode::Polygon,
            Self::Pergola => DrawingMode::Rectangle,
            Self::WaterFeature => DrawingMode::PointOrPolygon,
        }
    }

    pub fn target_layer_name(&self) -> &'static str {
        match self {
            Self::Wall | Self::RaisedBed => "GD_HARDSCAPE_WALLS",
            Self::Path | Self::Patio | Self::Decking => "GD_HARDSCAPE_SURFACES",
            Self::Step => "GD_HARDSCAPE_STEPS",
            Self::Edging => "GD_HARDSCAPE_EDGING",
            Self::Fence | Self::Pergola => "GD_HARDSCAPE_STRUCTURES",
            Self::WaterFeature => "GD_HARDSCAPE_WATER",
        }
    }

    pub fn default_colour(&self) -> [u8; 4] {
        match self {
            Self::Wall => parse_hex_colour("#8B7D6B"),
            Self::Path => parse_hex_colour("#C4B8A8"),
            Self::Patio => parse_hex_colour("#C4B8A8"),
            Self::Step => parse_hex_colour("#A0937D"),
            Self::Edging => parse_hex_colour("#6B6158"),
            Self::RaisedBed => parse_hex_colour("#8B7D6B"),
            Self::Fence => parse_hex_colour("#A0522D"),
            Self::Pergola => parse_hex_colour("#A0522D"),
            Self::Decking => parse_hex_colour("#C19A6B"),
            Self::WaterFeature => parse_hex_colour("#3B7DD8"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingMode {
    Polyline,
    Polygon,
    Rectangle,
    Point,
    PointOrPolygon,
}

fn parse_hex_colour(hex: &str) -> [u8; 4] {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128);
    [r, g, b, 255]
}

// ---------------------------------------------------------------------------
// Tool struct
// ---------------------------------------------------------------------------

pub struct PlaceHardscapeTool {
    pub sub_type: HardscapeSubType,
    // Domain defaults
    pub wall: WallData,
    pub path: PathData,
    pub patio: PatioData,
    pub step: StepData,
    pub edging: EdgingData,
    pub raised_bed: RaisedBedData,
    pub fence: FenceData,
    pub pergola: PergolaData,
    pub decking: DeckingData,
    pub water_feature: WaterFeatureData,
    // Drawing state
    vertices: Vec<Vec2>,
    current_pos: Vec2,
    first_corner: Option<Vec2>,
    snap: Option<SnapResult>,
    self_intersects: bool,
    pub layer_id: uuid::Uuid,
}

impl PlaceHardscapeTool {
    pub fn new() -> Self {
        Self {
            sub_type: HardscapeSubType::Wall,
            wall: WallData::default(),
            path: PathData::default(),
            patio: PatioData::default(),
            step: StepData::default(),
            edging: EdgingData::default(),
            raised_bed: RaisedBedData::default(),
            fence: FenceData::default(),
            pergola: PergolaData::default(),
            decking: DeckingData::default(),
            water_feature: WaterFeatureData::default(),
            vertices: Vec::new(),
            current_pos: Vec2::ZERO,
            first_corner: None,
            snap: None,
            self_intersects: false,
            layer_id: uuid::Uuid::nil(),
        }
    }

    pub fn target_layer_name(&self) -> &str {
        self.sub_type.target_layer_name()
    }

    fn effective_mode(&self) -> DrawingMode {
        match self.sub_type.drawing_mode() {
            DrawingMode::PointOrPolygon => {
                match self.water_feature.feature_type {
                    WaterFeatureType::Pond | WaterFeatureType::Rill | WaterFeatureType::Stream => {
                        DrawingMode::Polygon
                    }
                    _ => DrawingMode::Point,
                }
            }
            other => other,
        }
    }

    fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        let constrained = if ctx.shift_held && !self.vertices.is_empty() {
            ortho_constrain(*self.vertices.last().unwrap(), raw)
        } else {
            raw
        };
        if ctx.snap_config.enabled {
            let (segs, centers) = self.snap_data(ctx);
            let candidates = snap::snap_points(constrained, &segs, &centers, ctx.snap_config);
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

    /// Width in mm for the current sub-type, if applicable.
    fn current_width_mm(&self) -> Option<f32> {
        match self.sub_type {
            HardscapeSubType::Wall => Some(self.wall.width_mm as f32),
            HardscapeSubType::Path => Some(self.path.width_mm as f32),
            HardscapeSubType::Step => Some(self.step.width_mm as f32),
            HardscapeSubType::Edging => Some(self.edging.width_mm as f32),
            _ => None,
        }
    }

    fn finish_shape(&mut self) -> ToolResult {
        // Remove near-duplicate consecutive vertices (from double-click, snapping, etc.)
        self.vertices.dedup_by(|a, b| (*a - *b).length() < 1.0);
        if let Some(entity) = self.make_entity() {
            let cmd = Box::new(AddEntityCommand::new(entity));
            self.vertices.clear();
            self.first_corner = None;
            ToolResult::EmitAndContinue(cmd)
        } else {
            ToolResult::Continue
        }
    }

    fn make_entity(&self) -> Option<Entity> {
        let mode = self.effective_mode();
        let geom = match mode {
            DrawingMode::Polyline => {
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
            DrawingMode::Polygon => {
                if self.vertices.len() < 3 {
                    return None;
                }
                Geometry::Polygon(PolygonData {
                    vertices: self.vertices.clone(),
                })
            }
            DrawingMode::Rectangle => {
                let first = self.first_corner?;
                let second = *self.vertices.first()?;
                let center = (first + second) * 0.5;
                let half_size = ((second - first) * 0.5).abs();
                if half_size.x < 1.0 || half_size.y < 1.0 {
                    return None;
                }
                Geometry::Rectangle(RectData {
                    center,
                    half_size,
                    rotation: 0.0,
                })
            }
            DrawingMode::Point | DrawingMode::PointOrPolygon => {
                if self.vertices.is_empty() {
                    return None;
                }
                Geometry::Point(self.vertices[0])
            }
        };

        let domain = match self.sub_type {
            HardscapeSubType::Wall => DomainData::Wall(self.wall.clone()),
            HardscapeSubType::Path => DomainData::Path(self.path.clone()),
            HardscapeSubType::Patio => DomainData::Patio(self.patio.clone()),
            HardscapeSubType::Step => DomainData::Step(self.step.clone()),
            HardscapeSubType::Edging => DomainData::Edging(self.edging.clone()),
            HardscapeSubType::RaisedBed => DomainData::RaisedBed(self.raised_bed.clone()),
            HardscapeSubType::Fence => DomainData::Fence(self.fence.clone()),
            HardscapeSubType::Pergola => DomainData::Pergola(self.pergola.clone()),
            HardscapeSubType::Decking => DomainData::Decking(self.decking.clone()),
            HardscapeSubType::WaterFeature => DomainData::WaterFeature(self.water_feature.clone()),
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

impl Tool for PlaceHardscapeTool {
    fn name(&self) -> &str {
        "Hardscape"
    }

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
        let mode = self.effective_mode();

        match mode {
            DrawingMode::Point | DrawingMode::PointOrPolygon => {
                self.vertices = vec![effective];
                self.finish_shape()
            }
            DrawingMode::Rectangle => {
                if self.first_corner.is_none() {
                    self.first_corner = Some(effective);
                    ToolResult::Continue
                } else {
                    self.vertices = vec![effective];
                    self.finish_shape()
                }
            }
            DrawingMode::Polyline | DrawingMode::Polygon => {
                if self.self_intersects {
                    return ToolResult::Continue;
                }
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
            let candidates = snap::snap_points(constrained, &segments, &centers, ctx.snap_config);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }

        // Check self-intersection for polygon/polyline modes
        let mode = self.effective_mode();
        self.self_intersects = match mode {
            DrawingMode::Polygon => {
                would_self_intersect(&self.vertices, effective, true)
            }
            DrawingMode::Polyline => {
                would_self_intersect(&self.vertices, effective, false)
            }
            _ => false,
        };
    }

    fn on_key_enter(&mut self) -> ToolResult {
        self.finish_shape()
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.vertices.is_empty() && self.first_corner.is_none() {
            ToolResult::Finished
        } else {
            self.vertices.clear();
            self.first_corner = None;
            ToolResult::Continue
        }
    }

    fn on_double_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        let mode = self.effective_mode();
        match mode {
            DrawingMode::Polyline | DrawingMode::Polygon => {
                // The first click of the double-click already added a vertex via on_mouse_down.
                // Remove duplicate trailing vertex if it matches the previous one.
                if self.vertices.len() >= 2 {
                    let last = self.vertices[self.vertices.len() - 1];
                    let prev = self.vertices[self.vertices.len() - 2];
                    if (last - prev).length() < 1.0 {
                        self.vertices.pop();
                    }
                }
                self.finish_shape()
            }
            _ => ToolResult::Continue,
        }
    }

    fn on_right_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        if self.vertices.len() > 1 {
            self.vertices.pop();
            ToolResult::Continue
        } else if !self.vertices.is_empty() || self.first_corner.is_some() {
            self.vertices.clear();
            self.first_corner = None;
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

        let mode = self.effective_mode();

        match mode {
            DrawingMode::Rectangle => {
                if let Some(first) = self.first_corner {
                    let a = viewport.world_to_screen(first);
                    let b = viewport.world_to_screen(self.current_pos);
                    let rect = egui::Rect::from_two_pos(a, b);
                    painter.rect_filled(rect, 0.0, fill_colour);
                    painter.rect_stroke(rect, 0.0, preview_stroke, egui::StrokeKind::Outside);
                    painter.circle_filled(a, 3.0, dot_colour);
                }
            }
            DrawingMode::Point | DrawingMode::PointOrPolygon => {
                let screen = viewport.world_to_screen(self.current_pos);
                painter.circle_filled(screen, 5.0, fill_colour);
                painter.circle_stroke(screen, 5.0, stroke);
                // Label
                painter.text(
                    egui::pos2(screen.x, screen.y + 10.0),
                    egui::Align2::CENTER_TOP,
                    self.sub_type.label(),
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_rgba_unmultiplied(200, 200, 205, 160),
                );
            }
            DrawingMode::Polyline | DrawingMode::Polygon => {
                if !self.vertices.is_empty() {
                    // Width preview for polyline types with domain width
                    if mode == DrawingMode::Polyline {
                        if let Some(w) = self.current_width_mm() {
                            let mut preview_verts = self.vertices.clone();
                            preview_verts.push(self.current_pos);
                            if preview_verts.len() >= 2 {
                                let width_fill = egui::Color32::from_rgba_unmultiplied(
                                    col[0], col[1], col[2], 25,
                                );
                                let width_stroke_col = egui::Color32::from_rgba_unmultiplied(
                                    col[0], col[1], col[2], 80,
                                );
                                crate::view::canvas::draw_width_shape(
                                    painter,
                                    viewport,
                                    &preview_verts,
                                    w,
                                    width_fill,
                                    egui::Stroke::new(1.0, width_stroke_col),
                                );
                            }
                        }
                    }

                    // Draw confirmed segments
                    for w in self.vertices.windows(2) {
                        let a = viewport.world_to_screen(w[0]);
                        let b = viewport.world_to_screen(w[1]);
                        painter.line_segment([a, b], stroke);
                    }
                    // Preview segment to cursor — red if self-intersecting
                    let last = viewport.world_to_screen(*self.vertices.last().unwrap());
                    let cursor = viewport.world_to_screen(self.current_pos);
                    if self.self_intersects {
                        let red_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(220, 60, 60));
                        painter.line_segment([last, cursor], red_stroke);

                        // For polygon mode, closing line in red too
                        if mode == DrawingMode::Polygon && self.vertices.len() >= 2 {
                            let first = viewport.world_to_screen(self.vertices[0]);
                            painter.line_segment([cursor, first], red_stroke);
                        }

                        // Red X near cursor
                        let x_size = 6.0;
                        let x_stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(220, 60, 60));
                        painter.line_segment(
                            [egui::pos2(cursor.x - x_size, cursor.y - x_size),
                             egui::pos2(cursor.x + x_size, cursor.y + x_size)],
                            x_stroke,
                        );
                        painter.line_segment(
                            [egui::pos2(cursor.x + x_size, cursor.y - x_size),
                             egui::pos2(cursor.x - x_size, cursor.y + x_size)],
                            x_stroke,
                        );

                        // Warning label
                        painter.text(
                            egui::pos2(cursor.x, cursor.y + 14.0),
                            egui::Align2::CENTER_TOP,
                            "Lines must not cross",
                            egui::FontId::proportional(11.0),
                            egui::Color32::from_rgb(220, 60, 60),
                        );
                    } else {
                        painter.line_segment([last, cursor], preview_stroke);

                        // For polygon mode, also draw closing line preview
                        if mode == DrawingMode::Polygon && self.vertices.len() >= 2 {
                            let first = viewport.world_to_screen(self.vertices[0]);
                            painter.line_segment(
                                [cursor, first],
                                egui::Stroke::new(
                                    0.5,
                                    egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 60),
                                ),
                            );
                        }
                    }

                    // Vertex dots
                    for v in &self.vertices {
                        let screen = viewport.world_to_screen(*v);
                        painter.circle_filled(screen, 3.0, dot_colour);
                    }
                }
            }
        }

        // Snap indicator
        if let Some(ref snap) = self.snap {
            draw_snap_indicator(painter, viewport, snap);
        }
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        self.snap.as_ref()
    }
}
