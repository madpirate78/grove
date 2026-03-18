use glam::Vec2;

use grove_core::commands::AddMultipleEntitiesCommand;
use grove_core::entity::{CircleData, DomainData, Entity, Geometry};
use grove_core::garden::colour::category_colour;
use grove_core::garden::distribute;
use grove_core::garden::plant::{DistributionMethod, Plant, PlantData};
use grove_core::snap::{self, SnapResult};

use super::{snap_candidates_from_entities, Tool, ToolContext, ToolResult};
use super::draw_line::draw_snap_indicator;
use crate::view::viewport::ViewTransform;

/// A species entry for proportional multi-species filling.
pub struct SpeciesEntry {
    pub plant_data: PlantData,
    pub colour: [u8; 4],
    pub radius_mm: f32,
    pub proportion: f32,
}

/// State machine for group placement.
#[derive(Debug, Clone, PartialEq)]
enum GroupState {
    /// Drawing the boundary polygon (clicking vertices).
    DrawingBoundary,
    /// Boundary complete, configuring distribution.
    Configuring,
}

/// Tool for placing a group of plants within a drawn polygon boundary.
/// 1. Click to define boundary vertices (double-click or Enter to close)
/// 2. Configure distribution method + spacing
/// 3. Generate and place the group
pub struct PlaceGroupTool {
    plant: Plant,
    state: GroupState,
    boundary: Vec<Vec2>,
    current_pos: Vec2,
    snap: Option<SnapResult>,
    pub layer_id: uuid::Uuid,

    // Configuration
    distribution: DistributionMethod,
    spacing_mm: f32,
    preview_points: Vec<Vec2>,

    // Multi-species mode (None = single-species)
    species_mix: Option<Vec<SpeciesEntry>>,
    preview_assignments: Vec<usize>,
}

impl PlaceGroupTool {
    pub fn new(plant: Plant) -> Self {
        let spacing = plant.spread_mm as f32;
        let spacing = if spacing > 0.0 { spacing } else { 400.0 };
        Self {
            plant,
            state: GroupState::DrawingBoundary,
            boundary: Vec::new(),
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
            distribution: DistributionMethod::OffsetGrid,
            spacing_mm: spacing,
            preview_points: Vec::new(),
            species_mix: None,
            preview_assignments: Vec::new(),
        }
    }

    /// Create a group tool in multi-species mode from a mix of species.
    pub fn new_from_mix(mix: Vec<SpeciesEntry>) -> Self {
        // Derive spacing from minimum spread across species
        let min_spacing = mix
            .iter()
            .map(|s| s.radius_mm * 2.0)
            .fold(f32::INFINITY, f32::min);
        let spacing = if min_spacing > 0.0 && min_spacing.is_finite() { min_spacing } else { 400.0 };
        // Use first species plant for fallback fields
        let plant = Plant {
            id: mix[0].plant_data.plant_id,
            botanical_name: mix[0].plant_data.botanical_name.clone(),
            common_name: mix[0].plant_data.common_name.clone(),
            category: mix[0].plant_data.category,
            height_mm: mix[0].plant_data.height_mm,
            spread_mm: mix[0].plant_data.spread_mm,
            colour_code: mix[0].plant_data.colour_code.clone(),
            flower_colour: mix[0].plant_data.flower_colour.clone(),
            foliage_colour: mix[0].plant_data.foliage_colour.clone(),
            evergreen: mix[0].plant_data.evergreen,
            notes: String::new(),
        };
        Self {
            plant,
            state: GroupState::DrawingBoundary,
            boundary: Vec::new(),
            current_pos: Vec2::ZERO,
            snap: None,
            layer_id: uuid::Uuid::nil(),
            distribution: DistributionMethod::OffsetGrid,
            spacing_mm: spacing,
            preview_points: Vec::new(),
            species_mix: Some(mix),
            preview_assignments: Vec::new(),
        }
    }

