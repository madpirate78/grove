use glam::Vec2;

use grove_core::commands::AddEntityCommand;
use grove_core::entity::{BezierAnchor, BezierPathData, Entity, Geometry};
use grove_core::snap::{self, SnapResult};

use super::{snap_candidates_from_entities, Tool, ToolContext, ToolResult};
use super::draw_line::draw_snap_indicator;
use crate::view::viewport::ViewTransform;

/// Pen-tool style bezier curve drawing.
/// Click to place corner anchors. Click-and-drag to place smooth anchors
/// with tangent handles. The tangent length/direction is set by the drag.
/// Double-click or Enter to finish. Right-click to undo last anchor.
pub struct DrawBezierTool {
    anchors: Vec<BezierAnchor>,
    /// While dragging after placing an anchor, this is the handle_out position.
    dragging_handle: Option<Vec2>,
    /// The mouse-down position for the current drag (the anchor point).
    drag_anchor_pos: Option<Vec2>,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    layer_id: uuid::Uuid,
}

impl DrawBezierTool {
    pub fn new() -> Self {
        Self {
            anchors: Vec::new(),
            dragging_handle: None,
            drag_anchor_pos: None,
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
        }
    }

    fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        let constrained = if ctx.shift_held && !self.anchors.is_empty() {
            let anchor = if let Some(ap) = self.drag_anchor_pos {
                ap
            } else {
                self.anchors.last().unwrap().point
            };
            super::ortho_constrain(anchor, raw)
        } else {
            raw
        };

        if ctx.snap_config.enabled {
            let candidates = snap_candidates_from_entities(constrained, ctx.entities, ctx.snap_config);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            if let Some(result) = snap::best_snap(&candidates, tolerance) {
                return result.position;
            }
        }

        constrained
    }

    fn finish_curve(&mut self) -> ToolResult {
        if self.anchors.len() < 2 {
            return ToolResult::Continue;
        }

        let entity = {
            let mut e = Entity::cad(
                "Curve",
                Geometry::BezierPath(BezierPathData {
                    anchors: self.anchors.clone(),
                    closed: false,
                }),
            );
            e.layer_id = self.layer_id;
            e.colour = [220, 220, 225, 255];
            e
        };

        self.anchors.clear();
        self.dragging_handle = None;
        self.drag_anchor_pos = None;
        ToolResult::EmitAndContinue(Box::new(AddEntityCommand::new(entity)))
    }

    /// Build a preview BezierPathData including a temporary next segment to the cursor.
    fn preview_path(&self) -> BezierPathData {
        let mut anchors = self.anchors.clone();

        // If currently dragging a handle on the last anchor, update it
        if let (Some(handle), Some(_)) = (self.dragging_handle, self.drag_anchor_pos) {
            if let Some(last) = anchors.last_mut() {
                let mirror = last.point * 2.0 - handle;
                last.handle_out = Some(handle);
                last.handle_in = Some(mirror);
            }
        }

        // Add a preview anchor at cursor position (corner point)
        if !anchors.is_empty() && self.dragging_handle.is_none() {
            anchors.push(BezierAnchor {
                point: self.current_pos,
                handle_in: None,
                handle_out: None,
            });
        }

        BezierPathData {
            anchors,
            closed: false,
        }
    }
}

impl Tool for DrawBezierTool {
    fn name(&self) -> &str {
        "Curve"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = ctx.active_layer_id;
        let effective = self.effective_pos(pos, ctx);

        // Place a new anchor (initially a corner point, drag will add handles)
        self.anchors.push(BezierAnchor {
            point: effective,
            handle_in: None,
            handle_out: None,
        });
        self.drag_anchor_pos = Some(effective);
        self.dragging_handle = None;

        ToolResult::Continue
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        let effective = self.effective_pos(pos, ctx);
        self.current_pos = effective;

        // If we're dragging (mouse is down after placing an anchor), update the handle
        if let Some(anchor_pos) = self.drag_anchor_pos {
            let dist = (effective - anchor_pos).length();
            if dist > 2.0 / ctx.viewport_zoom {
                self.dragging_handle = Some(effective);
            }
        }

        // Update snap
        if ctx.snap_config.enabled {
            let constrained = if ctx.shift_held && !self.anchors.is_empty() {
                let anchor = if let Some(ap) = self.drag_anchor_pos {
                    ap
                } else {
                    self.anchors.last().unwrap().point
                };
                super::ortho_constrain(anchor, pos)
            } else {
                pos
            };
            let candidates =
                snap_candidates_from_entities(constrained, ctx.entities, ctx.snap_config);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }
    }

