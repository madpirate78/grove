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
// Category / sub-kind enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardscapeCategory {
    Walls,
    Surfaces,
    Steps,
    Structures,
    Water,
    Buildings,
}

impl HardscapeCategory {
    pub const ALL: &'static [Self] = &[
        Self::Walls,
        Self::Surfaces,
        Self::Steps,
        Self::Structures,
        Self::Water,
        Self::Buildings,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Walls => "Walls",
            Self::Surfaces => "Surfaces",
            Self::Steps => "Steps",
            Self::Structures => "Structures",
            Self::Water => "Water",
            Self::Buildings => "Buildings",
        }
    }

    pub fn target_layer_name(&self) -> &'static str {
        match self {
            Self::Walls => "GD_HARDSCAPE_WALLS",
            Self::Surfaces => "GD_HARDSCAPE_SURFACES",
            Self::Steps => "GD_HARDSCAPE_STEPS",
            Self::Structures => "GD_HARDSCAPE_STRUCTURES",
            Self::Water => "GD_HARDSCAPE_WATER",
            Self::Buildings => "GD_BUILDINGS",
        }
    }

    pub fn default_colour(&self) -> [u8; 4] {
        match self {
            Self::Walls => parse_hex_colour("#8B7D6B"),
            Self::Surfaces => parse_hex_colour("#C4B8A8"),
            Self::Steps => parse_hex_colour("#A0937D"),
            Self::Structures => parse_hex_colour("#A0522D"),
            Self::Water => parse_hex_colour("#3B7DD8"),
            Self::Buildings => parse_hex_colour("#8B7355"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallKind {
    Wall,
    Edging,
}

impl WallKind {
    pub const ALL: &'static [Self] = &[Self::Wall, Self::Edging];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Wall => "Wall",
            Self::Edging => "Edging",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureKind {
    Fence,
    Pergola,
}

impl StructureKind {
    pub const ALL: &'static [Self] = &[Self::Fence, Self::Pergola];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Fence => "Fence",
            Self::Pergola => "Pergola",
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

/// Shape override for area types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeOverride {
    Line,
    Circle,
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
    pub category: HardscapeCategory,
    pub wall_kind: WallKind,
    pub structure_kind: StructureKind,
    /// Shape override for area types (None = use category default).
    pub shape_override: Option<ShapeOverride>,
    // Domain defaults
    pub wall: WallData,
    pub edging: EdgingData,
    pub surface: SurfaceData,
    pub step: StepData,
    pub fence: FenceData,
    pub pergola: PergolaData,
    pub water_feature: WaterFeatureData,
    pub house: HouseData,
    // Drawing state
    drawer: PolygonDrawer,
    current_pos: Vec2,
    first_corner: Option<Vec2>,
    circle_center: Option<Vec2>,
    snap: Option<SnapResult>,
    pub layer_id: uuid::Uuid,
}

impl PlaceHardscapeTool {
    pub fn new() -> Self {
        Self {
            category: HardscapeCategory::Walls,
            wall_kind: WallKind::Wall,
            structure_kind: StructureKind::Fence,
            shape_override: None,
            wall: WallData::default(),
            edging: EdgingData::default(),
            surface: SurfaceData::default(),
            step: StepData::default(),
            fence: FenceData::default(),
            pergola: PergolaData::default(),
            water_feature: WaterFeatureData::default(),
            house: HouseData::default(),
            drawer: PolygonDrawer::new(true),
            current_pos: Vec2::ZERO,
            first_corner: None,
            circle_center: None,
            snap: None,
            layer_id: uuid::Uuid::nil(),
        }
    }

    pub fn target_layer_name(&self) -> &str {
        self.category.target_layer_name()
    }

    fn base_drawing_mode(&self) -> DrawingMode {
        match self.category {
            HardscapeCategory::Walls => DrawingMode::Polyline,
            HardscapeCategory::Surfaces => {
                if self.surface.width_mm.is_some() {
                    DrawingMode::Polyline
                } else {
                    DrawingMode::Polygon
                }
            }
            HardscapeCategory::Steps => DrawingMode::Polyline,
            HardscapeCategory::Structures => {
                match self.structure_kind {
                    StructureKind::Fence => DrawingMode::Polyline,
                    StructureKind::Pergola => DrawingMode::Rectangle,
                }
            }
            HardscapeCategory::Water => DrawingMode::PointOrPolygon,
            HardscapeCategory::Buildings => DrawingMode::Polygon,
        }
    }

    fn effective_mode(&self) -> DrawingMode {
        let base = match self.base_drawing_mode() {
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

    /// Whether the current config produces an area (polygon) type.
    pub fn is_area_type(&self) -> bool {
        matches!(self.base_drawing_mode(), DrawingMode::Polygon | DrawingMode::PointOrPolygon)
    }

    pub fn reset_drawing_state(&mut self) {
        self.drawer.clear();
        self.first_corner = None;
        self.circle_center = None;
    }

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
        match self.category {
            HardscapeCategory::Surfaces => self.surface.default_colour(),
            _ => self.category.default_colour(),
        }
    }

    fn current_width_mm(&self) -> Option<f32> {
        match self.category {
            HardscapeCategory::Walls => {
                match self.wall_kind {
                    WallKind::Wall => Some(self.wall.width_mm as f32),
                    WallKind::Edging => Some(self.edging.width_mm as f32),
                }
            }
            HardscapeCategory::Surfaces => self.surface.width_mm.map(|w| w as f32),
            HardscapeCategory::Steps => Some(self.step.width_mm as f32),
            _ => None,
        }
    }

    /// Entity display name for the current selection.
    fn entity_name(&self) -> &str {
        match self.category {
            HardscapeCategory::Walls => self.wall_kind.label(),
            HardscapeCategory::Surfaces => self.surface.label.label(),
            HardscapeCategory::Steps => "Step",
            HardscapeCategory::Structures => self.structure_kind.label(),
            HardscapeCategory::Water => "Water Feature",
            HardscapeCategory::Buildings => "House",
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
                if self.is_area_type() {
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
                if self.is_area_type() {
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

        let domain = match self.category {
            HardscapeCategory::Walls => {
                match self.wall_kind {
                    WallKind::Wall => DomainData::Wall(self.wall.clone()),
                    WallKind::Edging => DomainData::Edging(self.edging.clone()),
                }
            }
            HardscapeCategory::Surfaces => DomainData::Surface(self.surface.clone()),
            HardscapeCategory::Steps => DomainData::Step(self.step.clone()),
            HardscapeCategory::Structures => {
                match self.structure_kind {
                    StructureKind::Fence => DomainData::Fence(self.fence.clone()),
                    StructureKind::Pergola => DomainData::Pergola(self.pergola.clone()),
                }
            }
            HardscapeCategory::Water => DomainData::WaterFeature(self.water_feature.clone()),
            HardscapeCategory::Buildings => DomainData::House(self.house.clone()),
        };

        let mut entity = Entity::new(self.entity_name(), geom, domain);
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
        match key {
            egui::Key::C => self.drawer.toggle_curve_mode(),
            egui::Key::E => self.drawer.toggle_edge_trace(),
            egui::Key::Tab => self.drawer.edge_trace_flip(),
            _ => {}
        }
        ToolResult::Continue
    }

    fn on_key_enter(&mut self) -> ToolResult {
        self.finish_shape()
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.drawer.edge_trace_cancel() {
            return ToolResult::Continue;
        }
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

                if self.drawer.anchors.len() >= 2 {
                    let last = self.drawer.anchors[self.drawer.anchors.len() - 1].point;
                    let prev = self.drawer.anchors[self.drawer.anchors.len() - 2].point;
                    if (last - prev).length() < 1.0 {
                        self.drawer.anchors.pop();
                    }
                }

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
                    self.entity_name(),
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_rgba_unmultiplied(200, 200, 205, 160),
                );
            }
            DrawingMode::Polyline | DrawingMode::Polygon => {
                let is_polygon = mode == DrawingMode::Polygon;

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
                            crate::view::canvas::render_width_shape(
                                painter,
                                &preview_verts,
                                w,
                                width_fill,
                                egui::Stroke::new(1.0, width_stroke_col),
                                |v| viewport.world_to_screen(v),
                            );
                        }
                    }
                }

                self.drawer.draw_preview(painter, viewport, col, is_polygon, true);
            }
        }

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
