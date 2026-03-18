use egui::Ui;

use grove_core::garden::hardscape::*;
use grove_core::garden::levels::*;
use grove_core::garden::material::Material;

// ---------------------------------------------------------------------------
// Shared drag/combo helpers for palettes and properties panel
// ---------------------------------------------------------------------------

pub(crate) fn drag_mm(ui: &mut Ui, label: &str, value: &mut i32) {
    ui.horizontal(|ui| {
        ui.label(format!("{label}:"));
        ui.add(egui::DragValue::new(value).suffix(" mm").range(0..=10000));
    });
}

pub(crate) fn combo_material(ui: &mut Ui, mat: &mut Material) {
    egui::ComboBox::from_label("Material")
        .selected_text(format!("{:?}", mat))
        .show_ui(ui, |ui| {
            for m in &[
                Material::Stone, Material::Brick, Material::Timber, Material::Concrete,
                Material::Metal, Material::Composite, Material::Gravel, Material::ResinBound,
                Material::Porcelain, Material::Render, Material::Corten, Material::Glass,
            ] {
                ui.selectable_value(mat, *m, format!("{:?}", m));
            }
        });
}

pub(crate) fn combo_wall_type(ui: &mut Ui, wt: &mut WallType) {
    egui::ComboBox::from_label("Wall Type")
        .selected_text(format!("{:?}", wt))
        .show_ui(ui, |ui| {
            for v in &[WallType::Freestanding, WallType::Retaining, WallType::Seat] {
                ui.selectable_value(wt, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_drainage(ui: &mut Ui, d: &mut DrainageType) {
    egui::ComboBox::from_label("Drainage")
        .selected_text(format!("{:?}", d))
        .show_ui(ui, |ui| {
            for v in &[DrainageType::None, DrainageType::CrossFall, DrainageType::Permeable, DrainageType::Channel] {
                ui.selectable_value(d, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_patio_drainage(ui: &mut Ui, d: &mut PatioDrainage) {
    egui::ComboBox::from_label("Drainage")
        .selected_text(format!("{:?}", d))
        .show_ui(ui, |ui| {
            for v in &[PatioDrainage::None, PatioDrainage::CrossFall, PatioDrainage::Permeable, PatioDrainage::AcoChannel] {
                ui.selectable_value(d, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_edging_type(ui: &mut Ui, et: &mut EdgingType) {
    egui::ComboBox::from_label("Edging Type")
        .selected_text(format!("{:?}", et))
        .show_ui(ui, |ui| {
            for v in &[EdgingType::Flush, EdgingType::Raised, EdgingType::MowingStrip] {
                ui.selectable_value(et, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_fence_type(ui: &mut Ui, ft: &mut FenceType) {
    egui::ComboBox::from_label("Fence Type")
        .selected_text(format!("{:?}", ft))
        .show_ui(ui, |ui| {
            for v in &[
                FenceType::CloseBoard, FenceType::HitAndMiss, FenceType::Trellis,
                FenceType::PostAndRail, FenceType::Picket, FenceType::Woven,
                FenceType::MetalRailing, FenceType::Slatted, FenceType::Acoustic,
            ] {
                ui.selectable_value(ft, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_post_setting(ui: &mut Ui, ps: &mut PostSetting) {
    egui::ComboBox::from_label("Post Setting")
        .selected_text(format!("{:?}", ps))
        .show_ui(ui, |ui| {
            for v in &[PostSetting::Concrete, PostSetting::PostSpike, PostSetting::BoltDown] {
                ui.selectable_value(ps, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_structure_type(ui: &mut Ui, st: &mut StructureType) {
    egui::ComboBox::from_label("Structure")
        .selected_text(format!("{:?}", st))
        .show_ui(ui, |ui| {
            for v in &[
                StructureType::Pergola, StructureType::Arbour, StructureType::Gazebo,
                StructureType::ShadeSail, StructureType::Carport,
            ] {
                ui.selectable_value(st, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_roof_type(ui: &mut Ui, rt: &mut RoofType) {
    egui::ComboBox::from_label("Roof")
        .selected_text(format!("{:?}", rt))
        .show_ui(ui, |ui| {
            for v in &[
                RoofType::Open, RoofType::Polycarbonate, RoofType::Glass,
                RoofType::Fabric, RoofType::GreenRoof,
            ] {
                ui.selectable_value(rt, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_post_fixing(ui: &mut Ui, pf: &mut PostFixing) {
    egui::ComboBox::from_label("Post Fixing")
        .selected_text(format!("{:?}", pf))
        .show_ui(ui, |ui| {
            for v in &[PostFixing::BoltDown, PostFixing::SetInConcrete, PostFixing::PostAnchor] {
                ui.selectable_value(pf, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_substructure(ui: &mut Ui, ss: &mut Substructure) {
    egui::ComboBox::from_label("Substructure")
        .selected_text(format!("{:?}", ss))
        .show_ui(ui, |ui| {
            for v in &[Substructure::Timber, Substructure::Aluminium, Substructure::AdjustablePedestal] {
                ui.selectable_value(ss, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_water_feature_type(ui: &mut Ui, ft: &mut WaterFeatureType) {
    egui::ComboBox::from_label("Feature")
        .selected_text(format!("{:?}", ft))
        .show_ui(ui, |ui| {
            for v in &[
                WaterFeatureType::Pond, WaterFeatureType::Rill, WaterFeatureType::Fountain,
                WaterFeatureType::WallSpout, WaterFeatureType::Bowl, WaterFeatureType::Trough,
                WaterFeatureType::Stream,
            ] {
                ui.selectable_value(ft, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_filtration(ui: &mut Ui, f: &mut Filtration) {
    egui::ComboBox::from_label("Filtration")
        .selected_text(format!("{:?}", f))
        .show_ui(ui, |ui| {
            for v in &[
                Filtration::None, Filtration::Mechanical, Filtration::Biological,
                Filtration::Uv, Filtration::Combined,
            ] {
                ui.selectable_value(f, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_height_source(ui: &mut Ui, src: &mut HeightSource) {
    egui::ComboBox::from_label("Source")
        .selected_text(format!("{:?}", src))
        .show_ui(ui, |ui| {
            for v in &[HeightSource::Survey, HeightSource::Design, HeightSource::Interpolated] {
                ui.selectable_value(src, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_contour_type(ui: &mut Ui, ct: &mut ContourType) {
    egui::ComboBox::from_label("Contour Type")
        .selected_text(format!("{:?}", ct))
        .show_ui(ui, |ui| {
            for v in &[ContourType::Major, ContourType::Minor, ContourType::Index] {
                ui.selectable_value(ct, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_contour_source(ui: &mut Ui, src: &mut ContourSource) {
    egui::ComboBox::from_label("Source")
        .selected_text(format!("{:?}", src))
        .show_ui(ui, |ui| {
            for v in &[ContourSource::Survey, ContourSource::Generated, ContourSource::Design] {
                ui.selectable_value(src, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_drain_type(ui: &mut Ui, dt: &mut DrainType) {
    egui::ComboBox::from_label("Drain Type")
        .selected_text(format!("{:?}", dt))
        .show_ui(ui, |ui| {
            for v in &[
                DrainType::Foul, DrainType::Surface, DrainType::Combined,
                DrainType::LandDrain, DrainType::FrenchDrain, DrainType::Soakaway,
            ] {
                ui.selectable_value(dt, *v, format!("{:?}", v));
            }
        });
}

pub(crate) fn combo_surface_label(ui: &mut Ui, label: &mut SurfaceLabel) {
    let current_text = label.label().to_string();
    egui::ComboBox::from_label("Surface")
        .selected_text(&current_text)
        .show_ui(ui, |ui| {
            for preset in SurfaceLabel::PRESET {
                let text = preset.label();
                if ui.selectable_label(label == preset, text).clicked() {
                    *label = preset.clone();
                }
            }
        });
}

pub(crate) fn combo_handrail_side(ui: &mut Ui, hs: &mut HandrailSide) {
    egui::ComboBox::from_label("Handrail Side")
        .selected_text(format!("{:?}", hs))
        .show_ui(ui, |ui| {
            for v in &[HandrailSide::None, HandrailSide::Left, HandrailSide::Right, HandrailSide::Both] {
                ui.selectable_value(hs, *v, format!("{:?}", v));
            }
        });
}
