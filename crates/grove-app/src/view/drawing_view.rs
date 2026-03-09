use egui::{Color32, Pos2, Rect, Stroke, Ui};
use grove_core::drawing::drawing::{Drawing, DrawingViewport};
use grove_core::drawing::layout::is_layer_visible_in_viewport;
use grove_core::entity::Geometry;
use grove_core::geometry::{offset_polyline, offset_polyline_sides};
use grove_core::project::Project;

use super::viewport::ViewTransform;

const PAPER_COLOR: Color32 = Color32::WHITE;
const MARGIN_COLOR: Color32 = Color32::from_rgb(200, 200, 210);
const VIEWPORT_FRAME_COLOR: Color32 = Color32::from_rgb(100, 140, 200);
const HANDLE_COLOR: Color32 = Color32::from_rgb(70, 130, 210);
const HANDLE_RADIUS: f32 = 5.0;

/// Which edge/corner of a viewport is being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Top,
    Bottom,
    Left,
    Right,
    Move,
}

/// Persistent state for drawing-mode interaction.
#[derive(Debug, Default)]
pub struct DrawingInteraction {
    /// Viewport being dragged, with handle type.
    pub drag: Option<(uuid::Uuid, ResizeHandle)>,
    /// Sheet-space position where a new-viewport drag started (Shift+drag).
    pub create_start: Option<[f32; 2]>,
    /// Selected viewport ID for showing handles.
    pub selected_viewport: Option<uuid::Uuid>,
}

/// Action produced by the drawing view for the app to process.
pub enum DrawingViewAction {
    UpdatedViewport(uuid::Uuid, DrawingViewport),
    CreatedViewport(DrawingViewport),
    SelectViewport(Option<uuid::Uuid>),
}

