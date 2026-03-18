use glam::Vec2;
use uuid::Uuid;

use grove_core::commands::{
    CompoundCommand, MoveEntityCommand, RotateEntityCommand, ScaleEntityCommand,
};
use grove_core::entity::{Entity, Geometry};
use grove_core::snap::{self, SnapResult};

use super::{collect_snap_centers, collect_snap_segments, Tool, ToolContext, ToolResult};
use crate::tools::draw_line::draw_snap_indicator;
use crate::view::viewport::ViewTransform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifyMode {
    Move,
    Rotate,
    Scale,
}

enum Phase {
    PickBase,
    Active {
        base: Vec2,
        initial_angle: f32,
        initial_distance: f32,
        input: String,
    },
}

pub struct ModifyTool {
    mode: ModifyMode,
    phase: Phase,
    target_ids: Vec<Uuid>,
    original_geometries: Vec<(Uuid, Geometry)>,
    current_pos: Vec2,
    snap: Option<SnapResult>,
}

impl ModifyTool {
    pub fn mode(&self) -> ModifyMode {
        self.mode
    }

    pub fn new(mode: ModifyMode, target_ids: Vec<Uuid>, entities: &[Entity]) -> Self {
        let original_geometries: Vec<_> = target_ids
            .iter()
            .filter_map(|id| {
                entities
                    .iter()
                    .find(|e| e.id == *id)
                    .map(|e| (*id, e.geometry.clone()))
            })
            .collect();

        Self {
            mode,
            phase: Phase::PickBase,
            target_ids,
            original_geometries,
            current_pos: Vec2::ZERO,
            snap: None,
        }
    }

    /// Create a ModifyTool that starts directly in the Active phase with a pre-set base point.
    /// Used when the anchor point is set and the mode is Rotate or Scale.
    pub fn new_with_base(mode: ModifyMode, target_ids: Vec<Uuid>, entities: &[Entity], base: Vec2) -> Self {
        let original_geometries: Vec<_> = target_ids
            .iter()
            .filter_map(|id| {
                entities
                    .iter()
                    .find(|e| e.id == *id)
                    .map(|e| (*id, e.geometry.clone()))
            })
            .collect();

        Self {
            mode,
            phase: Phase::Active {
                base,
                initial_angle: 0.0,
                initial_distance: 0.0,
                input: String::new(),
            },
            target_ids,
            original_geometries,
            current_pos: Vec2::ZERO,
            snap: None,
        }
    }

    fn snap_data(ctx: &ToolContext) -> (Vec<(Vec2, Vec2)>, Vec<snap::CenterPoint>) {
        let mut segs = Vec::new();
        let mut centers = Vec::new();
        for entity in ctx.entities {
            collect_snap_segments(&entity.geometry, &mut segs);
            collect_snap_centers(&entity.geometry, &mut centers);
        }
        (segs, centers)
    }

    fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        // Ortho constraint (shift) for Move mode in Active phase
        let constrained = if ctx.shift_held && self.mode == ModifyMode::Move {
            if let Phase::Active { base, .. } = &self.phase {
                super::ortho_constrain(*base, raw)
            } else {
                raw
            }
        } else {
            raw
        };