    pub fn set_plant(&mut self, plant: Plant) {
        let spacing = plant.spread_mm as f32;
        self.spacing_mm = if spacing > 0.0 { spacing } else { 400.0 };
        self.plant = plant;
        self.update_preview();
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

    fn update_preview(&mut self) {
        if self.boundary.len() < 3 || self.state != GroupState::Configuring {
            self.preview_points.clear();
            self.preview_assignments.clear();
            return;
        }
        self.preview_points = match self.distribution {
            DistributionMethod::Grid => {
                distribute::grid_distribution(&self.boundary, self.spacing_mm)
            }
            DistributionMethod::OffsetGrid => {
                distribute::offset_grid_distribution(&self.boundary, self.spacing_mm)
            }
            DistributionMethod::PoissonDisk => {
                distribute::poisson_disk_distribution(&self.boundary, self.spacing_mm, 42)
            }
        };

        // Assign species for multi-species mode
        if let Some(ref mix) = self.species_mix {
            let proportions: Vec<f32> = mix.iter().map(|s| s.proportion).collect();
            self.preview_assignments = distribute::assign_species_proportional(
                self.preview_points.len(),
                &proportions,
                42,
            );
        } else {
            self.preview_assignments.clear();
        }
    }

    fn finish_boundary(&mut self) {
        if self.boundary.len() >= 3 {
            self.state = GroupState::Configuring;
            self.update_preview();
        }
    }

    fn generate_group(&mut self) -> ToolResult {
        if self.preview_points.is_empty() {
            self.state = GroupState::DrawingBoundary;
            self.boundary.clear();
            return ToolResult::Continue;
        }

        let entities: Vec<Entity> = if let Some(ref mix) = self.species_mix {
            // Multi-species mode
            self.preview_points
                .iter()
                .enumerate()
                .map(|(i, &pos)| {
                    let species_idx = self.preview_assignments.get(i).copied().unwrap_or(0);
                    let species = &mix[species_idx.min(mix.len() - 1)];
                    let r = if species.radius_mm > 0.0 { species.radius_mm } else { 200.0 };
                    let mut entity = Entity::new(
                        &species.plant_data.botanical_name,
                        Geometry::Circle(CircleData {
                            center: pos,
                            radius: r,
                        }),
                        DomainData::Plant(species.plant_data.clone()),
                    );
                    entity.layer_id = self.layer_id;
                    entity.colour = species.colour;
                    entity
                })
                .collect()
        } else {
            // Single-species mode
            let radius = self.plant.radius_mm();
            let r = if radius > 0.0 { radius } else { 200.0 };
            let colour = category_colour(self.plant.category);
            self.preview_points
                .iter()
                .map(|&pos| {
                    let mut entity = Entity::new(
                        &self.plant.botanical_name,
                        Geometry::Circle(CircleData {
                            center: pos,
                            radius: r,
                        }),
                        DomainData::Plant(PlantData::from_plant(&self.plant)),
                    );
                    entity.layer_id = self.layer_id;
                    entity.colour = colour;
                    entity
                })
                .collect()
        };

        let count = entities.len();
        let cmd = AddMultipleEntitiesCommand::new(entities);

        // Reset state for another group
        self.boundary.clear();
        self.preview_points.clear();
        self.preview_assignments.clear();
        self.state = GroupState::DrawingBoundary;

        log::info!("Placed group of {} plants", count);
        ToolResult::EmitAndContinue(Box::new(cmd))
    }
}

impl Tool for PlaceGroupTool {
    fn name(&self) -> &str {
        "Place Group"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn on_mouse_down(&mut self, pos: Vec2, ctx: &ToolContext) -> ToolResult {
        self.layer_id = if self.layer_id.is_nil() { ctx.active_layer_id } else { self.layer_id };
        match self.state {
            GroupState::DrawingBoundary => {
                let snapped = self.snapped_pos(pos, ctx);
                self.boundary.push(snapped);
                ToolResult::Continue
            }
            GroupState::Configuring => {
                // Click in config mode = confirm and place
                self.generate_group()
            }
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, ctx: &ToolContext) {
        self.current_pos = self.snapped_pos(pos, ctx);

        if ctx.snap_config.enabled && self.state == GroupState::DrawingBoundary {
            let candidates = snap_candidates_from_entities(pos, ctx.entities, ctx.snap_config, ctx.smart_points);
            let tolerance = ctx.snap_config.snap_distance / ctx.viewport_zoom;
            self.snap = snap::best_snap(&candidates, tolerance);
        } else {
            self.snap = None;
        }
    }

    fn on_double_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        if self.state == GroupState::DrawingBoundary {
            // Remove the duplicate point from the double click
            if self.boundary.len() > 3 {
                self.boundary.pop();
            }
            self.finish_boundary();
        }
        ToolResult::Continue
    }

    fn on_right_click(&mut self, _pos: Vec2, _ctx: &ToolContext) -> ToolResult {
        if self.state == GroupState::Configuring {
            // Cycle distribution method
            self.distribution = match self.distribution {
                DistributionMethod::Grid => DistributionMethod::OffsetGrid,
                DistributionMethod::OffsetGrid => DistributionMethod::PoissonDisk,
                DistributionMethod::PoissonDisk => DistributionMethod::Grid,
            };
            self.update_preview();
        }
        ToolResult::Continue
    }

    fn on_text_input(&mut self, text: &str) -> bool {
        if self.state != GroupState::Configuring {
            return false;
        }
        match text {
            // +/- adjust spacing by 50mm
            "+" | "=" => {
                self.spacing_mm = (self.spacing_mm + 50.0).min(5000.0);
                self.update_preview();
                true
            }
            "-" => {
                self.spacing_mm = (self.spacing_mm - 50.0).max(50.0);
                self.update_preview();
                true
            }
            // 1/2/3 switch distribution
            "1" => { self.distribution = DistributionMethod::Grid; self.update_preview(); true }
            "2" => { self.distribution = DistributionMethod::OffsetGrid; self.update_preview(); true }
            "3" => { self.distribution = DistributionMethod::PoissonDisk; self.update_preview(); true }
            _ => false,
        }
    }

    fn on_key_enter(&mut self) -> ToolResult {
        match self.state {
            GroupState::DrawingBoundary => {
                self.finish_boundary();
                ToolResult::Continue
            }
            GroupState::Configuring => {
                self.generate_group()
            }
        }
    }

    fn on_key_escape(&mut self) -> ToolResult {
        match self.state {
            GroupState::DrawingBoundary => {
                if self.boundary.is_empty() {
                    ToolResult::Finished
                } else {
                    self.boundary.clear();
                    ToolResult::Continue
                }
            }
            GroupState::Configuring => {
                // Go back to drawing
                self.state = GroupState::DrawingBoundary;
                self.boundary.clear();
                self.preview_points.clear();
                ToolResult::Continue
            }
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, viewport: &ViewTransform) {
        let accent = egui::Color32::from_rgb(70, 130, 210);
        let boundary_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(70, 130, 210, 160));
        let boundary_fill = egui::Color32::from_rgba_unmultiplied(70, 130, 210, 20);
        let colour = category_colour(self.plant.category);
        let plant_fill = egui::Color32::from_rgba_unmultiplied(colour[0], colour[1], colour[2], 50);
        let plant_stroke_col = egui::Color32::from_rgba_unmultiplied(colour[0], colour[1], colour[2], 140);

        match self.state {
            GroupState::DrawingBoundary => {
                // Draw existing boundary vertices and edges
                if !self.boundary.is_empty() {
                    let screen_pts: Vec<egui::Pos2> = self
                        .boundary
                        .iter()
                        .map(|p| viewport.world_to_screen(*p))
                        .collect();

                    // Edges
                    for w in screen_pts.windows(2) {
                        painter.line_segment([w[0], w[1]], boundary_stroke);
                    }

                    // Line to cursor
                    let cursor = viewport.world_to_screen(self.current_pos);
                    painter.line_segment(
                        [*screen_pts.last().unwrap(), cursor],
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(70, 130, 210, 100)),
                    );

                    // Close line (from cursor back to first)
                    if self.boundary.len() >= 2 {
                        painter.line_segment(
                            [cursor, screen_pts[0]],
                            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(70, 130, 210, 50)),
                        );
                    }

                    // Vertex dots
                    for sp in &screen_pts {
                        painter.circle_filled(*sp, 4.0, accent);
                    }
                }

                // Instructions
                let instruction = if self.boundary.is_empty() {
                    "Click to start boundary"
                } else if self.boundary.len() < 3 {
                    "Click to add points (3 min)"
                } else {
                    "Double-click or Enter to finish"
                };
                let cursor = viewport.world_to_screen(self.current_pos);
                painter.text(
                    egui::pos2(cursor.x, cursor.y - 20.0),
                    egui::Align2::CENTER_BOTTOM,
                    instruction,
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgba_unmultiplied(200, 200, 210, 180),
                );
            }
            GroupState::Configuring => {
                // Draw filled boundary polygon
                if self.boundary.len() >= 3 {
                    let screen_pts: Vec<egui::Pos2> = self
                        .boundary
                        .iter()
                        .map(|p| viewport.world_to_screen(*p))
                        .collect();
                    painter.add(egui::Shape::convex_polygon(
                        screen_pts.clone(),
                        boundary_fill,
                        boundary_stroke,
                    ));
                }

                // Draw preview plant positions
                if let Some(ref mix) = self.species_mix {
                    // Multi-species: draw each point with its assigned species colour/radius
                    for (i, pt) in self.preview_points.iter().enumerate() {
                        let species_idx = self.preview_assignments.get(i).copied().unwrap_or(0);
                        let species = &mix[species_idx.min(mix.len() - 1)];
                        let r = if species.radius_mm > 0.0 { species.radius_mm } else { 200.0 };
                        let screen_r = r * viewport.zoom;
                        let c = species.colour;
                        let fill = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 50);
                        let stroke = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 140);
                        let screen = viewport.world_to_screen(*pt);
                        painter.circle_filled(screen, screen_r, fill);
                        painter.circle_stroke(screen, screen_r, egui::Stroke::new(1.0, stroke));
                    }
                } else {
                    // Single-species
                    let radius = self.plant.radius_mm();
                    let r = if radius > 0.0 { radius } else { 200.0 };
                    let screen_r = r * viewport.zoom;
                    for pt in &self.preview_points {
                        let screen = viewport.world_to_screen(*pt);
                        painter.circle_filled(screen, screen_r, plant_fill);
                        painter.circle_stroke(screen, screen_r, egui::Stroke::new(1.0, plant_stroke_col));
                    }
                }