/// Render a drawing sheet on the canvas. Returns actions for the app to process.
///
/// `response` is the egui::Response from allocate_painter — we use it for
/// click/drag detection so we don't fight with egui's input system.
pub fn draw_drawing_view(
    ui: &mut Ui,
    project: &Project,
    drawing: &Drawing,
    view: &ViewTransform,
    response: &egui::Response,
    interaction: &mut DrawingInteraction,
) -> Vec<DrawingViewAction> {
    let mut actions = Vec::new();
    let (paper_w, paper_h) = drawing.paper_dimensions_mm();
    let painter = ui.painter();

    // --- Paper ---
    let paper_min_s = view.world_to_screen(glam::Vec2::ZERO);
    let paper_max_s = view.world_to_screen(glam::Vec2::new(paper_w, paper_h));
    let paper_rect = Rect::from_min_max(
        Pos2::new(paper_min_s.x, paper_min_s.y),
        Pos2::new(paper_max_s.x, paper_max_s.y),
    );
    // Drop shadow
    let shadow_rect = paper_rect.translate(egui::vec2(3.0, 3.0));
    painter.rect_filled(shadow_rect, 0.0, Color32::from_rgba_premultiplied(0, 0, 0, 40));
    painter.rect_filled(paper_rect, 0.0, PAPER_COLOR);
    painter.rect_stroke(paper_rect, 0.0, Stroke::new(1.0, Color32::from_gray(80)), egui::StrokeKind::Outside);

    // Margin lines
    let [mt, mr, mb, ml] = drawing.margins;
    let mtl = view.world_to_screen(glam::Vec2::new(ml, mt));
    let mbr = view.world_to_screen(glam::Vec2::new(paper_w - mr, paper_h - mb));
    let margin_rect = Rect::from_min_max(Pos2::new(mtl.x, mtl.y), Pos2::new(mbr.x, mbr.y));
    painter.rect_stroke(margin_rect, 0.0, Stroke::new(0.5, MARGIN_COLOR), egui::StrokeKind::Outside);

    // --- Viewports ---
    for vp in &drawing.viewports {
        let vp_rect = vp_screen_rect(vp, view);

        // Clip entity rendering to viewport rectangle
        let clipped = ui.painter_at(vp_rect);
        let effective_scale = view.zoom / vp.scale as f32;
        let vp_cx = (vp_rect.left() + vp_rect.right()) / 2.0;
        let vp_cy = (vp_rect.top() + vp_rect.bottom()) / 2.0;

        let to_screen = |world: glam::Vec2| -> Pos2 {
            Pos2::new(
                vp_cx + (world.x - vp.model_center[0]) * effective_scale,
                vp_cy + (world.y - vp.model_center[1]) * effective_scale,
            )
        };

        for entity in &project.entities {
            if !is_layer_visible_in_viewport(entity.layer_id, vp, &project.layers) {
                continue;
            }
            let colour = Color32::from_rgba_premultiplied(
                entity.colour[0], entity.colour[1], entity.colour[2], entity.colour[3],
            );
            let stroke = Stroke::new(1.0, colour);

            // Width-aware rendering for walls, paths, steps, edging
            if let Some(w) = entity.domain.width_mm() {
                if let Some(centerline) = entity.geometry.centerline_points() {
                    let fill = Color32::from_rgba_unmultiplied(
                        entity.colour[0], entity.colour[1], entity.colour[2], 30,
                    );
                    draw_width_shape_in_viewport(&clipped, &centerline, w, fill, stroke, &to_screen);
                    continue;
                }
            }

            draw_entity_simple(&clipped, &entity.geometry, stroke, colour, &to_screen);
        }

        // Viewport frame
        let is_selected = interaction.selected_viewport == Some(vp.id);
        let frame_color = if is_selected { HANDLE_COLOR } else { VIEWPORT_FRAME_COLOR };
        let frame_width = if is_selected { 1.5 } else { 1.0 };
        painter.rect_stroke(vp_rect, 0.0, Stroke::new(frame_width, frame_color), egui::StrokeKind::Outside);

        // Viewport label
        painter.text(
            Pos2::new(vp_rect.left() + 4.0, vp_rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            format!("{} (1:{}@{})", vp.name, vp.scale, drawing.paper_size.label()),
            egui::FontId::proportional(11.0),
            frame_color,
        );

        // Resize handles (when selected)
        if is_selected {
            for (pos, _) in handle_positions(vp_rect) {
                let r = Rect::from_center_size(pos, egui::vec2(HANDLE_RADIUS * 2.0, HANDLE_RADIUS * 2.0));
                painter.rect_filled(r, 1.0, HANDLE_COLOR);
            }
        }
    }

    // --- Title block preview ---
    if drawing.show_title_block {
        draw_title_block_preview(painter, view, project, drawing, paper_w, paper_h);
    }

    // --- North arrow preview ---
    if drawing.show_north_arrow {
        let na_pos = view.world_to_screen(glam::Vec2::new(paper_w - 25.0, mt + 5.0));
        let na_size = 12.0 * view.zoom;
        painter.text(
            na_pos, egui::Align2::CENTER_TOP, "N",
            egui::FontId::proportional(na_size.max(8.0)), Color32::from_gray(80),
        );
        let arrow_top = Pos2::new(na_pos.x, na_pos.y + na_size * 0.6);
        let arrow_bot = Pos2::new(na_pos.x, na_pos.y + na_size * 1.8);
        let arr_stroke = Stroke::new(1.5, Color32::from_gray(80));
        painter.line_segment([arrow_top, arrow_bot], arr_stroke);
        let arr_w = na_size * 0.3;
        painter.line_segment([arrow_top, Pos2::new(na_pos.x - arr_w, na_pos.y + na_size)], arr_stroke);
        painter.line_segment([arrow_top, Pos2::new(na_pos.x + arr_w, na_pos.y + na_size)], arr_stroke);
    }

    // --- Scale bar preview ---
    if drawing.show_scale_bar {
        let scale = drawing.viewports.first().map(|v| v.scale).unwrap_or(100);
        let bar_pos = view.world_to_screen(glam::Vec2::new(ml + 5.0, paper_h - mb - 10.0));
        let bar_len_px = (60.0 * view.zoom).max(30.0).min(200.0);
        painter.rect_filled(
            Rect::from_min_size(bar_pos, egui::vec2(bar_len_px, 3.0 * view.zoom)),
            0.0, Color32::from_gray(80),
        );
        painter.text(
            Pos2::new(bar_pos.x + bar_len_px / 2.0, bar_pos.y - 2.0),
            egui::Align2::CENTER_BOTTOM,
            format!("1:{}@{}", scale, drawing.paper_size.label()),
            egui::FontId::proportional(9.0),
            Color32::from_gray(100),
        );
    }

    // --- New-viewport creation preview ---
    if let Some(start) = interaction.create_start {
        if let Some(hover) = response.hover_pos() {
            let s1 = view.world_to_screen(glam::Vec2::new(start[0], start[1]));
            let preview = Rect::from_two_pos(s1, hover);
            painter.rect_stroke(preview, 0.0, Stroke::new(2.0, HANDLE_COLOR), egui::StrokeKind::Outside);
            painter.rect_filled(preview, 0.0, Color32::from_rgba_unmultiplied(70, 130, 210, 20));
        }
    }

    // --- Input handling ---
    // We use the Response from allocate_painter so we integrate with egui properly.

    // Left-click: select viewport or start creation
    if response.clicked() {
        if let Some(hover) = response.interact_pointer_pos() {
            let hit = hit_test_viewports(drawing, view, interaction, hover);
            match hit {
                HitResult::Handle(id, _) | HitResult::Body(id) => {
                    // Just select — drag starts on subsequent frames via response.dragged()
                    if interaction.selected_viewport != Some(id) {
                        actions.push(DrawingViewAction::SelectViewport(Some(id)));
                        interaction.selected_viewport = Some(id);
                    }
                }
                HitResult::None => {
                    // Shift+click on paper = start viewport creation
                    let shift = ui.input(|i| i.modifiers.shift);
                    if shift && paper_rect.contains(hover) {
                        let sheet = view.screen_to_world(hover);
                        interaction.create_start = Some([sheet.x, sheet.y]);
                    } else if interaction.selected_viewport.is_some() {
                        actions.push(DrawingViewAction::SelectViewport(None));
                        interaction.selected_viewport = None;
                    }
                }
            }
        }
    }

    // Left-drag: move/resize viewport or create new one
    if response.dragged_by(egui::PointerButton::Primary) {
        let delta = response.drag_delta();
        let sheet_delta = glam::Vec2::new(delta.x, delta.y) / view.zoom;

        if let Some(hover) = response.hover_pos() {
            // Active drag in progress
            if let Some((vp_id, handle)) = interaction.drag {
                if let Some(vp) = drawing.viewports.iter().find(|v| v.id == vp_id) {
                    let updated = apply_drag(vp, handle, sheet_delta);
                    actions.push(DrawingViewAction::UpdatedViewport(vp_id, updated));
                }
            }
            // Active viewport creation
            else if interaction.create_start.is_some() {
                // Preview is drawn above; finalization happens on release
            }
            // Start a new drag — figure out what we're dragging
            else {
                let hit = hit_test_viewports(drawing, view, interaction, hover);
                match hit {
                    HitResult::Handle(id, handle) => {
                        interaction.drag = Some((id, handle));
                        interaction.selected_viewport = Some(id);
                        actions.push(DrawingViewAction::SelectViewport(Some(id)));
                    }
                    HitResult::Body(id) => {
                        interaction.drag = Some((id, ResizeHandle::Move));
                        if interaction.selected_viewport != Some(id) {
                            interaction.selected_viewport = Some(id);
                            actions.push(DrawingViewAction::SelectViewport(Some(id)));
                        }
                    }
                    HitResult::None => {
                        // Shift+drag on paper = create viewport
                        let shift = ui.input(|i| i.modifiers.shift);
                        if shift && paper_rect.contains(hover) {
                            let sheet = view.screen_to_world(hover);
                            interaction.create_start = Some([sheet.x, sheet.y]);
                        }
                    }
                }
            }
        }
    }

    // Release: end drag or finalize viewport creation
    if response.drag_stopped() {
        if interaction.drag.is_some() {
            interaction.drag = None;
        }
        if let Some(start) = interaction.create_start.take() {
            if let Some(hover) = response.interact_pointer_pos() {
                let end = view.screen_to_world(hover);
                let x = start[0].min(end.x);
                let y = start[1].min(end.y);
                let w = (start[0] - end.x).abs();
                let h = (start[1] - end.y).abs();
                if w > 5.0 && h > 5.0 {
                    let mut vp = DrawingViewport::new("New View", 100);
                    vp.sheet_position = [x, y];
                    vp.sheet_size = [w, h];
                    actions.push(DrawingViewAction::CreatedViewport(vp));
                }
            }
        }
    }

    actions
}

// --- Hit testing ---

enum HitResult {
    Handle(uuid::Uuid, ResizeHandle),
    Body(uuid::Uuid),
    None,
}

fn hit_test_viewports(
    drawing: &Drawing,
    view: &ViewTransform,
    interaction: &DrawingInteraction,
    hover: Pos2,
) -> HitResult {
    // Check resize handles on selected viewport first
    if let Some(sel_id) = interaction.selected_viewport {
        if let Some(vp) = drawing.viewports.iter().find(|v| v.id == sel_id) {
            let rect = vp_screen_rect(vp, view);
            for (pos, handle) in handle_positions(rect) {
                if pos.distance(hover) < HANDLE_RADIUS * 2.5 {
                    return HitResult::Handle(sel_id, handle);
                }
            }
            // Check body of selected viewport
            if rect.contains(hover) {
                return HitResult::Body(sel_id);
            }
        }
    }

    // Check other viewport bodies (top-most = last in list)
    for vp in drawing.viewports.iter().rev() {
        let rect = vp_screen_rect(vp, view);
        if rect.contains(hover) {
            return HitResult::Body(vp.id);
        }
    }

    HitResult::None
}

fn vp_screen_rect(vp: &DrawingViewport, view: &ViewTransform) -> Rect {
    let tl = view.world_to_screen(glam::Vec2::new(vp.sheet_position[0], vp.sheet_position[1]));
    let br = view.world_to_screen(glam::Vec2::new(
        vp.sheet_position[0] + vp.sheet_size[0],
        vp.sheet_position[1] + vp.sheet_size[1],
    ));
    Rect::from_min_max(Pos2::new(tl.x, tl.y), Pos2::new(br.x, br.y))
}

fn handle_positions(rect: Rect) -> [(Pos2, ResizeHandle); 8] {
    [
        (rect.left_top(), ResizeHandle::TopLeft),
        (rect.right_top(), ResizeHandle::TopRight),
        (rect.left_bottom(), ResizeHandle::BottomLeft),
        (rect.right_bottom(), ResizeHandle::BottomRight),
        (Pos2::new(rect.center().x, rect.top()), ResizeHandle::Top),
        (Pos2::new(rect.center().x, rect.bottom()), ResizeHandle::Bottom),
        (Pos2::new(rect.left(), rect.center().y), ResizeHandle::Left),
        (Pos2::new(rect.right(), rect.center().y), ResizeHandle::Right),
    ]
}

fn apply_drag(vp: &DrawingViewport, handle: ResizeHandle, delta: glam::Vec2) -> DrawingViewport {
    let mut u = vp.clone();
    match handle {
        ResizeHandle::Move => {
            u.sheet_position[0] += delta.x;
            u.sheet_position[1] += delta.y;
        }
        ResizeHandle::Right => {
            u.sheet_size[0] = (u.sheet_size[0] + delta.x).max(20.0);
        }
        ResizeHandle::Bottom => {
            u.sheet_size[1] = (u.sheet_size[1] + delta.y).max(20.0);
        }
        ResizeHandle::Left => {
            u.sheet_position[0] += delta.x;
            u.sheet_size[0] = (u.sheet_size[0] - delta.x).max(20.0);
        }
        ResizeHandle::Top => {
            u.sheet_position[1] += delta.y;
            u.sheet_size[1] = (u.sheet_size[1] - delta.y).max(20.0);
        }
        ResizeHandle::BottomRight => {
            u.sheet_size[0] = (u.sheet_size[0] + delta.x).max(20.0);
            u.sheet_size[1] = (u.sheet_size[1] + delta.y).max(20.0);
        }
        ResizeHandle::TopLeft => {
            u.sheet_position[0] += delta.x;
            u.sheet_position[1] += delta.y;
            u.sheet_size[0] = (u.sheet_size[0] - delta.x).max(20.0);
            u.sheet_size[1] = (u.sheet_size[1] - delta.y).max(20.0);
        }
        ResizeHandle::TopRight => {
            u.sheet_position[1] += delta.y;
            u.sheet_size[0] = (u.sheet_size[0] + delta.x).max(20.0);
            u.sheet_size[1] = (u.sheet_size[1] - delta.y).max(20.0);
        }
        ResizeHandle::BottomLeft => {
            u.sheet_position[0] += delta.x;
            u.sheet_size[0] = (u.sheet_size[0] - delta.x).max(20.0);
            u.sheet_size[1] = (u.sheet_size[1] + delta.y).max(20.0);
        }
    }
    u
}

// --- Drawing decorations ---

fn draw_title_block_preview(
    painter: &egui::Painter,
    view: &ViewTransform,
    project: &Project,
    drawing: &Drawing,
    paper_w: f32,
    paper_h: f32,
) {
    let tb_w = (180.0 * paper_w / 841.0).max(80.0);
    let tb_h = 60.0_f32;
    let tb_tl = view.world_to_screen(glam::Vec2::new(paper_w - tb_w - 10.0, paper_h - tb_h - 10.0));
    let tb_br = view.world_to_screen(glam::Vec2::new(paper_w - 10.0, paper_h - 10.0));
    let tb_rect = Rect::from_min_max(Pos2::new(tb_tl.x, tb_tl.y), Pos2::new(tb_br.x, tb_br.y));
    painter.rect_filled(tb_rect, 0.0, Color32::from_gray(250));
    painter.rect_stroke(tb_rect, 0.0, Stroke::new(1.0, Color32::from_gray(100)), egui::StrokeKind::Outside);

    let tx = tb_tl.x + 4.0;
    let name = if drawing.title_block.project_name.is_empty() {
        &project.metadata.name
    } else {
        &drawing.title_block.project_name
    };
    painter.text(
        Pos2::new(tx, tb_tl.y + 4.0), egui::Align2::LEFT_TOP,
        name, egui::FontId::proportional(12.0), Color32::from_gray(60),
    );
    painter.text(
        Pos2::new(tx, tb_tl.y + 20.0), egui::Align2::LEFT_TOP,
        &drawing.title, egui::FontId::proportional(11.0), Color32::from_gray(80),
    );
    let rev_text = drawing.current_revision()
        .map(|r| format!(" Rev {}", r.label))
        .unwrap_or_default();
    painter.text(
        Pos2::new(tx, tb_tl.y + 34.0), egui::Align2::LEFT_TOP,
        format!("Sheet {} | 1:{}@{}{}", drawing.full_sheet_ref(),
            drawing.viewports.first().map(|v| v.scale).unwrap_or(100),
            drawing.paper_size.label(), rev_text),
        egui::FontId::proportional(10.0), Color32::from_gray(100),
    );
    if !drawing.title_block.drawn_by.is_empty() {
        painter.text(
            Pos2::new(tx, tb_tl.y + 46.0), egui::Align2::LEFT_TOP,
            format!("Drawn: {} | {}", drawing.title_block.drawn_by, drawing.title_block.date),
            egui::FontId::proportional(9.0), Color32::from_gray(120),
        );
    }
}

// --- Entity rendering ---

/// Render a width-based shape (wall, path, step, edging) within a viewport.
/// Mirrors `draw_width_shape` from canvas.rs but uses a `to_screen` closure
/// instead of a ViewTransform, matching the viewport coordinate system.
fn draw_width_shape_in_viewport(
    painter: &egui::Painter,
    centerline: &[glam::Vec2],
    width_mm: f32,
    fill: Color32,
    stroke: Stroke,
    to_screen: &dyn Fn(glam::Vec2) -> Pos2,
) {
    if centerline.len() < 2 {
        return;
    }
    let hw = width_mm / 2.0;

    // Fill each segment as a quad using miter-joined offsets
    let (left, right) = offset_polyline_sides(centerline, hw);
    for i in 0..centerline.len() - 1 {
        let quad = vec![
            to_screen(left[i]),
            to_screen(left[i + 1]),
            to_screen(right[i + 1]),
            to_screen(right[i]),
        ];
        painter.add(egui::Shape::convex_polygon(quad, fill, Stroke::NONE));
    }

    // Outline using the full offset polygon
    let outline = offset_polyline(centerline, hw);
    let screen_pts: Vec<Pos2> = outline.iter().map(|v| to_screen(*v)).collect();
    painter.add(egui::Shape::Path(egui::epaint::PathShape {
        points: screen_pts,
        closed: true,
        fill: Color32::TRANSPARENT,
        stroke: egui::epaint::PathStroke::from(stroke),
    }));
}

fn draw_entity_simple(
    painter: &egui::Painter,
    geometry: &Geometry,
    stroke: Stroke,
    colour: Color32,
    to_screen: &dyn Fn(glam::Vec2) -> Pos2,
) {
    match geometry {
        Geometry::None => {}
        Geometry::Point(p) => {
            painter.circle_filled(to_screen(*p), 2.0, colour);
        }
        Geometry::Line(l) => {
            painter.line_segment([to_screen(l.start), to_screen(l.end)], stroke);
        }
        Geometry::Polyline(pl) => {
            if pl.vertices.len() >= 2 {
                let pts: Vec<Pos2> = pl.vertices.iter().map(|v| to_screen(*v)).collect();
                for w in pts.windows(2) {
                    painter.line_segment([w[0], w[1]], stroke);
                }
                if pl.closed {
                    painter.line_segment([*pts.last().unwrap(), pts[0]], stroke);
                }
            }
        }
        Geometry::Polygon(pg) => {
            if pg.vertices.len() >= 3 {
                let pts: Vec<Pos2> = pg.vertices.iter().map(|v| to_screen(*v)).collect();
                let fill = Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), 30);
                painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
            }
        }
        Geometry::Circle(c) => {
            let center = to_screen(c.center);
            let edge = to_screen(c.center + glam::Vec2::new(c.radius, 0.0));
            let radius_px = (edge.x - center.x).abs();
            painter.circle_stroke(center, radius_px, stroke);
        }
        Geometry::Arc(a) => {
            let segments = 32;
            let mut sweep = a.end_angle - a.start_angle;
            if sweep < 0.0 { sweep += std::f32::consts::TAU; }
            let step = sweep / segments as f32;
            let pts: Vec<Pos2> = (0..=segments)
                .map(|i| {
                    let angle = a.start_angle + i as f32 * step;
                    to_screen(glam::Vec2::new(
                        a.center.x + a.radius * angle.cos(),
                        a.center.y + a.radius * angle.sin(),
                    ))
                })
                .collect();
            for w in pts.windows(2) {
                painter.line_segment([w[0], w[1]], stroke);
            }
        }
        Geometry::Rectangle(r) => {
            let corners = [
                r.center + glam::Vec2::new(-r.half_size.x, -r.half_size.y),
                r.center + glam::Vec2::new(r.half_size.x, -r.half_size.y),
                r.center + glam::Vec2::new(r.half_size.x, r.half_size.y),
                r.center + glam::Vec2::new(-r.half_size.x, r.half_size.y),
            ];
            let pts: Vec<Pos2> = corners.iter().map(|v| to_screen(*v)).collect();
            for i in 0..4 {
                painter.line_segment([pts[i], pts[(i + 1) % 4]], stroke);
            }
        }
        Geometry::Text(t) => {
            let pos = to_screen(t.position);
            painter.text(
                pos, egui::Align2::LEFT_TOP, &t.text,
                egui::FontId::proportional(t.font_size.max(8.0)), colour,
            );
        }
        Geometry::BezierPath(bp) => {
            let segments = bp.to_bezier_segments();
            for seg in &segments {
                let steps = 16;
                let pts: Vec<Pos2> = (0..=steps)
                    .map(|i| {
                        let t = i as f32 / steps as f32;
                        let t2 = t * t;
                        let t3 = t2 * t;
                        let mt = 1.0 - t;
                        let mt2 = mt * mt;
                        let mt3 = mt2 * mt;
                        let p = seg.p0 * mt3
                            + seg.p1 * (3.0 * mt2 * t)
                            + seg.p2 * (3.0 * mt * t2)
                            + seg.p3 * t3;
                        to_screen(p)
                    })
                    .collect();
                for w in pts.windows(2) {
                    painter.line_segment([w[0], w[1]], stroke);
                }
            }
        }
        Geometry::Dimension(dim) => {
            use grove_core::drawing::dimension::DimensionData;
            match dim {
                DimensionData::Linear { start, end, offset, text_override } => {
                    let dir = (*end - *start).normalize_or_zero();
                    let normal = glam::Vec2::new(-dir.y, dir.x) * *offset;
                    let ext_s = *start + normal;
                    let ext_e = *end + normal;
                    painter.line_segment([to_screen(*start), to_screen(ext_s)], Stroke::new(0.5, colour));
                    painter.line_segment([to_screen(*end), to_screen(ext_e)], Stroke::new(0.5, colour));
                    painter.line_segment([to_screen(ext_s), to_screen(ext_e)], stroke);
                    let mid = (ext_s + ext_e) * 0.5;
                    let measurement = (*end - *start).length();
                    let label = text_override.clone().unwrap_or_else(|| format!("{:.0}", measurement));
                    painter.text(to_screen(mid), egui::Align2::CENTER_BOTTOM, label,
                        egui::FontId::proportional(10.0), colour);
                }
                DimensionData::Angular { center, start_angle, end_angle, radius } => {
                    let segments = 24;
                    let mut sweep = end_angle - start_angle;
                    if sweep < 0.0 { sweep += std::f32::consts::TAU; }
                    let step = sweep / segments as f32;
                    let pts: Vec<Pos2> = (0..=segments)
                        .map(|i| {
                            let angle = start_angle + i as f32 * step;
                            to_screen(*center + glam::Vec2::new(radius * angle.cos(), radius * angle.sin()))
                        })
                        .collect();
                    for w in pts.windows(2) {
                        painter.line_segment([w[0], w[1]], stroke);
                    }
                }
                DimensionData::Radial { center, radius, angle } => {
                    let end = *center + glam::Vec2::new(*radius * angle.cos(), *radius * angle.sin());
                    painter.line_segment([to_screen(*center), to_screen(end)], stroke);
                    let mid = (*center + end) * 0.5;
                    painter.text(to_screen(mid), egui::Align2::CENTER_BOTTOM,
                        format!("R{:.0}", radius), egui::FontId::proportional(10.0), colour);
                }
            }
        }
    }
}
