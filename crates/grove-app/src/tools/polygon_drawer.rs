use glam::Vec2;

use grove_core::entity::{BezierAnchor, BezierPathData};
use grove_core::geometry::would_self_intersect;
use grove_core::snap::{self, SnapResult};

use super::draw_line::draw_snap_indicator;
use super::{collect_snap_centers, collect_snap_segments, ortho_constrain, ToolContext};
use crate::view::viewport::ViewTransform;

/// Shared drawing state machine for placing vertices with optional bezier curve mode.
/// Used by both `DrawLineTool` and `PlaceHardscapeTool` to avoid duplicating the
/// click-to-place / click-drag-for-curves logic.
pub struct PolygonDrawer {
    pub anchors: Vec<BezierAnchor>,
    pub curve_mode: bool,
    pub dragging_handle: Option<Vec2>,
    pub drag_anchor_pos: Option<Vec2>,
    pub current_pos: Vec2,
    pub snap: Option<SnapResult>,
    /// Live feedback: true when the next edge would self-intersect.
    pub self_intersects: bool,
    /// true = polygon (checks closing edge for intersection), false = polyline.
    closing_mode: bool,
}

impl PolygonDrawer {
    pub fn new(closing_mode: bool) -> Self {
        Self {
            anchors: Vec::new(),
            curve_mode: false,
            dragging_handle: None,
            drag_anchor_pos: None,
            current_pos: Vec2::ZERO,
            snap: None,
            self_intersects: false,
            closing_mode,
        }
    }

    pub fn set_closing_mode(&mut self, closing: bool) {
        self.closing_mode = closing;
    }