        if ctx.snap_config.enabled {
            let (segs, centers) = Self::snap_data(ctx);
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

    fn update_snap(&mut self, raw: Vec2, ctx: &ToolContext) {
        if ctx.snap_config.enabled {
            let constrained = if ctx.shift_held && self.mode == ModifyMode::Move {
                if let Phase::Active { base, .. } = &self.phase {
                    super::ortho_constrain(*base, raw)
                } else {
                    raw
                }
            } else {
                raw
            };
            let (segs, centers) = Self::snap_data(ctx);
            let mut candidates = snap::snap_points(constrained, &segs, &centers, ctx.snap_config);
            if !ctx.smart_points.is_empty() {
                candidates.extend(super::extension_snap_candidates(constrained, ctx.smart_points));
            }
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }
    }

    fn compute_transform(&self, base: Vec2, shift: bool) -> TransformValue {
        match self.mode {
            ModifyMode::Move => {
                let mut delta = self.current_pos - base;
                if shift {
                    // Constrain to axis
                    if delta.x.abs() >= delta.y.abs() {
                        delta.y = 0.0;
                    } else {
                        delta.x = 0.0;
                    }
                }
                TransformValue::Move(delta)
            }
            ModifyMode::Rotate => {
                let d = self.current_pos - base;
                let angle = d.y.atan2(d.x);
                let mut angle = angle - if let Phase::Active { initial_angle, .. } = &self.phase {
                    *initial_angle
                } else {
                    0.0
                };
                if shift {
                    // Snap to 15-degree increments
                    let step = 15.0_f32.to_radians();
                    angle = (angle / step).round() * step;
                }
                TransformValue::Rotate(base, angle)
            }
            ModifyMode::Scale => {
                let dist = (self.current_pos - base).length();
                let initial = if let Phase::Active { initial_distance, .. } = &self.phase {
                    *initial_distance
                } else {
                    1.0
                };
                let mut factor = if initial > 0.001 { dist / initial } else { 1.0 };
                if shift {
                    // Snap to 0.25 increments
                    factor = (factor / 0.25).round() * 0.25;
                }
                factor = factor.max(0.01);
                TransformValue::Scale(base, factor)
            }
        }
    }

    fn build_command(&self, transform: TransformValue) -> Box<dyn grove_core::commands::Command> {
        let commands: Vec<Box<dyn grove_core::commands::Command>> = self
            .target_ids
            .iter()
            .map(|&id| -> Box<dyn grove_core::commands::Command> {
                match transform {
                    TransformValue::Move(delta) => Box::new(MoveEntityCommand::new(id, delta)),
                    TransformValue::Rotate(center, angle) => {
                        Box::new(RotateEntityCommand::new(id, center, angle))
                    }
                    TransformValue::Scale(center, factor) => {
                        Box::new(ScaleEntityCommand::new(id, center, factor))
                    }
                }
            })
            .collect();

        let desc = match self.mode {
            ModifyMode::Move => "Move",
            ModifyMode::Rotate => "Rotate",
            ModifyMode::Scale => "Scale",
        };

        if commands.len() == 1 {
            commands.into_iter().next().unwrap()
        } else {
            Box::new(CompoundCommand::new(commands, desc))
        }
    }

    fn parse_numeric_input(input: &str, mode: ModifyMode) -> Option<TransformValue> {
        let val: f32 = input.parse().ok()?;
        match mode {
            ModifyMode::Move => None, // Move needs direction, handled separately
            ModifyMode::Rotate => Some(TransformValue::Rotate(Vec2::ZERO, val.to_radians())),
            ModifyMode::Scale => Some(TransformValue::Scale(Vec2::ZERO, val)),
        }
    }

    pub fn status_hint(&self) -> &'static str {
        match (&self.mode, &self.phase) {
            (ModifyMode::Move, Phase::PickBase) => "G: Click base point",
            (ModifyMode::Move, Phase::Active { .. }) => "G: Click destination (type mm)",
            (ModifyMode::Rotate, Phase::PickBase) => "R: Click rotation center",
            (ModifyMode::Rotate, Phase::Active { .. }) => "R: Drag to rotate (type degrees)",
            (ModifyMode::Scale, Phase::PickBase) => "S: Click scale center",
            (ModifyMode::Scale, Phase::Active { .. }) => "S: Drag to scale (type factor)",
        }
    }
}

#[derive(Clone, Copy)]
enum TransformValue {
    Move(Vec2),
    Rotate(Vec2, f32),
    Scale(Vec2, f32),
}