                // Config overlay
                let center = if !self.boundary.is_empty() {
                    let sum: Vec2 = self.boundary.iter().copied().sum();
                    viewport.world_to_screen(sum / self.boundary.len() as f32)
                } else {
                    viewport.world_to_screen(self.current_pos)
                };

                let method_name = match self.distribution {
                    DistributionMethod::Grid => "Grid",
                    DistributionMethod::OffsetGrid => "Offset Grid",
                    DistributionMethod::PoissonDisk => "Poisson Disk",
                };

                let info = if let Some(ref mix) = self.species_mix {
                    // Build species breakdown string
                    let mut counts: Vec<(String, usize)> = mix.iter().map(|s| {
                        let name = if !s.plant_data.common_name.is_empty() {
                            s.plant_data.common_name.clone()
                        } else {
                            s.plant_data.botanical_name.clone()
                        };
                        (name, 0)
                    }).collect();
                    for &idx in &self.preview_assignments {
                        let idx = idx.min(counts.len() - 1);
                        counts[idx].1 += 1;
                    }
                    let breakdown: Vec<String> = counts.iter()
                        .filter(|(_, c)| *c > 0)
                        .map(|(name, c)| format!("{c} {name}"))
                        .collect();
                    format!(
                        "Mix: {} — {} — {:.0}mm spacing\n{}  |  1/2/3: method  +/-: spacing\nEnter/Click to place, Esc to cancel",
                        breakdown.join(", "),
                        method_name,
                        self.spacing_mm,
                        self.preview_points.len(),
                    )
                } else {
                    format!(
                        "{} — {} — {:.0}mm spacing\n{} plants  |  1/2/3: method  +/-: spacing\nEnter/Click to place, Esc to cancel",
                        if !self.plant.common_name.is_empty() { &self.plant.common_name } else { &self.plant.botanical_name },
                        method_name,
                        self.spacing_mm,
                        self.preview_points.len(),
                    )
                };

                // Background box for text
                let text_pos = egui::pos2(center.x, center.y - 40.0);
                let bg_rect = egui::Rect::from_center_size(
                    text_pos,
                    egui::vec2(320.0, 56.0),
                );
                painter.rect_filled(
                    bg_rect, 6.0,
                    egui::Color32::from_rgba_unmultiplied(30, 30, 36, 220),
                );
                painter.rect_stroke(
                    bg_rect, 6.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(70, 130, 210, 120)),
                    egui::StrokeKind::Outside,
                );
                painter.text(
                    text_pos,
                    egui::Align2::CENTER_CENTER,
                    info,
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgb(200, 200, 210),
                );
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
