//! Bezier control-point editing: hit-testing, handle rendering, drag application.

use super::*;

impl GroveApp {
    /// Hit-test bezier anchors and handles for the selected entity.
    /// Returns (anchor_index, drag_target) if hit, plus the entity id.
    pub(super) fn hit_test_bezier_points(
        &self,
        pos: glam::Vec2,
        tolerance: f32,
    ) -> Option<(uuid::Uuid, usize, BezierDragTarget)> {
        let sel_id = self.selection.single()?;
        let entity = self.project.find_entity(sel_id)?;
        let anchors = match &entity.geometry {
            Geometry::BezierPath(bp) => &bp.anchors,
            _ => return None,
        };

        // Check handles first (they're smaller targets, prioritize them)
        for (i, anchor) in anchors.iter().enumerate() {
            if let Some(h) = anchor.handle_in {
                if (pos - h).length() <= tolerance {
                    return Some((sel_id, i, BezierDragTarget::HandleIn));
                }
            }
            if let Some(h) = anchor.handle_out {
                if (pos - h).length() <= tolerance {
                    return Some((sel_id, i, BezierDragTarget::HandleOut));
                }
            }
        }
        // Then check anchor points
        for (i, anchor) in anchors.iter().enumerate() {
            if (pos - anchor.point).length() <= tolerance {
                return Some((sel_id, i, BezierDragTarget::Anchor));
            }
        }
        None
    }

    /// Draw bezier edit handles (anchors + tangent lines) for a selected bezier entity.
    pub(super) fn draw_bezier_edit_handles(&self, painter: &egui::Painter, entity_id: uuid::Uuid) {
        let Some(entity) = self.project.find_entity(entity_id) else { return };
        let anchors = match &entity.geometry {
            Geometry::BezierPath(bp) => &bp.anchors,
            _ => return,
        };

        let accent = Color32::from_rgb(70, 130, 210);
        let handle_colour = Color32::from_rgb(200, 140, 60);
        let handle_stroke = egui::Stroke::new(1.0, handle_colour);

        for anchor in anchors {
            let screen = self.canvas.viewport.world_to_screen(anchor.point);

            // Draw tangent lines and handle dots
            if let Some(h_in) = anchor.handle_in {
                let h_screen = self.canvas.viewport.world_to_screen(h_in);
                painter.line_segment([h_screen, screen], handle_stroke);
                painter.circle_filled(h_screen, 3.5, handle_colour);
            }
            if let Some(h_out) = anchor.handle_out {
                let h_screen = self.canvas.viewport.world_to_screen(h_out);
                painter.line_segment([screen, h_screen], handle_stroke);
                painter.circle_filled(h_screen, 3.5, handle_colour);
            }

            // Anchor point (drawn on top)
            painter.circle_filled(screen, 5.0, accent);
            painter.circle_filled(screen, 3.0, Color32::from_rgb(30, 30, 36));
        }
    }

    /// Apply a drag delta to the bezier point being edited.
    pub(super) fn apply_bezier_drag(&mut self, pos: glam::Vec2) {
        let Some(ref edit) = self.bezier_edit else { return };
        let entity_id = edit.entity_id;
        let idx = edit.anchor_index;
        let target = edit.target;

        let Some(entity) = self.project.find_entity_mut(entity_id) else { return };
        let anchors = match &mut entity.geometry {
            Geometry::BezierPath(bp) => &mut bp.anchors,
            _ => return,
        };
        let Some(anchor) = anchors.get_mut(idx) else { return };

        match target {
            BezierDragTarget::Anchor => {
                let delta = pos - anchor.point;
                anchor.point = pos;
                // Move handles with the anchor
                if let Some(ref mut h) = anchor.handle_in {
                    *h += delta;
                }
                if let Some(ref mut h) = anchor.handle_out {
                    *h += delta;
                }
            }
            BezierDragTarget::HandleOut => {
                anchor.handle_out = Some(pos);
                // Mirror the other handle for smooth continuity
                anchor.handle_in = Some(anchor.point * 2.0 - pos);
            }
            BezierDragTarget::HandleIn => {
                anchor.handle_in = Some(pos);
                // Mirror the other handle for smooth continuity
                anchor.handle_out = Some(anchor.point * 2.0 - pos);
            }
        }
    }
}
