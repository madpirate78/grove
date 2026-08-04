use egui::{Color32, Pos2, Rect, Stroke, Ui};
use grove_core::drawing::drawing::{Drawing, DrawingAnnotation, DrawingViewport, PlantStyle};
use grove_core::drawing::layout::is_layer_visible_in_viewport;
use grove_core::entity::Geometry;
use grove_core::garden::colour::resolve_colour;
use grove_core::project::Project;

use super::canvas::{render_polygon_mesh, render_width_shape};
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
    /// Selected viewport ID for showing handles.
    pub selected_viewport: Option<uuid::Uuid>,
    /// When true, next click places an annotation on the paper.
    pub annotation_mode: bool,
    /// Annotation currently being dragged.
    pub dragging_annotation: Option<uuid::Uuid>,
    /// When Some, next click in a viewport sets the leader target for this annotation.
    pub setting_leader: Option<uuid::Uuid>,
    /// Selected annotation for property editing.
    pub selected_annotation: Option<uuid::Uuid>,
    /// Viewport whose model_center is being panned via Shift+left-drag.
    pub panning_viewport: Option<uuid::Uuid>,
}

/// Action produced by the drawing view for the app to process.
pub enum DrawingViewAction {
    UpdatedViewport(uuid::Uuid, DrawingViewport),
    SelectViewport(Option<uuid::Uuid>),
    CreatedAnnotation(DrawingAnnotation),
    UpdatedAnnotation(uuid::Uuid, DrawingAnnotation),
    SelectAnnotation(Option<uuid::Uuid>),
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
            let resolved = resolve_colour(entity, vp.colour_mode);
            let colour = Color32::from_rgba_premultiplied(
                resolved[0], resolved[1], resolved[2], resolved[3],
            );
            let paper_sw = grove_core::export::svg::stroke_weight_for_domain(&entity.domain);
            let screen_sw = (paper_sw / 0.35).max(0.5);
            let stroke = Stroke::new(screen_sw, colour);

            // Width-aware rendering for walls, paths, steps, edging
            if let Some(w) = entity.domain.width_mm() {
                if let Some(centerline) = entity.geometry.centerline_points() {
                    let fill = Color32::from_rgba_unmultiplied(
                        resolved[0], resolved[1], resolved[2], 30,
                    );
                    render_width_shape(&clipped, &centerline, w, fill, stroke, to_screen);
                    continue;
                }
            }

