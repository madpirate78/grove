use glam::Vec2;

use grove_core::boolean_ops::{circle_to_polygon, rect_to_polygon};
use grove_core::commands::AddEntityCommand;
use grove_core::entity::{
    BezierPathData, CircleData, DomainData, Entity, Geometry, LineData, PolygonData, PolylineData,
    RectData,
};
use grove_core::garden::hardscape::*;
use grove_core::snap::{self, SnapResult};

use super::draw_line::draw_snap_indicator;
use super::polygon_drawer::PolygonDrawer;
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
    House,
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
        Self::House,
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
            Self::House => "House",
        }
    }

    pub fn drawing_mode(&self) -> DrawingMode {
        match self {
            Self::Wall | Self::Path | Self::Step | Self::Edging | Self::Fence => DrawingMode::Polyline,
            Self::Patio | Self::RaisedBed | Self::Decking => DrawingMode::Polygon,
            Self::Pergola | Self::House => DrawingMode::Rectangle,
            Self::WaterFeature => DrawingMode::PointOrPolygon,
        }
    }

    /// Whether this sub-type is an area (polygon) type that supports shape overrides.
    pub fn is_area_type(&self) -> bool {
        matches!(self.drawing_mode(), DrawingMode::Polygon | DrawingMode::PointOrPolygon)
    }

    pub fn target_layer_name(&self) -> &'static str {
        match self {
            Self::Wall | Self::RaisedBed => "GD_HARDSCAPE_WALLS",
            Self::Path | Self::Patio | Self::Decking => "GD_HARDSCAPE_SURFACES",
            Self::Step => "GD_HARDSCAPE_STEPS",
            Self::Edging => "GD_HARDSCAPE_EDGING",
            Self::Fence | Self::Pergola => "GD_HARDSCAPE_STRUCTURES",
            Self::WaterFeature => "GD_HARDSCAPE_WATER",
            Self::House => "GD_BUILDINGS",
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
            Self::House => parse_hex_colour("#8B7355"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingMode {
    Polyline,
    Polygon,
    Rectangle,
    Circle,
    Point,
    PointOrPolygon,
}

/// Shape override for area hardscape types (Patio, Decking, RaisedBed).
/// Allows switching between polygon, circle, and rectangle drawing sub-modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeOverride {
    /// Polygon drawer (polyline with curve mode) — the default for area types.
    Line,
    /// Circle: click center, click edge.
    Circle,
    /// Rectangle: click corner, click opposite corner.
    Rectangle,
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
    /// Shape override for area types (None = use sub_type default).
    pub shape_override: Option<ShapeOverride>,
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
    pub house: HouseData,
    // Drawing state — shared drawer for polygon/polyline modes
    drawer: PolygonDrawer,
    // Rectangle mode state (not using drawer)
    current_pos: Vec2,
    first_corner: Option<Vec2>,
    // Circle mode state
    circle_center: Option<Vec2>,
    snap: Option<SnapResult>,
    pub layer_id: uuid::Uuid,
}

impl PlaceHardscapeTool {
    pub fn new() -> Self {
        Self {
            sub_type: HardscapeSubType::Wall,
            shape_override: None,
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
            house: HouseData::default(),
            drawer: PolygonDrawer::new(true), // default polygon (closing) mode
            current_pos: Vec2::ZERO,
            first_corner: None,
            circle_center: None,
            snap: None,
            layer_id: uuid::Uuid::nil(),
        }
    }

    pub fn target_layer_name(&self) -> &str {
        self.sub_type.target_layer_name()
    }

    fn effective_mode(&self) -> DrawingMode {
        let base = match self.sub_type.drawing_mode() {
            DrawingMode::PointOrPolygon => {
                match self.water_feature.feature_type {
                    WaterFeatureType::Pond | WaterFeatureType::Rill | WaterFeatureType::Stream => {
                        DrawingMode::Polygon
                    }
                    _ => DrawingMode::Point,
                }
            }
            other => other,
        };

        // Apply shape override for area types
        if let Some(ovr) = self.shape_override {
            if base == DrawingMode::Polygon {
                return match ovr {
                    ShapeOverride::Line => DrawingMode::Polygon,
                    ShapeOverride::Circle => DrawingMode::Circle,
                    ShapeOverride::Rectangle => DrawingMode::Rectangle,
                };
            }
        }
        base
    }

    /// Clear drawing-in-progress state (used when switching shape override).
    pub fn reset_drawing_state(&mut self) {
        self.drawer.clear();
        self.first_corner = None;
        self.circle_center = None;
    }

    /// Update the drawer's closing mode based on the current drawing mode.
    fn sync_drawer_mode(&mut self) {
        let mode = self.effective_mode();
        self.drawer.set_closing_mode(mode == DrawingMode::Polygon);
    }

    fn effective_pos_rect(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        let constrained = if ctx.shift_held && self.first_corner.is_some() {
            ortho_constrain(self.first_corner.unwrap(), raw)
        } else {
            raw
        };
        if ctx.snap_config.enabled {
            let (segs, centers) = self.snap_data_rect(ctx);
            let candidates = snap::snap_points(constrained, &segs, &centers, ctx.snap_config);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }
        constrained
    }

    fn snap_data_rect(&self, ctx: &ToolContext) -> (Vec<(Vec2, Vec2)>, Vec<snap::CenterPoint>) {
        let mut segs = Vec::new();
        let mut centers = Vec::new();
        for entity in ctx.entities {
            collect_snap_segments(&entity.geometry, &mut segs);
            collect_snap_centers(&entity.geometry, &mut centers);
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
        if let Some(entity) = self.make_entity() {
            let cmd = Box::new(AddEntityCommand::new(entity));
            self.drawer.clear();
            self.first_corner = None;
            self.circle_center = None;
            ToolResult::EmitAndContinue(cmd)
        } else {
            ToolResult::Continue
        }
    }

    fn make_entity(&self) -> Option<Entity> {
        let mode = self.effective_mode();
        let geom = match mode {
            DrawingMode::Polyline => {
                if self.drawer.anchors.len() < 2 {
                    return None;
                }
                if self.drawer.has_curves() {
                    Geometry::BezierPath(BezierPathData {
                        anchors: self.drawer.anchors.clone(),
                        closed: false,
                        holes: Vec::new(),
                    })
                } else if self.drawer.anchors.len() == 2 {
                    Geometry::Line(LineData {
                        start: self.drawer.anchors[0].point,
                        end: self.drawer.anchors[1].point,
                    })
                } else {
                    Geometry::Polyline(PolylineData {
                        vertices: self.drawer.to_vertices(),
                        closed: false,
                    })
                }
            }
            DrawingMode::Polygon => {
                if self.drawer.anchors.len() < 3 {
                    return None;
                }
                if self.drawer.has_curves() {
                    Geometry::BezierPath(BezierPathData {
                        anchors: self.drawer.anchors.clone(),
                        closed: true,
                        holes: Vec::new(),
                    })
                } else {
                    Geometry::Polygon(PolygonData {
                        vertices: self.drawer.to_vertices(),
                        holes: Vec::new(),
                    })
                }
            }
            DrawingMode::Circle => {
                let center = self.circle_center?;
                let radius = (self.current_pos - center).length();
                if radius < 1.0 {
                    return None;
                }
                if self.sub_type.is_area_type() {
                    let vertices = circle_to_polygon(center, radius, 48);
                    Geometry::Polygon(PolygonData { vertices, holes: Vec::new() })
                } else {
                    Geometry::Circle(CircleData { center, radius })
                }
            }
            DrawingMode::Rectangle => {
                let first = self.first_corner?;
                let second = self.current_pos;
                let center = (first + second) * 0.5;
                let half_size = ((second - first) * 0.5).abs();
                if half_size.x < 1.0 || half_size.y < 1.0 {
                    return None;
                }
                if self.sub_type.is_area_type() {
                    let vertices = rect_to_polygon(center, half_size);
                    Geometry::Polygon(PolygonData { vertices, holes: Vec::new() })
                } else {
                    Geometry::Rectangle(RectData {
                        center,
                        half_size,
                        rotation: 0.0,
                    })
                }
            }
            DrawingMode::Point | DrawingMode::PointOrPolygon => {
                if self.drawer.anchors.is_empty() {
                    return None;
                }
                Geometry::Point(self.drawer.anchors[0].point)
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
            HardscapeSubType::House => DomainData::House(self.house.clone()),
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

    fn as_any_ref(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = if self.layer_id.is_nil() {
            ctx.active_layer_id
        } else {
            self.layer_id
        };
        self.sync_drawer_mode();
        let mode = self.effective_mode();

        match mode {
            DrawingMode::Point | DrawingMode::PointOrPolygon => {
                let effective = self.drawer.effective_pos(pos, ctx);
                self.drawer.anchors = vec![grove_core::entity::BezierAnchor {
                    point: effective,
                    handle_in: None,
                    handle_out: None,
                }];
                self.finish_shape()
            }
            DrawingMode::Circle => {
                let effective = self.effective_pos_rect(pos, ctx);
                if self.circle_center.is_none() {
                    self.circle_center = Some(effective);
                    ToolResult::Continue
                } else {
                    self.current_pos = effective;
                    self.finish_shape()
                }
            }
            DrawingMode::Rectangle => {
                let effective = self.effective_pos_rect(pos, ctx);
                if self.first_corner.is_none() {
                    self.first_corner = Some(effective);
                    ToolResult::Continue
                } else {
                    self.current_pos = effective;
                    self.finish_shape()
                }
            }
            DrawingMode::Polyline | DrawingMode::Polygon => {
                self.drawer.add_anchor(pos, ctx);
                ToolResult::Continue
            }
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        self.sync_drawer_mode();
        let mode = self.effective_mode();

        match mode {
            DrawingMode::Circle | DrawingMode::Rectangle | DrawingMode::Point | DrawingMode::PointOrPolygon => {
                let effective = self.effective_pos_rect(pos, ctx);
                self.current_pos = effective;

                if ctx.snap_config.enabled {
                    let constrained = if ctx.shift_held && self.first_corner.is_some() {
                        ortho_constrain(self.first_corner.unwrap(), pos)
                    } else {
                        pos
                    };
                    let (segments, centers) = self.snap_data_rect(ctx);
                    let candidates = snap::snap_points(constrained, &segments, &centers, ctx.snap_config);
                    let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
                    self.snap = snap::best_snap(&candidates, tolerance);
                } else {
                    self.snap = None;
                }
            }
            DrawingMode::Polyline | DrawingMode::Polygon => {
                self.drawer.update_cursor(pos, ctx);
            }
        }
    }

    fn on_mouse_up(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        let mode = self.effective_mode();
        if matches!(mode, DrawingMode::Polyline | DrawingMode::Polygon) {
            self.drawer.finalize_handle();
        }
        ToolResult::Continue
    }

    fn on_key_pressed(&mut self, key: egui::Key) -> ToolResult {
        if key == egui::Key::C {
            self.drawer.toggle_curve_mode();
        }
        ToolResult::Continue
    }

    fn on_key_enter(&mut self) -> ToolResult {
        self.finish_shape()
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.drawer.anchors.is_empty() && self.first_corner.is_none() && self.circle_center.is_none() {
            ToolResult::Finished
        } else {
            self.drawer.clear();
            self.first_corner = None;
            self.circle_center = None;
            ToolResult::Continue
        }
    }

    fn on_double_click(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        let mode = self.effective_mode();
        match mode {
            DrawingMode::Polyline | DrawingMode::Polygon => {
                self.drawer.drag_anchor_pos = None;
                self.drawer.dragging_handle = None;

                // Remove duplicate trailing anchor from the double-click's mouse_down
                if self.drawer.anchors.len() >= 2 {
                    let last = self.drawer.anchors[self.drawer.anchors.len() - 1].point;
                    let prev = self.drawer.anchors[self.drawer.anchors.len() - 2].point;
                    if (last - prev).length() < 1.0 {
                        self.drawer.anchors.pop();
                    }
                }

                // Check close-on-first for polygon mode
                if mode == DrawingMode::Polygon {
                    let tolerance = 8.0 / ctx.viewport_zoom;
                    let _ = pos;
                    if self.drawer.anchors.len() >= 4 {
                        let last = self.drawer.anchors.last().unwrap().point;
                        if (last - self.drawer.anchors[0].point).length() <= tolerance {
                            self.drawer.anchors.pop();
                        }
                    }
                }

                self.finish_shape()
            }
            _ => ToolResult::Continue,
        }
    }

    fn on_right_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        let mode = self.effective_mode();
        match mode {
            DrawingMode::Polyline | DrawingMode::Polygon => {
                if self.drawer.anchors.len() > 1 {
                    self.drawer.undo_last();
                    ToolResult::Continue
                } else if !self.drawer.anchors.is_empty() {
                    self.drawer.clear();
                    ToolResult::Continue
                } else {
                    ToolResult::Finished
                }
            }
            _ => {
                if self.first_corner.is_some() || self.circle_center.is_some() {
                    self.first_corner = None;
                    self.circle_center = None;
                    ToolResult::Continue
                } else {
                    ToolResult::Finished
                }
            }
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let col = self.colour();
        let mode = self.effective_mode();

        match mode {
            DrawingMode::Circle => {
                let preview_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 140));
                let fill_colour = egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 30);
                let dot_colour = egui::Color32::from_rgb(col[0], col[1], col[2]);

                if let Some(center) = self.circle_center {
                    let screen_center = viewport.world_to_screen(center);
                    let screen_edge = viewport.world_to_screen(self.current_pos);
                    let screen_radius = screen_center.distance(screen_edge);

                    painter.circle_filled(screen_center, screen_radius, fill_colour);
                    painter.circle_stroke(screen_center, screen_radius, preview_stroke);

                    // Radius line
                    painter.line_segment(
                        [screen_center, screen_edge],
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 80)),
                    );

                    painter.circle_filled(screen_center, 3.0, dot_colour);
                }
            }
            DrawingMode::Rectangle => {
                let fill_colour = egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 40);
                let preview_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 120));
                let dot_colour = egui::Color32::from_rgb(col[0], col[1], col[2]);

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
                let stroke_colour = egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 180);
                let fill_colour = egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], 40);
                let stroke = egui::Stroke::new(1.5, stroke_colour);

                let screen = viewport.world_to_screen(self.current_pos);
                painter.circle_filled(screen, 5.0, fill_colour);
                painter.circle_stroke(screen, 5.0, stroke);
                painter.text(
                    egui::pos2(screen.x, screen.y + 10.0),
                    egui::Align2::CENTER_TOP,
                    self.sub_type.label(),
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_rgba_unmultiplied(200, 200, 205, 160),
                );
            }
            DrawingMode::Polyline | DrawingMode::Polygon => {
                let is_polygon = mode == DrawingMode::Polygon;

                // Width preview for polyline types with domain width
                if mode == DrawingMode::Polyline && !self.drawer.anchors.is_empty() {
                    if let Some(w) = self.current_width_mm() {
                        let mut preview_verts = self.drawer.to_vertices();
                        preview_verts.push(self.drawer.current_pos);
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

                self.drawer.draw_preview(painter, viewport, col, is_polygon, true);
            }
        }

        // Snap indicator (for rectangle/point modes that don't use the drawer)
        let snap_ref = match mode {
            DrawingMode::Polyline | DrawingMode::Polygon => self.drawer.snap.as_ref(),
            _ => self.snap.as_ref(),
        };
        if let Some(snap) = snap_ref {
            draw_snap_indicator(painter, viewport, snap);
        }
    }

    fn snap_result(&self) -> Option<&SnapResult> {
        let mode = self.effective_mode();
        match mode {
            DrawingMode::Polyline | DrawingMode::Polygon => self.drawer.snap.as_ref(),
            _ => self.snap.as_ref(),
        }
    }
}
