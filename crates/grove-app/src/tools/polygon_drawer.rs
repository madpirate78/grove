use glam::Vec2;

use grove_core::entity::{BezierAnchor, BezierPathData};
use grove_core::geometry::{
    closest_point_on_polyline, extract_polyline_section, visible_edges, would_self_intersect,
    VisibleEdge,
};
use grove_core::snap::{self, SnapResult};

use super::draw_line::draw_snap_indicator;
use super::{collect_snap_centers, collect_snap_segments, ortho_constrain, ToolContext};
use crate::view::viewport::ViewTransform;

/// First trace point on an edge.
pub struct EdgeTracePoint {
    pub position: Vec2,
    pub segment_idx: usize,
    pub t: f32,
}

/// State for an in-progress edge trace.
pub struct EdgeTraceState {
    /// Cached visible edge being traced.
    pub edge_vertices: Vec<Vec2>,
    /// Whether the edge is a closed loop.
    pub edge_closed: bool,
    /// First trace point (None until first click).
    pub start: Option<EdgeTracePoint>,
    /// Preview vertices from start to current cursor projection.
    pub preview: Vec<Vec2>,
    /// Flip direction for closed edges.
    pub reversed: bool,
    /// How many anchors the last completed trace inserted.
    pub last_insert_count: usize,
}

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
    /// Whether edge-trace mode is active (user pressed E).
    pub edge_trace_active: bool,
    /// Current edge-trace state (Some when tracing an edge).
    pub edge_trace: Option<EdgeTraceState>,
    /// Nearest edge highlight info for pre-click hover preview.
    edge_trace_hover: Option<(Vec<Vec2>, bool)>,
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
            edge_trace_active: false,
            edge_trace: None,
            edge_trace_hover: None,
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
        // Edge trace mode: delegate to trace click logic
        if self.edge_trace_active {
            return self.edge_trace_click(pos, ctx);
        }

        let effective = self.effective_pos(pos, ctx);

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

        // Edge trace preview
        if self.edge_trace_active {
            self.update_edge_trace_preview(pos, ctx);
            return;
        }

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
            let mut candidates = snap::snap_points(constrained, &segments, &centers, ctx.snap_config);
            if !ctx.smart_points.is_empty() {
                candidates.extend(super::extension_snap_candidates(constrained, ctx.smart_points));
            }
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

        // If last action was an edge trace, undo all traced vertices at once
        if let Some(ref mut state) = self.edge_trace {
            if state.last_insert_count > 0 {
                let count = state.last_insert_count;
                for _ in 0..count {
                    self.anchors.pop();
                }
                state.last_insert_count = 0;
                return !self.anchors.is_empty();
            }
        }

        self.anchors.pop();
        !self.anchors.is_empty()
    }

    /// Clear all state.
    pub fn clear(&mut self) {
        self.anchors.clear();
        self.dragging_handle = None;
        self.drag_anchor_pos = None;
        self.self_intersects = false;
        self.edge_trace_active = false;
        self.edge_trace = None;
        self.edge_trace_hover = None;
    }

    pub fn toggle_curve_mode(&mut self) {
        self.curve_mode = !self.curve_mode;
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

    /// Build a closed BezierPathData from the current anchors.
    pub fn to_bezier_path_closed(&self) -> BezierPathData {
        BezierPathData {
            anchors: self.anchors.clone(),
            closed: true,
            holes: Vec::new(),
        }
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
        let mode_label = if self.edge_trace_active {
            "E"
        } else if self.curve_mode {
            "C"
        } else {
            "L"
        };
        let label_pos = egui::pos2(cursor_screen.x + 12.0, cursor_screen.y - 12.0);
        let font = egui::FontId::monospace(10.0);
        let label_colour = if self.edge_trace_active {
            egui::Color32::from_rgb(0, 220, 220)
        } else {
            egui::Color32::from_rgba_unmultiplied(200, 200, 200, 180)
        };
        painter.text(
            label_pos,
            egui::Align2::LEFT_BOTTOM,
            mode_label,
            font,
            label_colour,
        );

        // Edge trace visuals
        self.draw_edge_trace_preview(painter, viewport);

        // Snap indicator
        if let Some(ref snap) = self.snap {
            draw_snap_indicator(painter, viewport, snap);
        }
    }

    // -----------------------------------------------------------------------
    // Edge trace methods
    // -----------------------------------------------------------------------

    /// Toggle edge-trace mode on/off.
    pub fn toggle_edge_trace(&mut self) {
        self.edge_trace_active = !self.edge_trace_active;
        if !self.edge_trace_active {
            self.edge_trace = None;
            self.edge_trace_hover = None;
        }
    }

    /// Flip trace direction (Tab key). Only meaningful for closed edges.
    pub fn edge_trace_flip(&mut self) {
        if let Some(ref mut state) = self.edge_trace {
            if state.edge_closed {
                state.reversed = !state.reversed;
            }
        }
    }

    /// Cancel current edge trace. Returns true if something was cancelled.
    pub fn edge_trace_cancel(&mut self) -> bool {
        if let Some(ref state) = self.edge_trace {
            if state.start.is_some() {
                // Cancel the in-progress trace, keep edge-trace mode active
                self.edge_trace = None;
                self.edge_trace_hover = None;
                return true;
            }
        }
        if self.edge_trace_active {
            self.edge_trace_active = false;
            self.edge_trace = None;
            self.edge_trace_hover = None;
            return true;
        }
        false
    }

    /// Handle a click in edge-trace mode. Returns true if consumed.
    fn edge_trace_click(&mut self, pos: Vec2, ctx: &ToolContext) -> bool {
        let pick_tolerance = 10.0 / ctx.viewport_zoom;

        if let Some(ref mut state) = self.edge_trace {
            if let Some(ref start) = state.start {
                // Second click: project onto cached edge and extract section
                let (_, seg, t) = closest_point_on_polyline(
                    pos,
                    &state.edge_vertices,
                    state.edge_closed,
                );
                let section = extract_polyline_section(
                    &state.edge_vertices,
                    state.edge_closed,
                    start.segment_idx,
                    start.t,
                    seg,
                    t,
                    state.reversed,
                );

                // Insert all section vertices as anchors
                let count = section.len();
                for pt in section {
                    self.anchors.push(BezierAnchor {
                        point: pt,
                        handle_in: None,
                        handle_out: None,
                    });
                }

                // Store insert count for batch undo, then clear trace state
                let reversed = state.reversed;
                self.edge_trace = Some(EdgeTraceState {
                    edge_vertices: Vec::new(),
                    edge_closed: false,
                    start: None,
                    preview: Vec::new(),
                    reversed,
                    last_insert_count: count,
                });
                self.edge_trace_active = false;
                self.edge_trace_hover = None;
                return true;
            }
        }

        // First click: find nearest visible edge across all entities
        let mut best_dist = f32::MAX;
        let mut best_edge: Option<VisibleEdge> = None;
        let mut best_proj = Vec2::ZERO;
        let mut best_seg = 0usize;
        let mut best_t = 0.0f32;

        for entity in ctx.entities {
            let edges = visible_edges(&entity.geometry, &entity.domain);
            for edge in edges {
                if edge.vertices.len() < 2 {
                    continue;
                }
                let (proj, seg, t) =
                    closest_point_on_polyline(pos, &edge.vertices, edge.closed);
                let dist = (pos - proj).length();
                if dist < best_dist && dist < pick_tolerance {
                    best_dist = dist;
                    best_proj = proj;
                    best_seg = seg;
                    best_t = t;
                    best_edge = Some(edge);
                }
            }
        }

        if let Some(edge) = best_edge {
            self.edge_trace = Some(EdgeTraceState {
                edge_vertices: edge.vertices,
                edge_closed: edge.closed,
                start: Some(EdgeTracePoint {
                    position: best_proj,
                    segment_idx: best_seg,
                    t: best_t,
                }),
                preview: Vec::new(),
                reversed: false,
                last_insert_count: 0,
            });
            return true;
        }

        false
    }

    /// Update edge-trace hover/preview state during mouse move.
    fn update_edge_trace_preview(&mut self, pos: Vec2, ctx: &ToolContext) {
        let pick_tolerance = 10.0 / ctx.viewport_zoom;

        if let Some(ref mut state) = self.edge_trace {
            if let Some(ref start) = state.start {
                // Project cursor onto cached edge for preview
                let (_, seg, t) = closest_point_on_polyline(
                    pos,
                    &state.edge_vertices,
                    state.edge_closed,
                );
                state.preview = extract_polyline_section(
                    &state.edge_vertices,
                    state.edge_closed,
                    start.segment_idx,
                    start.t,
                    seg,
                    t,
                    state.reversed,
                );
                return;
            }
        }

        // No start yet: find nearest edge for hover highlight
        let mut best_dist = f32::MAX;
        let mut best_verts: Option<(Vec<Vec2>, bool)> = None;

        for entity in ctx.entities {
            let edges = visible_edges(&entity.geometry, &entity.domain);
            for edge in edges {
                if edge.vertices.len() < 2 {
                    continue;
                }
                let (proj, _, _) =
                    closest_point_on_polyline(pos, &edge.vertices, edge.closed);
                let dist = (pos - proj).length();
                if dist < best_dist && dist < pick_tolerance {
                    best_dist = dist;
                    best_verts = Some((edge.vertices, edge.closed));
                }
            }
        }

        self.edge_trace_hover = best_verts;
    }

    /// Draw edge-trace visuals (hover highlight, traced section preview, markers).
    fn draw_edge_trace_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let cyan = egui::Color32::from_rgb(0, 220, 220);
        let cyan_stroke = egui::Stroke::new(2.0, cyan);
        let thin_cyan = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(0, 220, 220, 100));

        // Hover highlight (before first click)
        if self.edge_trace_active {
            if let Some(ref state) = self.edge_trace {
                if let Some(ref start) = state.start {
                    // Draw traced section preview
                    if state.preview.len() >= 2 {
                        let screen_pts: Vec<egui::Pos2> = state
                            .preview
                            .iter()
                            .map(|v| viewport.world_to_screen(*v))
                            .collect();
                        for w in screen_pts.windows(2) {
                            painter.line_segment([w[0], w[1]], cyan_stroke);
                        }
                    }

                    // Start marker
                    let start_screen = viewport.world_to_screen(start.position);
                    painter.circle_filled(start_screen, 4.0, cyan);

                    // End marker at last preview point
                    if let Some(last) = state.preview.last() {
                        let end_screen = viewport.world_to_screen(*last);
                        painter.circle_filled(end_screen, 4.0, cyan);
                    }

                    // Tab hint for closed edges
                    if state.edge_closed {
                        let cursor_screen = viewport.world_to_screen(self.current_pos);
                        painter.text(
                            egui::pos2(cursor_screen.x + 12.0, cursor_screen.y + 4.0),
                            egui::Align2::LEFT_TOP,
                            "Tab: flip",
                            egui::FontId::proportional(9.0),
                            egui::Color32::from_rgba_unmultiplied(0, 220, 220, 160),
                        );
                    }
                    return;
                }
            }

            // Hover: highlight nearest edge
            if let Some((ref verts, closed)) = self.edge_trace_hover {
                let n = verts.len();
                let seg_count = if closed { n } else { n - 1 };
                for i in 0..seg_count {
                    let a = viewport.world_to_screen(verts[i]);
                    let b = viewport.world_to_screen(verts[(i + 1) % n]);
                    painter.line_segment([a, b], thin_cyan);
                }
            }
        }
    }
}