            draw_entity_simple(&clipped, &entity.geometry, &entity.domain, stroke, colour, resolved, &to_screen, effective_scale, vp.plant_style);
        }

        // Viewport frame
        let is_panning = interaction.panning_viewport == Some(vp.id);
        let is_selected = interaction.selected_viewport == Some(vp.id);
        let frame_color = if is_panning { Color32::from_rgb(255, 180, 50) } else if is_selected { HANDLE_COLOR } else { VIEWPORT_FRAME_COLOR };
        let frame_width = if is_panning { 2.0 } else if is_selected { 1.5 } else { 1.0 };
        painter.rect_stroke(vp_rect, 0.0, Stroke::new(frame_width, frame_color), egui::StrokeKind::Outside);

        // Viewport label (scales with zoom so it stays proportional to paper)
        let label_size = (3.0 * view.zoom).clamp(4.0, 24.0);
        painter.text(
            Pos2::new(vp_rect.left() + 2.0 * view.zoom, vp_rect.top() - 1.0 * view.zoom),
            egui::Align2::LEFT_BOTTOM,
            format!("{} (1:{}@{})", vp.name, vp.scale, drawing.paper_size.label()),
            egui::FontId::proportional(label_size),
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

    // --- Annotations ---
    draw_annotations(painter, view, drawing, interaction);

    // --- Title block preview ---
    if drawing.show_title_block {
        draw_title_block_preview(painter, view, project, drawing, paper_w, paper_h);
    }

    // --- North arrow preview ---
    if drawing.show_north_arrow {
        let na_pos = view.world_to_screen(glam::Vec2::new(paper_w - 25.0, mt + 5.0));
        let na_size = (7.5 * view.zoom).clamp(6.0, 40.0);
        painter.text(
            na_pos, egui::Align2::CENTER_TOP, "N",
            egui::FontId::proportional(na_size), Color32::from_gray(80),
        );
        let arrow_top = Pos2::new(na_pos.x, na_pos.y + na_size * 0.6);
        let arrow_bot = Pos2::new(na_pos.x, na_pos.y + na_size * 1.8);
        let arr_stroke = Stroke::new(1.0, Color32::from_gray(80));
        painter.line_segment([arrow_top, arrow_bot], arr_stroke);
        let arr_w = na_size * 0.3;
        painter.line_segment([arrow_top, Pos2::new(na_pos.x - arr_w, na_pos.y + na_size)], arr_stroke);
        painter.line_segment([arrow_top, Pos2::new(na_pos.x + arr_w, na_pos.y + na_size)], arr_stroke);
    }

    // --- Scale bar preview ---
    if drawing.show_scale_bar {
        let scale = drawing.viewports.first().map(|v| v.scale).unwrap_or(100);
        let bar_pos = view.world_to_screen(glam::Vec2::new(ml + 5.0, paper_h - mb - 10.0));
        let bar_len_px = 50.0 * view.zoom;
        painter.rect_filled(
            Rect::from_min_size(bar_pos, egui::vec2(bar_len_px, 2.5 * view.zoom)),
            0.0, Color32::from_gray(80),
        );
        let label_size = (3.0 * view.zoom).clamp(4.0, 20.0);
        painter.text(
            Pos2::new(bar_pos.x + bar_len_px / 2.0, bar_pos.y - 1.0 * view.zoom),
            egui::Align2::CENTER_BOTTOM,
            format!("1:{}@{}", scale, drawing.paper_size.label()),
            egui::FontId::proportional(label_size),
            Color32::from_gray(100),
        );
    }

    // --- Input handling ---
    // We use the Response from allocate_painter so we integrate with egui properly.

    // Left-click: annotations first (they render on top), then viewports
    if response.clicked() {
        if let Some(hover) = response.interact_pointer_pos() {
            // Setting leader mode: click in viewport → set leader target
            if let Some(ann_id) = interaction.setting_leader {
                if let Some((vp, _)) = hit_test_viewport_body(drawing, view, hover) {
                    let model_pt = screen_to_model_via_viewport(hover, vp, view);
                    let mut ann = drawing.annotations.iter()
                        .find(|a| a.id == ann_id).cloned();
                    if let Some(ref mut a) = ann {
                        a.leader_target_model = Some(model_pt);
                        a.viewport_id = Some(vp.id);
                        actions.push(DrawingViewAction::UpdatedAnnotation(ann_id, a.clone()));
                    }
                    interaction.setting_leader = None;
                }
            }
            // Annotation placement mode: click on paper → create annotation
            else if interaction.annotation_mode {
                let sheet = view.screen_to_world(hover);
                if paper_rect.contains(hover) {
                    let ann = DrawingAnnotation::new("Label", [sheet.x, sheet.y]);
                    let ann_id = ann.id;
                    actions.push(DrawingViewAction::CreatedAnnotation(ann));
                    interaction.selected_annotation = Some(ann_id);
                    actions.push(DrawingViewAction::SelectAnnotation(Some(ann_id)));
                }
                interaction.annotation_mode = false;
            }
            // Normal click: check annotations first, then viewports
            else {
                let ann_hit = hit_test_annotations(drawing, view, hover);
                if let Some(ann_id) = ann_hit {
                    interaction.selected_annotation = Some(ann_id);
                    actions.push(DrawingViewAction::SelectAnnotation(Some(ann_id)));
                } else {
                    if interaction.selected_annotation.is_some() {
                        interaction.selected_annotation = None;
                        actions.push(DrawingViewAction::SelectAnnotation(None));
                    }
                    let hit = hit_test_viewports(drawing, view, interaction, hover);
                    match hit {
                        HitResult::Handle(id, _) | HitResult::Body(id) => {
                            if interaction.selected_viewport != Some(id) {
                                actions.push(DrawingViewAction::SelectViewport(Some(id)));
                                interaction.selected_viewport = Some(id);
                            }
                        }
                        HitResult::None => {
                            if interaction.selected_viewport.is_some() {
                                actions.push(DrawingViewAction::SelectViewport(None));
                                interaction.selected_viewport = None;
                            }
                        }
                    }
                }
            }
        }
    }

    // Left-drag: annotation drag or viewport move/resize
    // (Skip if Shift+drag is panning a viewport's model_center — handled in app.rs)
    if response.dragged_by(egui::PointerButton::Primary) && interaction.panning_viewport.is_none() {
        let delta = response.drag_delta();
        let sheet_delta = glam::Vec2::new(delta.x, delta.y) / view.zoom;

        if let Some(hover) = response.hover_pos() {
            // Dragging an annotation
            if let Some(ann_id) = interaction.dragging_annotation {
                if let Some(ann) = drawing.annotations.iter().find(|a| a.id == ann_id) {
                    let mut updated = ann.clone();
                    updated.sheet_position[0] += sheet_delta.x;
                    updated.sheet_position[1] += sheet_delta.y;
                    actions.push(DrawingViewAction::UpdatedAnnotation(ann_id, updated));
                }
            }
            // Active viewport drag in progress
            else if let Some((vp_id, handle)) = interaction.drag {
                if let Some(vp) = drawing.viewports.iter().find(|v| v.id == vp_id) {
                    let updated = apply_drag(vp, handle, sheet_delta);
                    actions.push(DrawingViewAction::UpdatedViewport(vp_id, updated));
                }
            }
            // Start a new drag — check annotations first, then viewports
            else {
                let ann_hit = hit_test_annotations(drawing, view, hover);
                if let Some(ann_id) = ann_hit {
                    interaction.dragging_annotation = Some(ann_id);
                    interaction.selected_annotation = Some(ann_id);
                    actions.push(DrawingViewAction::SelectAnnotation(Some(ann_id)));
                } else {
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
                        HitResult::None => {}
                    }
                }
            }
        }
    }

    // Release: end drag
    if response.drag_stopped() {
        interaction.dragging_annotation = None;
        if interaction.drag.is_some() {
            interaction.drag = None;
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

pub(crate) fn vp_screen_rect(vp: &DrawingViewport, view: &ViewTransform) -> Rect {
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

    // Scale text sizes with zoom so they stay proportional to the paper
    let z = view.zoom;
    let tx = tb_tl.x + 4.0 * z;
    let name = if drawing.title_block.project_name.is_empty() {
        &project.metadata.name
    } else {
        &drawing.title_block.project_name
    };
    painter.text(
        Pos2::new(tx, tb_tl.y + 4.0 * z), egui::Align2::LEFT_TOP,
        name, egui::FontId::proportional((7.0 * z).clamp(4.0, 36.0)), Color32::from_gray(60),
    );
    painter.text(
        Pos2::new(tx, tb_tl.y + 14.0 * z), egui::Align2::LEFT_TOP,
        &drawing.title, egui::FontId::proportional((5.0 * z).clamp(4.0, 30.0)), Color32::from_gray(80),
    );
    let rev_text = drawing.current_revision()
        .map(|r| format!(" Rev {}", r.label))
        .unwrap_or_default();
    painter.text(
        Pos2::new(tx, tb_tl.y + 22.0 * z), egui::Align2::LEFT_TOP,
        format!("Sheet {} | 1:{}@{}{}", drawing.full_sheet_ref(),
            drawing.viewports.first().map(|v| v.scale).unwrap_or(100),
            drawing.paper_size.label(), rev_text),
        egui::FontId::proportional((4.0 * z).clamp(4.0, 26.0)), Color32::from_gray(100),
    );
    if !drawing.title_block.drawn_by.is_empty() {
        painter.text(
            Pos2::new(tx, tb_tl.y + 30.0 * z), egui::Align2::LEFT_TOP,
            format!("Drawn: {} | {}", drawing.title_block.drawn_by, drawing.title_block.date),
            egui::FontId::proportional((4.0 * z).clamp(4.0, 24.0)), Color32::from_gray(120),
        );
    }
}

// --- Entity rendering ---

#[allow(clippy::too_many_arguments)]
fn draw_entity_simple(
    painter: &egui::Painter,
    geometry: &Geometry,
    domain: &grove_core::entity::DomainData,
    stroke: Stroke,
    colour: Color32,
    raw_colour: [u8; 4],
    to_screen: &dyn Fn(glam::Vec2) -> Pos2,
    px_per_mm: f32,
    plant_style: PlantStyle,
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
            let fill = if matches!(domain, grove_core::entity::DomainData::None) {
                Color32::TRANSPARENT
            } else {
                Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), 30)
            };
            render_polygon_mesh(painter, &pg.vertices, &pg.holes, fill, stroke, to_screen);
        }
        Geometry::Circle(c) => {
            let center = to_screen(c.center);
            let edge = to_screen(c.center + glam::Vec2::new(c.radius, 0.0));
            let radius_px = (edge.x - center.x).abs();

            if matches!(domain, grove_core::entity::DomainData::Plant(_)) {
                draw_plant_disc(painter, domain, center, radius_px, raw_colour, plant_style);
            } else {
                painter.circle_stroke(center, radius_px, stroke);
            }
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
            let font_px = (t.font_size * px_per_mm).clamp(4.0, 200.0);
            painter.text(
                pos, egui::Align2::LEFT_TOP, &t.text,
                egui::FontId::proportional(font_px), colour,
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
                    let dim_font = (3.0 * px_per_mm).clamp(4.0, 40.0);
                    painter.text(to_screen(mid), egui::Align2::CENTER_BOTTOM, label,
                        egui::FontId::proportional(dim_font), colour);
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
                    let dim_font = (3.0 * px_per_mm).clamp(4.0, 40.0);
                    painter.text(to_screen(mid), egui::Align2::CENTER_BOTTOM,
                        format!("R{:.0}", radius), egui::FontId::proportional(dim_font), colour);
                }
            }
        }
    }
}