impl Tool for ModifyTool {
    fn name(&self) -> &str {
        match self.mode {
            ModifyMode::Move => "Move",
            ModifyMode::Rotate => "Rotate",
            ModifyMode::Scale => "Scale",
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any_ref(&self) -> &dyn std::any::Any {
        self
    }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        let snapped = self.effective_pos(pos, ctx);
        match &self.phase {
            Phase::PickBase => {
                self.phase = Phase::Active {
                    base: snapped,
                    initial_angle: 0.0,
                    initial_distance: 0.0,
                    input: String::new(),
                };
                ToolResult::Continue
            }
            Phase::Active { base, input, .. } => {
                let base = *base;
                // If numeric input, use it
                if !input.is_empty() {
                    if let Some(mut tv) = Self::parse_numeric_input(input, self.mode) {
                        match &mut tv {
                            TransformValue::Rotate(c, _) => *c = base,
                            TransformValue::Scale(c, _) => *c = base,
                            _ => {}
                        }
                        let cmd = self.build_command(tv);
                        return ToolResult::Emit(cmd);
                    }
                }
                // Use snapped position for the confirm click
                self.current_pos = snapped;
                let transform = self.compute_transform(base, ctx.shift_held);
                let cmd = self.build_command(transform);
                ToolResult::Emit(cmd)
            }
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        let snapped = self.effective_pos(pos, ctx);
        self.current_pos = snapped;
        self.update_snap(pos, ctx);

        // Set initial angle/distance on first movement after picking base
        if let Phase::Active {
            base,
            initial_angle,
            initial_distance,
            ..
        } = &mut self.phase
        {
            if *initial_angle == 0.0 && *initial_distance == 0.0 {
                let d = snapped - *base;
                if d.length() > 1.0 {
                    *initial_angle = d.y.atan2(d.x);
                    *initial_distance = d.length();
                }
            }
        }
    }

    fn on_key_enter(&mut self) -> ToolResult {
        if let Phase::Active { base, input, .. } = &self.phase {
            let base = *base;
            if !input.is_empty() {
                if let Some(mut tv) = Self::parse_numeric_input(input, self.mode) {
                    match &mut tv {
                        TransformValue::Rotate(c, _) => *c = base,
                        TransformValue::Scale(c, _) => *c = base,
                        _ => {}
                    }
                    let cmd = self.build_command(tv);
                    return ToolResult::Emit(cmd);
                }
            }
            // For move with numeric: treat as distance along current direction
            if self.mode == ModifyMode::Move && !input.is_empty() {
                if let Ok(dist) = input.parse::<f32>() {
                    let dir = (self.current_pos - base).normalize_or_zero();
                    let delta = dir * dist;
                    let cmd = self.build_command(TransformValue::Move(delta));
                    return ToolResult::Emit(cmd);
                }
            }
        }
        ToolResult::Continue
    }

    fn on_key_escape(&mut self) -> ToolResult {
        ToolResult::Finished
    }

    fn on_text_input(&mut self, text: &str) -> bool {
        if let Phase::Active { input, .. } = &mut self.phase {
            if text == "\u{8}" {
                // Backspace
                input.pop();
                return true;
            }
            // Accept digits, decimal point, minus sign
            for c in text.chars() {
                if c.is_ascii_digit() || c == '.' || (c == '-' && input.is_empty()) {
                    input.push(c);
                }
            }
            true
        } else {
            false
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        if let Phase::Active { base, input, .. } = &self.phase {
            let base_screen = viewport.world_to_screen(*base);

            // Draw base point marker (crosshair)
            let size = 8.0;
            let marker_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 200, 50));
            painter.line_segment(
                [
                    egui::pos2(base_screen.x - size, base_screen.y),
                    egui::pos2(base_screen.x + size, base_screen.y),
                ],
                marker_stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(base_screen.x, base_screen.y - size),
                    egui::pos2(base_screen.x, base_screen.y + size),
                ],
                marker_stroke,
            );

            // Draw ghost outlines of transformed geometries
            let preview_stroke =
                egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(100, 200, 255, 160));

            // Compute transform - use shift_held=false for preview (we don't have ctx here)
            let transform = match self.mode {
                ModifyMode::Move => {
                    TransformValue::Move(self.current_pos - *base)
                }
                ModifyMode::Rotate => {
                    let d = self.current_pos - *base;
                    let angle = d.y.atan2(d.x);
                    let initial = if let Phase::Active { initial_angle, .. } = &self.phase {
                        *initial_angle
                    } else {
                        0.0
                    };
                    TransformValue::Rotate(*base, angle - initial)
                }
                ModifyMode::Scale => {
                    let dist = (self.current_pos - *base).length();
                    let initial = if let Phase::Active { initial_distance, .. } = &self.phase {
                        *initial_distance
                    } else {
                        1.0
                    };
                    let factor = if initial > 0.001 { dist / initial } else { 1.0 };
                    TransformValue::Scale(*base, factor.max(0.01))
                }
            };

            for (_, geom) in &self.original_geometries {
                let mut preview_geom = geom.clone();
                match transform {
                    TransformValue::Move(delta) => preview_geom.translate(delta),
                    TransformValue::Rotate(center, angle) => preview_geom.rotate(center, angle),
                    TransformValue::Scale(center, factor) => preview_geom.scale(center, factor),
                }
                draw_geometry_outline(painter, viewport, &preview_geom, preview_stroke);
            }

            // Show constraint guide
            match self.mode {
                ModifyMode::Move => {
                    // Dashed line from base to cursor
                    let cursor_screen = viewport.world_to_screen(self.current_pos);
                    let guide_stroke = egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 200, 50, 100),
                    );
                    painter.line_segment([base_screen, cursor_screen], guide_stroke);
                }
                ModifyMode::Rotate => {
                    // Arc sweep indicator
                    let cursor_screen = viewport.world_to_screen(self.current_pos);
                    let guide_stroke = egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 200, 50, 100),
                    );
                    painter.line_segment([base_screen, cursor_screen], guide_stroke);
                }
                ModifyMode::Scale => {
                    let cursor_screen = viewport.world_to_screen(self.current_pos);
                    let guide_stroke = egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 200, 50, 100),
                    );
                    painter.line_segment([base_screen, cursor_screen], guide_stroke);
                }
            }

            // Draw snap indicator
            if let Some(ref snap) = self.snap {
                draw_snap_indicator(painter, viewport, snap);
            }

            // Show numeric input near cursor if typing
            if !input.is_empty() {
                let cursor_screen = viewport.world_to_screen(self.current_pos);
                let label = match self.mode {
                    ModifyMode::Move => format!("{} mm", input),
                    ModifyMode::Rotate => format!("{}°", input),
                    ModifyMode::Scale => format!("×{}", input),
                };
                painter.text(
                    egui::pos2(cursor_screen.x + 15.0, cursor_screen.y - 15.0),
                    egui::Align2::LEFT_BOTTOM,
                    label,
                    egui::FontId::proportional(14.0),
                    egui::Color32::from_rgb(255, 200, 50),
                );
            }
        }
    }
    fn snap_result(&self) -> Option<&SnapResult> {
        self.snap.as_ref()
    }
}

