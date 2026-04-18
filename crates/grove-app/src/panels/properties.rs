use egui::Ui;
use uuid::Uuid;

use grove_core::entity::DomainData;
use grove_core::garden::colour::category_colour;
use grove_core::project::Project;

use crate::panels::widgets::*;
use crate::theme::ThemeColours;
use crate::tools::select::Selection;

/// Draw the properties panel for the current selection.
/// Returns `Some((entity_id, new_domain))` if the user edited a property.
pub fn draw_properties_panel(
    ui: &mut Ui,
    project: &Project,
    selection: &Selection,
) -> Option<(Uuid, DomainData)> {
    ui.heading("Properties");
    ui.separator();

    let id = selection.single()?;
    let entity = project.find_entity(id)?;

    // Common header: name and geometry type
    let geom_type = match &entity.geometry {
        grove_core::entity::Geometry::None => "None",
        grove_core::entity::Geometry::Point(_) => "Point",
        grove_core::entity::Geometry::Line(_) => "Line",
        grove_core::entity::Geometry::Polyline(_) => "Polyline",
        grove_core::entity::Geometry::Polygon(_) => "Polygon",
        grove_core::entity::Geometry::Circle(_) => "Circle",
        grove_core::entity::Geometry::Arc(_) => "Arc",
        grove_core::entity::Geometry::Rectangle(_) => "Rectangle",
        grove_core::entity::Geometry::Text(_) => "Text",
        grove_core::entity::Geometry::BezierPath(_) => "Curve",
        grove_core::entity::Geometry::Dimension(_) => "Dimension",
    };

    ui.label(format!("Name: {}", entity.name));
    ui.colored_label(ThemeColours::TEXT_DIM, format!("Geometry: {geom_type}"));

    // Layer name
    if let Some(layer) = project.layers.layers.iter().find(|l| l.id == entity.layer_id) {
        ui.colored_label(ThemeColours::TEXT_DIM, format!("Layer: {}", layer.name));
    }

    // Bounding box info
    if let Some(bb) = entity.bounding_box() {
        ui.colored_label(
            ThemeColours::TEXT_DIM,
            format!("Position: ({:.0}, {:.0})", bb.center().x, bb.center().y),
        );
        ui.colored_label(
            ThemeColours::TEXT_DIM,
            format!("Size: {:.0} x {:.0}", bb.size().x, bb.size().y),
        );
    }

    ui.add_space(4.0);
    ui.separator();

    // Clone domain for editing
    let mut domain = entity.domain.clone();
    let original = entity.domain.clone();

    match &mut domain {
        DomainData::Plant(ref pd) => {
            // Plant properties are read-only display
            ui.label(format!("Botanical: {}", pd.botanical_name));
            if !pd.common_name.is_empty() {
                ui.label(format!("Common: {}", pd.common_name));
            }
            let colour = category_colour(pd.category);
            let col = egui::Color32::from_rgb(colour[0], colour[1], colour[2]);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, col);
                ui.label(pd.category.as_str());
            });
            ui.add_space(4.0);
            if pd.height_mm > 0 {
                ui.label(format!("Height: {}mm", pd.height_mm));
            }
            if pd.spread_mm > 0 {
                ui.label(format!("Spread: {}mm", pd.spread_mm));
            }
            if !pd.flower_colour.is_empty() {
                ui.label(format!("Flower: {}", pd.flower_colour));
            }
            if !pd.foliage_colour.is_empty() {
                ui.label(format!("Foliage: {}", pd.foliage_colour));
            }
            if pd.evergreen {
                ui.label("Evergreen: Yes");
            }
            if !pd.notes.is_empty() {
                ui.add_space(4.0);
                ui.colored_label(ThemeColours::TEXT_SECONDARY, &pd.notes);
            }
            // Plants are read-only in properties, return None
            return None;
        }
        DomainData::Wall(w) => {
            combo_wall_type(ui, &mut w.wall_type);
            drag_mm(ui, "Height", &mut w.height_mm);
            drag_mm(ui, "Width", &mut w.width_mm);
            combo_material(ui, &mut w.base.material);
            ui.horizontal(|ui| {
                ui.label("Capping:");
                ui.text_edit_singleline(&mut w.capping);
            });
            drag_mm(ui, "Foundation", &mut w.foundation_depth_mm);
            ui.horizontal(|ui| {
                ui.label("Jointing:");
                ui.text_edit_singleline(&mut w.jointing);
            });
        }
        DomainData::Path(p) => {
            drag_mm(ui, "Width", &mut p.width_mm);
            ui.horizontal(|ui| {
                ui.label("Pattern:");
                ui.text_edit_singleline(&mut p.laying_pattern);
            });
            combo_drainage(ui, &mut p.drainage);
            combo_material(ui, &mut p.base.material);
            drag_mm(ui, "Sub-base", &mut p.sub_base_depth_mm);
            drag_mm(ui, "Joint W", &mut p.joint_width_mm);
            ui.horizontal(|ui| {
                ui.label("Joint mat:");
                ui.text_edit_singleline(&mut p.joint_material);
            });
        }
        DomainData::Patio(p) => {
            ui.horizontal(|ui| {
                ui.label("Pattern:");
                ui.text_edit_singleline(&mut p.laying_pattern);
            });
            combo_patio_drainage(ui, &mut p.drainage);
            drag_mm(ui, "Fall (mm/m)", &mut p.fall_rate_mm_per_m);
            combo_material(ui, &mut p.base.material);
        }
        DomainData::Step(s) => {
            drag_mm(ui, "Riser H", &mut s.riser_height_mm);
            drag_mm(ui, "Tread D", &mut s.tread_depth_mm);
            drag_mm(ui, "Width", &mut s.width_mm);
            drag_mm(ui, "Nosing", &mut s.nosing_mm);
            ui.add(egui::DragValue::new(&mut s.num_risers).prefix("Risers: ").range(1..=20));
            ui.checkbox(&mut s.handrail, "Handrail");
            if s.handrail {
                combo_handrail_side(ui, &mut s.handrail_side);
            }
            combo_material(ui, &mut s.base.material);
        }
        DomainData::Edging(e) => {
            combo_edging_type(ui, &mut e.edging_type);
            drag_mm(ui, "Height", &mut e.height_mm);
            drag_mm(ui, "Width", &mut e.width_mm);
            drag_mm(ui, "Below", &mut e.depth_below_mm);
            combo_material(ui, &mut e.base.material);
        }
        DomainData::RaisedBed(r) => {
            drag_mm(ui, "Height", &mut r.height_mm);
            drag_mm(ui, "Wall W", &mut r.wall_width_mm);
            drag_mm(ui, "Soil D", &mut r.soil_depth_mm);
            ui.checkbox(&mut r.drainage, "Drainage");
            ui.checkbox(&mut r.liner, "Liner");
            combo_material(ui, &mut r.base.material);
        }
        DomainData::Fence(f) => {
            combo_fence_type(ui, &mut f.fence_type);
            drag_mm(ui, "Height", &mut f.height_mm);
            drag_mm(ui, "Post Spacing", &mut f.post_spacing_mm);
            combo_post_setting(ui, &mut f.post_setting);
            ui.checkbox(&mut f.gravel_board, "Gravel board");
            ui.checkbox(&mut f.gate, "Gate");
        }
        DomainData::Pergola(p) => {
            combo_structure_type(ui, &mut p.structure_type);
            drag_mm(ui, "Height", &mut p.height_mm);
            combo_roof_type(ui, &mut p.roof);
            combo_post_fixing(ui, &mut p.post_fixing);
            combo_material(ui, &mut p.base.material);
        }
        DomainData::Decking(d) => {
            drag_mm(ui, "Board W", &mut d.board_width_mm);
            drag_mm(ui, "Gap", &mut d.board_gap_mm);
            drag_mm(ui, "Deck H", &mut d.deck_height_mm);
            combo_substructure(ui, &mut d.substructure);
            ui.checkbox(&mut d.balustrade, "Balustrade");
            combo_material(ui, &mut d.base.material);
        }
        DomainData::WaterFeature(wf) => {
            combo_water_feature_type(ui, &mut wf.feature_type);
            drag_mm(ui, "Depth", &mut wf.depth_mm);
            ui.checkbox(&mut wf.pump, "Pump");
            ui.checkbox(&mut wf.lighting, "Lighting");
            combo_filtration(ui, &mut wf.filtration);
        }
        DomainData::SpotHeight(sh) => {
            ui.horizontal(|ui| {
                ui.label("Elevation:");
                ui.add(egui::DragValue::new(&mut sh.elevation_mm).suffix(" mm").speed(10.0));
            });
            combo_height_source(ui, &mut sh.source);
            ui.checkbox(&mut sh.label_display, "Show label");
            ui.horizontal(|ui| {
                ui.label("Datum:");
                ui.text_edit_singleline(&mut sh.datum_reference);
            });
        }
        DomainData::Contour(c) => {
            ui.horizontal(|ui| {
                ui.label("Elevation:");
                ui.add(egui::DragValue::new(&mut c.elevation_mm).suffix(" mm").speed(10.0));
            });
            combo_contour_type(ui, &mut c.contour_type);
            combo_contour_source(ui, &mut c.source);
            ui.horizontal(|ui| {
                ui.label("Interval:");
                ui.add(egui::DragValue::new(&mut c.interval_mm).suffix(" mm").range(50..=5000));
            });
        }
        DomainData::SlopeArrow(sa) => {
            ui.horizontal(|ui| {
                ui.label("Start level:");
                ui.add(egui::DragValue::new(&mut sa.start_level_mm).suffix(" mm").speed(10.0));
            });
            ui.horizontal(|ui| {
                ui.label("End level:");
                ui.add(egui::DragValue::new(&mut sa.end_level_mm).suffix(" mm").speed(10.0));
            });
        }
        DomainData::DrainRun(dr) => {
            combo_drain_type(ui, &mut dr.drain_type);
            ui.horizontal(|ui| {
                ui.label("Pipe dia:");
                ui.add(egui::DragValue::new(&mut dr.pipe_dia_mm).suffix(" mm").range(50..=600));
            });
            ui.horizontal(|ui| {
                ui.label("Invert start:");
                ui.add(egui::DragValue::new(&mut dr.invert_start_mm).suffix(" mm").speed(10.0));
            });
            ui.horizontal(|ui| {
                ui.label("Invert end:");
                ui.add(egui::DragValue::new(&mut dr.invert_end_mm).suffix(" mm").speed(10.0));
            });
            ui.checkbox(&mut dr.manhole, "Manhole");
        }
        DomainData::Surface(s) => {
            ui.label(format!("Surface: {}", s.label.label()));
            if let Some(ref mut w) = s.width_mm {
                drag_mm(ui, "Width", w);
            }
            ui.horizontal(|ui| {
                ui.label("Pattern:");
                ui.text_edit_singleline(&mut s.laying_pattern);
            });
            combo_drainage(ui, &mut s.drainage);
            combo_material(ui, &mut s.base.material);
            drag_mm(ui, "Sub-base", &mut s.sub_base_depth_mm);
            if matches!(s.label, grove_core::garden::hardscape::SurfaceLabel::Decking) {
                drag_mm(ui, "Board W", &mut s.board_width_mm);
                drag_mm(ui, "Board Gap", &mut s.board_gap_mm);
            }
        }
        DomainData::House(h) => {
            drag_mm(ui, "Wall Thickness", &mut h.wall_thickness_mm);
            drag_mm(ui, "Height", &mut h.height_mm);
            combo_material(ui, &mut h.base.material);
        }
        DomainData::None | DomainData::PlantGroup(_) | DomainData::TitleBlock(_) | DomainData::Legend(_) | DomainData::Entourage(_) => {
            // No editable properties
            return None;
        }
    }

    // Compare serialized forms to detect changes
    let changed = !domain_eq(&original, &domain);
    if changed {
        Some((id, domain))
    } else {
        None
    }
}

/// Quick structural comparison via Debug representation.
/// This avoids requiring PartialEq on all domain types.
fn domain_eq(a: &DomainData, b: &DomainData) -> bool {
    format!("{:?}", a) == format!("{:?}", b)
}

/// Draw properties panel content when multiple entities are selected.
pub fn draw_multi_selection(ui: &mut Ui, count: usize) {
    ui.heading("Properties");
    ui.separator();
    ui.colored_label(
        ThemeColours::TEXT_SECONDARY,
        format!("{} entities selected", count),
    );
}

/// Draw properties panel content when nothing is selected.
pub fn draw_no_selection(ui: &mut Ui) {
    ui.heading("Properties");
    ui.separator();
    ui.colored_label(ThemeColours::TEXT_SECONDARY, "No selection");
}