    /// Compute effective cursor position with ortho constraint + snap.
    pub fn effective_pos(&self, raw: Vec2, ctx: &ToolContext) -> Vec2 {
        let constrained = if ctx.shift_held && !self.anchors.is_empty() {
            let anchor = if let Some(ap) = self.drag_anchor_pos {
                ap
            } else {
                self.anchors.last().unwrap().point
            };
            ortho_constrain(anchor, raw)
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

    pub fn snap_data(&self, ctx: &ToolContext) -> (Vec<(Vec2, Vec2)>, Vec<snap::CenterPoint>) {
        let mut segs = Vec::new();
        let mut centers = Vec::new();
        for entity in ctx.entities {
            collect_snap_segments(&entity.geometry, &mut segs);
            collect_snap_centers(&entity.geometry, &mut centers);
        }
        // Segments from the path being drawn
        for w in self.anchors.windows(2) {
            segs.push((w[0].point, w[1].point));
        }
        (segs, centers)
    }

    /// Returns true if any anchor has bezier handles set.
    pub fn has_curves(&self) -> bool {
        self.anchors
            .iter()
            .any(|a| a.handle_in.is_some() || a.handle_out.is_some())
    }

    /// Add an anchor at the given position. Returns false if self-intersecting.
    pub fn add_anchor(&mut self, pos: Vec2, ctx: &ToolContext) -> bool {
        let effective = self.effective_pos(pos, ctx);

        if self.self_intersects {
            return false;
        }

        self.anchors.push(BezierAnchor {
            point: effective,
            handle_in: None,
            handle_out: None,
        });

        if self.curve_mode {
            self.drag_anchor_pos = Some(effective);
            self.dragging_handle = None;
        }

        true
    }

    /// Update cursor position, snap indicator, and self-intersection check.
    pub fn update_cursor(&mut self, pos: Vec2, ctx: &ToolContext) {
        let effective = self.effective_pos(pos, ctx);
        self.current_pos = effective;

        // In curve mode, detect drag to set handles
        if self.curve_mode {
            if let Some(anchor_pos) = self.drag_anchor_pos {
                let dist = (effective - anchor_pos).length();
                if dist > 2.0 / ctx.viewport_zoom {
                    self.dragging_handle = Some(effective);
                }
            }
        }

        // Update snap indicator
        if ctx.snap_config.enabled {
            let constrained = if ctx.shift_held && !self.anchors.is_empty() {
                let anchor = if let Some(ap) = self.drag_anchor_pos {
                    ap
                } else {
                    self.anchors.last().unwrap().point
                };
                ortho_constrain(anchor, pos)
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

        // Self-intersection check
        if self.anchors.len() >= 2 {
            let verts: Vec<Vec2> = self.anchors.iter().map(|a| a.point).collect();
            self.self_intersects = would_self_intersect(&verts, effective, self.closing_mode);
        } else {
            self.self_intersects = false;
        }
    }

    /// Finalize handle on mouse_up in curve mode.
    pub fn finalize_handle(&mut self) {
        if self.curve_mode {
            if let Some(anchor_pos) = self.drag_anchor_pos.take() {
                if let Some(handle_pos) = self.dragging_handle.take() {
                    if let Some(last) = self.anchors.last_mut() {
                        let mirror = anchor_pos * 2.0 - handle_pos;
                        last.handle_out = Some(handle_pos);
                        last.handle_in = Some(mirror);
                    }
                }
            }
        }
    }

    /// Pop the last anchor. Returns true if there are still anchors remaining.
    pub fn undo_last(&mut self) -> bool {
        self.drag_anchor_pos = None;
        self.dragging_handle = None;
        self.anchors.pop();
        !self.anchors.is_empty()
    }

    /// Clear all state.
    pub fn clear(&mut self) {
        self.anchors.clear();
        self.dragging_handle = None;
        self.drag_anchor_pos = None;
        self.self_intersects = false;
    }

    pub fn toggle_curve_mode(&mut self) {
        self.curve_mode = !self.curve_mode;
    }

    /// Check if `pos` is near the first anchor (for close-on-first-node detection).
    pub fn is_near_first_anchor(&self, pos: Vec2, tolerance: f32) -> bool {
        if let Some(first) = self.anchors.first() {
            (pos - first.point).length() <= tolerance
        } else {
            false
        }
    }

    /// Build a preview BezierPathData including a temporary segment to cursor.
    pub fn preview_path(&self) -> BezierPathData {
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
            holes: Vec::new(),
        }
    }

    /// Extract plain vertex positions (for non-curve output).
    pub fn to_vertices(&self) -> Vec<Vec2> {
        self.anchors.iter().map(|a| a.point).collect()
    }

    /// Draw the preview on the canvas.
    pub fn draw_preview(
        &self,
        painter: &egui::Painter,
        viewport: &ViewTransform,
        colour: [u8; 4],
        show_closing_line: bool,
        show_self_intersect: bool,
    ) {
        if self.anchors.is_empty() {
            return;
        }

        let stroke_colour =
            egui::Color32::from_rgba_unmultiplied(colour[0], colour[1], colour[2], 180);
        let preview_colour =
            egui::Color32::from_rgba_unmultiplied(colour[0], colour[1], colour[2], 120);
        let stroke = egui::Stroke::new(1.5, stroke_colour);
        let preview_stroke = egui::Stroke::new(1.0, preview_colour);
        let dot_colour = egui::Color32::from_rgb(colour[0], colour[1], colour[2]);

        let handle_colour = egui::Color32::from_rgb(200, 140, 60);
        let handle_stroke = egui::Stroke::new(1.0, handle_colour);

        if self.has_curves() || self.curve_mode {
            // Use bezier preview path
            let path = self.preview_path();
            let flat = path.flatten(16);
            if flat.len() >= 2 {
                let screen_points: Vec<egui::Pos2> = flat
                    .iter()
                    .map(|v| viewport.world_to_screen(*v))
                    .collect();

                let confirmed = BezierPathData {
                    anchors: self.anchors.clone(),
                    closed: false,
                    holes: Vec::new(),
                };
                let confirmed_flat = confirmed.flatten(16);
                let confirmed_screen_count = confirmed_flat.len();

                for (i, w) in screen_points.windows(2).enumerate() {
                    let s = if i + 1 < confirmed_screen_count {
                        stroke
                    } else {
                        preview_stroke
                    };
                    painter.line_segment([w[0], w[1]], s);
                }
            }

            // Draw anchor points and handles
            let accent = egui::Color32::from_rgb(70, 130, 210);
            for anchor in &self.anchors {
                let screen = viewport.world_to_screen(anchor.point);
                painter.circle_filled(screen, 4.0, accent);

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

            // Live handle preview during drag
            if let (Some(handle), Some(anchor_pos)) = (self.dragging_handle, self.drag_anchor_pos) {
                let handle_screen = viewport.world_to_screen(handle);
                let mirror = anchor_pos * 2.0 - handle;
                let mirror_screen = viewport.world_to_screen(mirror);
                painter.line_segment([mirror_screen, handle_screen], handle_stroke);
                painter.circle_filled(handle_screen, 3.0, handle_colour);
                painter.circle_filled(mirror_screen, 3.0, handle_colour);
            }
        } else {
            // Pure straight-line mode
            for w in self.anchors.windows(2) {
                let a = viewport.world_to_screen(w[0].point);
                let b = viewport.world_to_screen(w[1].point);
                painter.line_segment([a, b], stroke);
            }

            // Preview segment to cursor
            let last = viewport.world_to_screen(self.anchors.last().unwrap().point);
            let cursor = viewport.world_to_screen(self.current_pos);

            if show_self_intersect && self.self_intersects {
                let red_stroke =
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(220, 60, 60));
                painter.line_segment([last, cursor], red_stroke);

                // For polygon mode, closing line in red too
                if show_closing_line && self.anchors.len() >= 2 {
                    let first = viewport.world_to_screen(self.anchors[0].point);
                    painter.line_segment([cursor, first], red_stroke);
                }

                // Red X near cursor
                let x_size = 6.0;
                let x_stroke =
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(220, 60, 60));
                painter.line_segment(
                    [
                        egui::pos2(cursor.x - x_size, cursor.y - x_size),
                        egui::pos2(cursor.x + x_size, cursor.y + x_size),
                    ],
                    x_stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(cursor.x + x_size, cursor.y - x_size),
                        egui::pos2(cursor.x - x_size, cursor.y + x_size),
                    ],
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
                if show_closing_line && self.anchors.len() >= 2 {
                    let first = viewport.world_to_screen(self.anchors[0].point);
                    painter.line_segment(
                        [cursor, first],
                        egui::Stroke::new(
                            0.5,
                            egui::Color32::from_rgba_unmultiplied(
                                colour[0], colour[1], colour[2], 60,
                            ),
                        ),
                    );
                }
            }

            // Vertex dots
            for a in &self.anchors {
                let screen = viewport.world_to_screen(a.point);
                painter.circle_filled(screen, 3.0, dot_colour);
            }
        }

        // Mode indicator near cursor
        let cursor_screen = viewport.world_to_screen(self.current_pos);
        let mode_label = if self.curve_mode { "C" } else { "L" };
        let label_pos = egui::pos2(cursor_screen.x + 12.0, cursor_screen.y - 12.0);
        let font = egui::FontId::monospace(10.0);
        let label_colour = egui::Color32::from_rgba_unmultiplied(200, 200, 200, 180);
        painter.text(
            label_pos,
            egui::Align2::LEFT_BOTTOM,
            mode_label,
            font,
            label_colour,
        );

        // Snap indicator
        if let Some(ref snap) = self.snap {
            draw_snap_indicator(painter, viewport, snap);
        }
    }
}