// --- Plant style rendering ---

fn plant_label(domain: &grove_core::entity::DomainData) -> Option<&str> {
    if let grove_core::entity::DomainData::Plant(ref pd) = domain {
        if !pd.common_name.is_empty() {
            Some(pd.common_name.as_str())
        } else {
            Some(pd.botanical_name.as_str())
        }
    } else {
        None
    }
}

fn draw_plant_disc(
    painter: &egui::Painter,
    domain: &grove_core::entity::DomainData,
    center: Pos2,
    radius_px: f32,
    raw_colour: [u8; 4],
    style: PlantStyle,
) {
    let r = raw_colour[0];
    let g = raw_colour[1];
    let b = raw_colour[2];
    let fill_soft = Color32::from_rgba_unmultiplied(r, g, b, 60);
    let stroke_col = Color32::from_rgba_unmultiplied(r, g, b, 200);
    let label_colour = Color32::from_rgba_unmultiplied(240, 240, 245, 200);

    let draw_label = |font_scale: f32, colour: Color32| {
        if radius_px <= 8.0 {
            return;
        }
        if let Some(label) = plant_label(domain) {
            let font_size = (radius_px * font_scale).clamp(8.0, 14.0);
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(font_size),
                colour,
            );
        }
    };

    match style {
        PlantStyle::Simple => {
            painter.circle_filled(center, radius_px, fill_soft);
            painter.circle_stroke(center, radius_px, Stroke::new(1.5, stroke_col));
            draw_label(0.35, label_colour);
        }
        PlantStyle::Outline => {
            painter.circle_stroke(center, radius_px, Stroke::new(1.5, stroke_col));
            draw_label(0.35, stroke_col);
        }
        PlantStyle::Silhouette => {
            let solid = Color32::from_rgba_unmultiplied(r, g, b, 230);
            painter.circle_filled(center, radius_px, solid);
        }
        PlantStyle::Canopy => {
            // Outer stroked ring + soft fill + an inner concentric ring hinting at canopy foliage.
            painter.circle_filled(center, radius_px, fill_soft);
            painter.circle_stroke(center, radius_px, Stroke::new(1.5, stroke_col));
            let inner = radius_px * 0.6;
            painter.circle_stroke(center, inner, Stroke::new(1.0, stroke_col));
            draw_label(0.32, label_colour);
        }
        PlantStyle::Numbered => {
            // Solid-enough disc + small key number (placeholder: label's first char).
            painter.circle_filled(center, radius_px, fill_soft);
            painter.circle_stroke(center, radius_px, Stroke::new(1.5, stroke_col));
            if radius_px > 6.0 {
                let key = plant_label(domain)
                    .and_then(|s| s.chars().next())
                    .map(|c| c.to_ascii_uppercase().to_string())
                    .unwrap_or_else(|| "?".to_string());
                let font_size = (radius_px * 0.8).clamp(10.0, 18.0);
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    key,
                    egui::FontId::proportional(font_size),
                    stroke_col,
                );
            }
        }
    }
}