/// Draw a simple outline of geometry for preview purposes.
pub fn draw_geometry_outline(
    painter: &egui::Painter,
    viewport: &ViewTransform,
    geom: &Geometry,
    stroke: egui::Stroke,
) {
    match geom {
        Geometry::None => {}
        Geometry::Point(p) => {
            let s = viewport.world_to_screen(*p);
            painter.circle_stroke(s, 3.0, stroke);
        }
        Geometry::Line(l) => {
            let a = viewport.world_to_screen(l.start);
            let b = viewport.world_to_screen(l.end);
            painter.line_segment([a, b], stroke);
        }
        Geometry::Polyline(pl) => {
            for w in pl.vertices.windows(2) {
                let a = viewport.world_to_screen(w[0]);
                let b = viewport.world_to_screen(w[1]);
                painter.line_segment([a, b], stroke);
            }
            if pl.closed && pl.vertices.len() >= 2 {
                let a = viewport.world_to_screen(*pl.vertices.last().unwrap());
                let b = viewport.world_to_screen(pl.vertices[0]);
                painter.line_segment([a, b], stroke);
            }
        }
        Geometry::Polygon(pg) => {
            for w in pg.vertices.windows(2) {
                let a = viewport.world_to_screen(w[0]);
                let b = viewport.world_to_screen(w[1]);
                painter.line_segment([a, b], stroke);
            }
            if pg.vertices.len() >= 2 {
                let a = viewport.world_to_screen(*pg.vertices.last().unwrap());
                let b = viewport.world_to_screen(pg.vertices[0]);
                painter.line_segment([a, b], stroke);
            }
        }
        Geometry::Circle(c) => {
            let center = viewport.world_to_screen(c.center);
            let radius = viewport.world_to_screen_distance(c.radius);
            painter.circle_stroke(center, radius, stroke);
        }
        Geometry::Arc(a) => {
            // Approximate arc with line segments
            let steps = 32;
            let mut points = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let angle = a.start_angle + (a.end_angle - a.start_angle) * t;
                let p = a.center + Vec2::new(angle.cos(), angle.sin()) * a.radius;
                points.push(viewport.world_to_screen(p));
            }
            for w in points.windows(2) {
                painter.line_segment([w[0], w[1]], stroke);
            }
        }
        Geometry::Rectangle(r) => {
            let half = r.half_size;
            let corners_local = [
                Vec2::new(-half.x, -half.y),
                Vec2::new(half.x, -half.y),
                Vec2::new(half.x, half.y),
                Vec2::new(-half.x, half.y),
            ];
            let (sin, cos) = r.rotation.sin_cos();
            let corners: Vec<_> = corners_local
                .iter()
                .map(|c| {
                    let rotated = Vec2::new(c.x * cos - c.y * sin, c.x * sin + c.y * cos);
                    viewport.world_to_screen(r.center + rotated)
                })
                .collect();
            for i in 0..4 {
                painter.line_segment([corners[i], corners[(i + 1) % 4]], stroke);
            }
        }
        Geometry::Text(t) => {
            let s = viewport.world_to_screen(t.position);
            painter.text(
                s,
                egui::Align2::LEFT_TOP,
                &t.text,
                egui::FontId::proportional(t.font_size * viewport.zoom),
                stroke.color,
            );
        }
        Geometry::BezierPath(bp) => {
            let flat = bp.flatten(16);
            for w in flat.windows(2) {
                let a = viewport.world_to_screen(w[0]);
                let b = viewport.world_to_screen(w[1]);
                painter.line_segment([a, b], stroke);
            }
            if bp.closed && flat.len() >= 2 {
                let a = viewport.world_to_screen(*flat.last().unwrap());
                let b = viewport.world_to_screen(flat[0]);
                painter.line_segment([a, b], stroke);
            }
        }
        Geometry::Dimension(_) => {
            // Dimension preview: just draw bounding box outline
            if let Some(bb) = geom.bounding_box() {
                let min = viewport.world_to_screen(bb.min);
                let max = viewport.world_to_screen(bb.max);
                let rect = egui::Rect::from_two_pos(min, max);
                painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
            }
        }
    }
}
