use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{ArcData, Entity, Geometry};
use grove_core::snap::{self, SnapResult};

use super::{snap_candidates_from_entities, Tool, ToolContext, ToolResult};
use super::draw_line::draw_snap_indicator;
use crate::view::viewport::ViewTransform;

/// Arc defined by three points: start, a point on the arc, end.
/// Internally converted to center + radius + start_angle + end_angle.
#[derive(Debug, Clone, Copy)]
enum ArcState {
    /// Waiting for first click (start point).
    WaitingStart,
    /// Have start, waiting for point on arc.
    HaveStart { start: Vec2 },
    /// Have start and mid, waiting for end point.
    HaveMid { start: Vec2, mid: Vec2 },
}

pub struct DrawArcTool {
    state: ArcState,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    layer_id: uuid::Uuid,
}

impl DrawArcTool {
    pub fn new() -> Self {
        Self {
            state: ArcState::WaitingStart,
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
        }
    }

    fn snapped_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        if ctx.snap_config.enabled {
            let candidates = snap_candidates_from_entities(raw, ctx.entities, ctx.snap_config, ctx.smart_points);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }
        raw
    }
}

/// Compute a circle (center, radius) from three points.
/// Returns None if points are collinear.
fn circle_from_3_points(p1: Vec2, p2: Vec2, p3: Vec2) -> Option<(Vec2, f32)> {
    let ax = p1.x;
    let ay = p1.y;
    let bx = p2.x;
    let by = p2.y;
    let cx = p3.x;
    let cy = p3.y;

    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-6 {
        return None;
    }

    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;

    let center = Vec2::new(ux, uy);
    let radius = (p1 - center).length();
    Some((center, radius))
}

/// Compute the angle of a point relative to a center.
fn angle_of(center: Vec2, point: Vec2) -> f32 {
    let d = point - center;
    d.y.atan2(d.x)
}

/// Check if angle `test` is between `start` and `end` going counter-clockwise.
fn angle_between_ccw(start: f32, end: f32, test: f32) -> bool {
    let tau = std::f32::consts::TAU;
    let s = start.rem_euclid(tau);
    let e = end.rem_euclid(tau);
    let t = test.rem_euclid(tau);

    if s <= e {
        t >= s && t <= e
    } else {
        t >= s || t <= e
    }
}

impl Tool for DrawArcTool {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = ctx.active_layer_id;
        let snapped = self.snapped_pos(pos, ctx);

        match self.state {
            ArcState::WaitingStart => {
                self.state = ArcState::HaveStart { start: snapped };
                ToolResult::Continue
            }
            ArcState::HaveStart { start } => {
                self.state = ArcState::HaveMid { start, mid: snapped };
                ToolResult::Continue
            }
            ArcState::HaveMid { start, mid } => {
                let end = snapped;
                // Compute arc from 3 points
                if let Some((center, radius)) = circle_from_3_points(start, mid, end) {
                    let angle_start = angle_of(center, start);
                    let angle_mid = angle_of(center, mid);
                    let angle_end = angle_of(center, end);

                    // Determine winding: does mid lie on the CCW arc from start to end?
                    let (sa, ea) = if angle_between_ccw(angle_start, angle_end, angle_mid) {
                        (angle_start, angle_end)
                    } else {
                        (angle_end, angle_start)
                    };

                    let mut entity = Entity::cad(
                        "Arc",
                        Geometry::Arc(ArcData {
                            center,
                            radius,
                            start_angle: sa,
                            end_angle: ea,
                        }),
                    );
                    entity.layer_id = self.layer_id;
                    entity.colour = [220, 220, 225, 255];

                    self.state = ArcState::WaitingStart;
                    ToolResult::EmitAndContinue(Box::new(AddEntityCommand::new(entity)))
                } else {
                    // Collinear — ignore, let user try again
                    self.state = ArcState::WaitingStart;
                    ToolResult::Continue
                }
            }
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        self.current_pos = self.snapped_pos(pos, ctx);

        if ctx.snap_config.enabled {
            let candidates = snap_candidates_from_entities(pos, ctx.entities, ctx.snap_config, ctx.smart_points);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }
    }

    fn on_key_escape(&mut self) -> ToolResult {
        match self.state {
            ArcState::WaitingStart => ToolResult::Finished,
            _ => {
                self.state = ArcState::WaitingStart;
                ToolResult::Continue
            }
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let accent = egui::Color32::from_rgb(70, 130, 210);
        let preview_colour = egui::Color32::from_rgba_unmultiplied(220, 220, 225, 140);

        match self.state {
            ArcState::WaitingStart => {}
            ArcState::HaveStart { start } => {
                // Draw start point and line to cursor
                let s = viewport.world_to_screen(start);
                let c = viewport.world_to_screen(self.current_pos);
                painter.circle_filled(s, 3.0, accent);
                painter.line_segment(
                    [s, c],
                    egui::Stroke::new(1.0, preview_colour),
                );
            }
            ArcState::HaveMid { start, mid } => {
                let end = self.current_pos;

                // Draw confirmed points
                let s = viewport.world_to_screen(start);
                let m = viewport.world_to_screen(mid);
                painter.circle_filled(s, 3.0, accent);
                painter.circle_filled(m, 3.0, accent);

                // Try to draw arc preview
                if let Some((center, radius)) = circle_from_3_points(start, mid, end) {
                    let angle_start = angle_of(center, start);
                    let angle_mid = angle_of(center, mid);
                    let angle_end = angle_of(center, end);

                    let (sa, ea) = if angle_between_ccw(angle_start, angle_end, angle_mid) {
                        (angle_start, angle_end)
                    } else {
                        (angle_end, angle_start)
                    };

                    // Draw arc as line segments
                    let n = 48;
                    let sweep = {
                        let mut s = ea - sa;
                        if s < 0.0 {
                            s += std::f32::consts::TAU;
                        }
                        s
                    };

                    let points: Vec<egui::Pos2> = (0..=n)
                        .map(|i| {
                            let t = i as f32 / n as f32;
                            let angle = sa + t * sweep;
                            let world = center + Vec2::new(angle.cos(), angle.sin()) * radius;
                            viewport.world_to_screen(world)
                        })
                        .collect();

                    for w in points.windows(2) {
                        painter.line_segment(
                            [w[0], w[1]],
                            egui::Stroke::new(1.0, preview_colour),
                        );
                    }
                } else {
                    // Collinear — just draw lines
                    let e = viewport.world_to_screen(end);
                    painter.line_segment([s, m], egui::Stroke::new(1.0, preview_colour));
                    painter.line_segment([m, e], egui::Stroke::new(1.0, preview_colour));
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