// --- Annotation rendering ---

const ANNOTATION_COLOR: Color32 = Color32::from_rgb(50, 50, 50);
const ANNOTATION_SELECTED_COLOR: Color32 = Color32::from_rgb(70, 130, 210);
const LEADER_COLOR: Color32 = Color32::from_rgb(100, 100, 100);

fn draw_annotations(
    painter: &egui::Painter,
    view: &ViewTransform,
    drawing: &Drawing,
    interaction: &DrawingInteraction,
) {
    for ann in &drawing.annotations {
        let is_selected = interaction.selected_annotation == Some(ann.id);
        let text_color = if is_selected { ANNOTATION_SELECTED_COLOR } else { ANNOTATION_COLOR };
        let font_size_px = ann.font_size * view.zoom;

        let screen_pos = view.world_to_screen(glam::Vec2::new(
            ann.sheet_position[0], ann.sheet_position[1],
        ));

        // Leader line (draw before text so it goes behind)
        if let (Some(target), Some(vp_id)) = (ann.leader_target_model, ann.viewport_id) {
            if let Some(vp) = drawing.viewports.iter().find(|v| v.id == vp_id) {
                let sheet_target = model_to_sheet(target, vp);
                let target_screen = view.world_to_screen(glam::Vec2::new(sheet_target[0], sheet_target[1]));
                painter.line_segment([screen_pos, target_screen], Stroke::new(0.8, LEADER_COLOR));
                // Filled arrowhead at target
                let dir = egui::vec2(target_screen.x - screen_pos.x, target_screen.y - screen_pos.y);
                let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
                if len > 1.0 {
                    let d = dir / len;
                    let perp = egui::vec2(-d.y, d.x);
                    let arrow_size = 4.0;
                    let tip = target_screen;
                    let left = Pos2::new(
                        tip.x - d.x * arrow_size + perp.x * arrow_size * 0.4,
                        tip.y - d.y * arrow_size + perp.y * arrow_size * 0.4,
                    );
                    let right = Pos2::new(
                        tip.x - d.x * arrow_size - perp.x * arrow_size * 0.4,
                        tip.y - d.y * arrow_size - perp.y * arrow_size * 0.4,
                    );
                    painter.add(egui::Shape::convex_polygon(
                        vec![tip, left, right],
                        LEADER_COLOR,
                        Stroke::NONE,
                    ));
                }
            }
        }

        // Bordered background
        if ann.bordered {
            let text_size = egui::vec2(
                ann.text.len() as f32 * font_size_px * 0.5,
                font_size_px * 1.3,
            );
            let bg_rect = Rect::from_min_size(
                Pos2::new(screen_pos.x - 2.0, screen_pos.y - 1.0),
                text_size + egui::vec2(4.0, 2.0),
            );
            painter.rect_filled(bg_rect, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 220));
            let border_color = if is_selected { ANNOTATION_SELECTED_COLOR } else { Color32::from_gray(160) };
            painter.rect_stroke(bg_rect, 1.0, Stroke::new(0.5, border_color), egui::StrokeKind::Outside);
        }

        // Text
        painter.text(
            screen_pos, egui::Align2::LEFT_TOP, &ann.text,
            egui::FontId::proportional(font_size_px.max(6.0)), text_color,
        );
    }
}