    fn on_mouse_up(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        if let Some(anchor_pos) = self.drag_anchor_pos.take() {
            if let Some(handle_pos) = self.dragging_handle.take() {
                // Finalize the anchor with symmetric handles
                if let Some(last) = self.anchors.last_mut() {
                    let mirror = anchor_pos * 2.0 - handle_pos;
                    last.handle_out = Some(handle_pos);
                    last.handle_in = Some(mirror);
                }
            }
            // else: it was a click without drag — corner point, no handles needed
        }
        let _ = (pos, ctx);
        ToolResult::Continue
    }

    fn on_double_click(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        // The on_mouse_down already added an anchor for this click.
        // Finalize any drag state, then finish.
        self.drag_anchor_pos = None;
        self.dragging_handle = None;
        let _ = (pos, ctx);
        self.finish_curve()
    }

    fn on_right_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        self.drag_anchor_pos = None;
        self.dragging_handle = None;
        if self.anchors.len() > 1 {
            self.anchors.pop();
            ToolResult::Continue
        } else if !self.anchors.is_empty() {
            self.anchors.clear();
            ToolResult::Continue
        } else {
            ToolResult::Finished
        }
    }

    fn on_key_enter(&mut self) -> ToolResult {
        self.finish_curve()
    }

    fn on_key_escape(&mut self) -> ToolResult {
        if self.anchors.is_empty() {
            ToolResult::Finished
        } else {
            self.anchors.clear();
            self.dragging_handle = None;
            self.drag_anchor_pos = None;
            ToolResult::Continue
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        if self.anchors.is_empty() {
            return;
        }

        let accent = egui::Color32::from_rgb(70, 130, 210);
        let handle_colour = egui::Color32::from_rgb(200, 140, 60);
        let curve_colour = egui::Color32::from_rgba_unmultiplied(220, 220, 225, 180);
        let stroke = egui::Stroke::new(1.5, curve_colour);
        let handle_stroke = egui::Stroke::new(1.0, handle_colour);

        // Draw the curve preview
        let path = self.preview_path();
        let flat = path.flatten(16);
        if flat.len() >= 2 {
            let screen_points: Vec<egui::Pos2> = flat
                .iter()
                .map(|v| viewport.world_to_screen(*v))
                .collect();
            for w in screen_points.windows(2) {
                painter.line_segment([w[0], w[1]], stroke);
            }
        }

        // Draw anchor points and their handles
        for anchor in &self.anchors {
            let screen = viewport.world_to_screen(anchor.point);
            painter.circle_filled(screen, 4.0, accent);

            // Draw handle lines and dots
            if let Some(h_in) = anchor.handle_in {
                let h_screen = viewport.world_to_screen(h_in);
                painter.line_segment([h_screen, screen], handle_stroke);
                painter.circle_filled(h_screen, 3.0, handle_colour);
            }
            if let Some(h_out) = anchor.handle_out {
                let h_screen = viewport.world_to_screen(h_out);
                painter.line_segment([screen, h_screen], handle_stroke);
                painter.circle_filled(h_screen, 3.0, handle_colour);
            }
        }

        // If currently dragging a handle, show it live
        if let (Some(handle), Some(anchor_pos)) = (self.dragging_handle, self.drag_anchor_pos) {
            let handle_screen = viewport.world_to_screen(handle);
            let mirror = anchor_pos * 2.0 - handle;
            let mirror_screen = viewport.world_to_screen(mirror);

            // Tangent line through anchor
            painter.line_segment([mirror_screen, handle_screen], handle_stroke);
            painter.circle_filled(handle_screen, 3.0, handle_colour);
            painter.circle_filled(mirror_screen, 3.0, handle_colour);
        }

        // Cursor dot (when not dragging)
        if self.dragging_handle.is_none() {
            let cursor_screen = viewport.world_to_screen(self.current_pos);
            painter.circle_stroke(
                cursor_screen,
                4.0,
                egui::Stroke::new(1.0, accent),
            );
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