/// Convert model-space point to sheet coords through a viewport.
fn model_to_sheet(model_pt: [f32; 2], vp: &DrawingViewport) -> [f32; 2] {
    let vp_cx = vp.sheet_position[0] + vp.sheet_size[0] / 2.0;
    let vp_cy = vp.sheet_position[1] + vp.sheet_size[1] / 2.0;
    [
        vp_cx + (model_pt[0] - vp.model_center[0]) / vp.scale as f32,
        vp_cy + (model_pt[1] - vp.model_center[1]) / vp.scale as f32,
    ]
}

/// Convert screen position to model-space through a viewport.
fn screen_to_model_via_viewport(screen_pos: Pos2, vp: &DrawingViewport, view: &ViewTransform) -> [f32; 2] {
    let sheet = view.screen_to_world(screen_pos);
    let vp_cx = vp.sheet_position[0] + vp.sheet_size[0] / 2.0;
    let vp_cy = vp.sheet_position[1] + vp.sheet_size[1] / 2.0;
    [
        (sheet.x - vp_cx) * vp.scale as f32 + vp.model_center[0],
        (sheet.y - vp_cy) * vp.scale as f32 + vp.model_center[1],
    ]
}

/// Hit-test annotations. Returns the first annotation whose text rect contains hover.
fn hit_test_annotations(drawing: &Drawing, view: &ViewTransform, hover: Pos2) -> Option<uuid::Uuid> {
    // Check in reverse order (last drawn = on top)
    for ann in drawing.annotations.iter().rev() {
        let screen_pos = view.world_to_screen(glam::Vec2::new(
            ann.sheet_position[0], ann.sheet_position[1],
        ));
        let font_size_px = ann.font_size * view.zoom;
        let text_size = egui::vec2(
            ann.text.len() as f32 * font_size_px * 0.5,
            font_size_px * 1.3,
        );
        let hit_rect = Rect::from_min_size(
            Pos2::new(screen_pos.x - 4.0, screen_pos.y - 4.0),
            text_size + egui::vec2(8.0, 8.0),
        );
        if hit_rect.contains(hover) {
            return Some(ann.id);
        }
    }
    None
}

/// Hit-test viewport bodies, returning the viewport and its index.
pub(crate) fn hit_test_viewport_body<'a>(
    drawing: &'a Drawing,
    view: &ViewTransform,
    hover: Pos2,
) -> Option<(&'a DrawingViewport, usize)> {
    for (i, vp) in drawing.viewports.iter().enumerate().rev() {
        let rect = vp_screen_rect(vp, view);
        if rect.contains(hover) {
            return Some((vp, i));
        }
    }
    None
}

