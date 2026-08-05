use super::*;
use crate::entity::*;
use crate::garden::plant::{Category, PlantData};
use glam::Vec2;

fn make_project() -> Project {
    Project::new()
}

#[test]
fn test_empty_project() {
    let project = make_project();
    let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("viewBox"));
    // Default fallback: 0,0 to 1000,1000 + margins
    assert!(svg.contains("1100mm")); // 1000 + 2*50
}

#[test]
fn test_line_to_svg() {
    let mut project = make_project();
    let mut e = Entity::cad(
        "line1",
        Geometry::Line(LineData {
            start: Vec2::new(10.0, 20.0),
            end: Vec2::new(100.0, 200.0),
        }),
    );
    e.layer_id = project.layers.layers[0].id;
    project.add_entity(e);

    let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("<line"));
    assert!(svg.contains("x1=\"10\""));
    assert!(svg.contains("y1=\"20\""));
    assert!(svg.contains("x2=\"100\""));
    assert!(svg.contains("y2=\"200\""));
}

#[test]
fn test_circle_to_svg() {
    let mut project = make_project();
    let mut e = Entity::cad(
        "circle1",
        Geometry::Circle(CircleData {
            center: Vec2::new(50.0, 60.0),
            radius: 30.0,
        }),
    );
    e.layer_id = project.layers.layers[0].id;
    project.add_entity(e);

    let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("<circle"));
    assert!(svg.contains("cx=\"50\""));
    assert!(svg.contains("cy=\"60\""));
    assert!(svg.contains("r=\"30\""));
    assert!(svg.contains("fill=\"none\""));
}

#[test]
fn test_plant_circle_has_fill_and_label() {
    let mut project = make_project();
    let plant_data = PlantData {
        plant_id: uuid::Uuid::new_v4(),
        botanical_name: "Rosa rugosa".into(),
        common_name: "Rugosa Rose".into(),
        category: Category::Shrub,
        height_mm: 1500,
        spread_mm: 1200,
        colour_code: "#4CAF50".into(),
        flower_colour: "Pink".into(),
        foliage_colour: "Green".into(),
        evergreen: false,
        notes: String::new(),
    };
    let mut e = Entity::new(
        "plant1",
        Geometry::Circle(CircleData {
            center: Vec2::new(100.0, 100.0),
            radius: 600.0,
        }),
        DomainData::Plant(plant_data),
    );
    e.colour = [76, 175, 80, 255];
    e.layer_id = project.layers.layers[0].id;
    project.add_entity(e);

    let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("fill-opacity=\"0.24\""), "Plant circle should have fill opacity");
    assert!(svg.contains("Rugosa Rose"), "Plant should have label");
}

#[test]
fn test_rectangle_to_svg() {
    let mut project = make_project();
    let mut e = Entity::cad(
        "rect1",
        Geometry::Rectangle(RectData {
            center: Vec2::new(50.0, 50.0),
            half_size: Vec2::new(25.0, 15.0),
            rotation: 0.0,
        }),
    );
    e.layer_id = project.layers.layers[0].id;
    project.add_entity(e);

    let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("<rect"));
    assert!(svg.contains("width=\"50\""));
    assert!(svg.contains("height=\"30\""));
}

#[test]
fn test_arc_to_svg() {
    let mut project = make_project();
    let mut e = Entity::cad(
        "arc1",
        Geometry::Arc(ArcData {
            center: Vec2::new(100.0, 100.0),
            radius: 50.0,
            start_angle: 0.0,
            end_angle: std::f32::consts::FRAC_PI_2,
        }),
    );
    e.layer_id = project.layers.layers[0].id;
    project.add_entity(e);

    let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("<path"));
    assert!(svg.contains("A")); // Arc command
}

#[test]
fn test_bezier_to_svg() {
    let mut project = make_project();
    let bp = BezierPathData {
        anchors: vec![
            BezierAnchor {
                point: Vec2::new(0.0, 0.0),
                handle_in: None,
                handle_out: Some(Vec2::new(50.0, 0.0)),
            },
            BezierAnchor {
                point: Vec2::new(100.0, 100.0),
                handle_in: Some(Vec2::new(50.0, 100.0)),
                handle_out: None,
            },
        ],
        closed: false,
        holes: Vec::new(),
    };
    let mut e = Entity::cad("bezier1", Geometry::BezierPath(bp));
    e.layer_id = project.layers.layers[0].id;
    project.add_entity(e);

    let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("<path"));
    assert!(svg.contains("C")); // Cubic bezier command
}

#[test]
fn test_text_to_svg() {
    let mut project = make_project();
    let mut e = Entity::cad(
        "text1",
        Geometry::Text(TextData {
            position: Vec2::new(10.0, 20.0),
            text: "Hello Garden".into(),
            font_size: 14.0,
            rotation: 0.0,
        }),
    );
    e.layer_id = project.layers.layers[0].id;
    project.add_entity(e);

    let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("<text"));
    assert!(svg.contains("Hello Garden"));
}

#[test]
fn test_hidden_layer_excluded() {
    let mut project = make_project();
    let layer_id = project.layers.layers[0].id;

    let mut e = Entity::cad(
        "hidden_line",
        Geometry::Line(LineData {
            start: Vec2::ZERO,
            end: Vec2::new(100.0, 0.0),
        }),
    );
    e.layer_id = layer_id;
    project.add_entity(e);

    // Hide the layer
    project.layers.layers[0].visible = false;

    let svg = export_svg(&project, &SvgExportOptions { visible_only: true, ..Default::default() }).unwrap();
    assert!(!svg.contains("<line"), "Hidden layer entities should be excluded");
}

#[test]
fn test_layer_grouping() {
    let mut project = make_project();
    let layer0_id = project.layers.layers[0].id;
    let layer1_id = project.layers.layers[1].id;
    let layer0_name = project.layers.layers[0].name.clone();
    let layer1_name = project.layers.layers[1].name.clone();

    let mut e1 = Entity::cad(
        "line_a",
        Geometry::Line(LineData { start: Vec2::ZERO, end: Vec2::new(10.0, 0.0) }),
    );
    e1.layer_id = layer0_id;
    let mut e2 = Entity::cad(
        "line_b",
        Geometry::Line(LineData { start: Vec2::ZERO, end: Vec2::new(0.0, 10.0) }),
    );
    e2.layer_id = layer1_id;
    project.add_entity(e1);
    project.add_entity(e2);

    let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains(&format!("id=\"{}\"", layer0_name)));
    assert!(svg.contains(&format!("id=\"{}\"", layer1_name)));
}

#[test]
fn test_drawing_svg_paper_dimensions() {
    let project = make_project();
    let drawing = crate::drawing::drawing::Drawing::new("01", "Test");
    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("841mm"), "Should have A1 landscape width");
    assert!(svg.contains("594mm"), "Should have A1 landscape height");
}

#[test]
fn test_drawing_svg_has_title_block() {
    let mut project = make_project();
    project.metadata.name = "Test Garden".into();
    let drawing = crate::drawing::drawing::Drawing::new("01", "Master Plan");
    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("title-block"), "Should have title block group");
    assert!(svg.contains("Test Garden"), "Should auto-populate project name");
    assert!(svg.contains("Master Plan"), "Should auto-populate drawing title");
}

#[test]
fn test_drawing_svg_viewport_clipping() {
    let mut project = make_project();
    let layer_id = project.layers.layers[0].id;
    let mut e = Entity::cad(
        "line1",
        Geometry::Line(LineData { start: Vec2::ZERO, end: Vec2::new(100.0, 0.0) }),
    );
    e.layer_id = layer_id;
    project.add_entity(e);

    let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
    drawing.viewports.push(
        crate::drawing::drawing::DrawingViewport::new("Main", 100),
    );

    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("clip-path"), "Should have clip path on viewport group");
    assert!(svg.contains("<clipPath"), "Should define clip path");
    assert!(svg.contains("<line"), "Should render the entity");
}

#[test]
fn test_drawing_svg_layer_override_hides_entity() {
    let mut project = make_project();
    let layer_id = project.layers.layers[0].id;
    let mut e = Entity::cad(
        "hidden_line",
        Geometry::Line(LineData { start: Vec2::ZERO, end: Vec2::new(100.0, 0.0) }),
    );
    e.layer_id = layer_id;
    project.add_entity(e);

    let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
    drawing.show_title_block = false;
    let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
    vp.layer_overrides.visibility.insert(layer_id, false);
    drawing.viewports.push(vp);

    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    assert!(!svg.contains("x1=\"0\""), "Entity on hidden layer should be excluded from viewport");
}

#[test]
fn test_drawing_svg_numbered_plant_style() {
    use crate::drawing::drawing::PlantStyle;
    let mut project = make_project();
    let layer_id = project.layers.layers[0].id;
    for (name, category, x) in [
        ("Rosa rugosa", Category::Shrub, 0.0_f32),
        ("Acer palmatum", Category::Tree, 2000.0),
    ] {
        let plant_data = PlantData {
            plant_id: uuid::Uuid::new_v4(),
            botanical_name: name.into(),
            common_name: name.into(),
            category,
            height_mm: 1000,
            spread_mm: 800,
            colour_code: "#4CAF50".into(),
            flower_colour: String::new(),
            foliage_colour: String::new(),
            evergreen: false,
            notes: String::new(),
        };
        let mut e = Entity::new(
            name,
            Geometry::Circle(CircleData { center: Vec2::new(x, 0.0), radius: 400.0 }),
            DomainData::Plant(plant_data),
        );
        e.layer_id = layer_id;
        project.add_entity(e);
    }

    let mut drawing = crate::drawing::drawing::Drawing::new("01", "Planting");
    let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
    vp.plant_style = PlantStyle::Numbered;
    drawing.viewports.push(vp);

    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    // Schedule order: Rosa rugosa (Shrub) = 1, Acer palmatum (Tree) = 2
    assert!(svg.contains(">\n1\n</text>") || svg.contains(">1</text>"),
        "Numbered style should render schedule number 1");
    assert!(svg.contains(">\n2\n</text>") || svg.contains(">2</text>"),
        "Numbered style should render schedule number 2");
}

#[test]
fn test_drawing_svg_canopy_style_renders_symbol_path() {
    use crate::drawing::drawing::PlantStyle;
    let mut project = make_project();
    let layer_id = project.layers.layers[0].id;
    let plant_data = PlantData {
        plant_id: uuid::Uuid::new_v4(),
        botanical_name: "Acer palmatum".into(),
        common_name: "Japanese Maple".into(),
        category: Category::Tree,
        height_mm: 4000,
        spread_mm: 3000,
        colour_code: "#4CAF50".into(),
        flower_colour: String::new(),
        foliage_colour: String::new(),
        evergreen: false,
        notes: String::new(),
    };
    let mut e = Entity::new(
        "maple",
        Geometry::Circle(CircleData { center: Vec2::new(0.0, 0.0), radius: 1500.0 }),
        DomainData::Plant(plant_data),
    );
    e.layer_id = layer_id;
    project.add_entity(e);

    let mut drawing = crate::drawing::drawing::Drawing::new("01", "Master");
    let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
    vp.plant_style = PlantStyle::Canopy;
    drawing.viewports.push(vp);

    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    // Deciduous tree → scalloped outline path, closed, plus a stem marker
    let path_idx = svg.find("<path d=\"M").expect("Canopy should render an outline path");
    let path_end = svg[path_idx..].find("/>").map(|i| path_idx + i).unwrap();
    let path_elem = &svg[path_idx..path_end];
    assert!(path_elem.contains("Z"), "Canopy outline should be closed");
    assert!(path_elem.matches('L').count() > 20, "Outline should have many segments");
    assert!(svg.contains("Japanese Maple"), "Canopy keeps the plant label");
}

#[test]
fn test_drawing_svg_monochrome() {
    use crate::drawing::drawing::ColourMode;

    let mut project = make_project();
    let layer_id = project.layers.layers[0].id;
    let mut e = Entity::cad(
        "line1",
        Geometry::Line(LineData { start: Vec2::ZERO, end: Vec2::new(100.0, 0.0) }),
    );
    e.colour = [255, 0, 0, 255];
    e.layer_id = layer_id;
    project.add_entity(e);

    let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
    let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
    vp.colour_mode = ColourMode::Monochrome;
    drawing.viewports.push(vp);
    drawing.show_title_block = false;

    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    // Monochrome should use dark grey (#282828) not red (#FF0000)
    assert!(svg.contains("#282828"), "Monochrome mode should produce dark grey stroke");
    assert!(!svg.contains("#FF0000"), "Monochrome mode should not use original red colour");
}

#[test]
fn test_drawing_svg_by_species() {
    use crate::drawing::drawing::ColourMode;

    let mut project = make_project();
    let layer_id = project.layers.layers[0].id;
    let plant_data = PlantData {
        plant_id: uuid::Uuid::new_v4(),
        botanical_name: "Rosa rugosa".into(),
        common_name: "Rugosa Rose".into(),
        category: Category::Shrub,
        height_mm: 1500,
        spread_mm: 1200,
        colour_code: "#FF0000".into(),
        flower_colour: String::new(),
        foliage_colour: String::new(),
        evergreen: false,
        notes: String::new(),
    };
    let mut e = Entity::new(
        "plant1",
        Geometry::Circle(CircleData {
            center: Vec2::new(0.0, 0.0),
            radius: 600.0,
        }),
        DomainData::Plant(plant_data),
    );
    e.colour = [255, 0, 0, 255];
    e.layer_id = layer_id;
    project.add_entity(e);

    let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
    let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
    vp.colour_mode = ColourMode::BySpecies;
    drawing.viewports.push(vp);
    drawing.show_title_block = false;
    drawing.show_legend = false;

    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    // Should not use original red (legend disabled to isolate viewport rendering)
    assert!(!svg.contains("#FF0000"), "BySpecies mode should not use original colour");
    // Should contain a species-derived colour (deterministic)
    let species_c = crate::garden::colour::species_colour("Rosa rugosa");
    let species_hex = rgba_to_hex(species_c);
    assert!(svg.contains(&species_hex), "BySpecies mode should use species-derived colour");
}

#[test]
fn test_drawing_svg_annotation_rendered() {
    let project = make_project();
    let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
    drawing.show_title_block = false;
    drawing.annotations.push(
        crate::drawing::drawing::DrawingAnnotation::new("Test Label", [100.0, 50.0])
    );

    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("Test Label"), "Annotation text should appear in SVG");
    assert!(svg.contains("annotation"), "Annotation group class should appear");
}

#[test]
fn test_drawing_svg_annotation_leader() {
    let project = make_project();
    let mut drawing = crate::drawing::drawing::Drawing::new("01", "Test");
    drawing.show_title_block = false;
    let vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
    let vp_id = vp.id;
    drawing.viewports.push(vp);

    let mut ann = crate::drawing::drawing::DrawingAnnotation::new("With Leader", [50.0, 30.0]);
    ann.leader_target_model = Some([1000.0, 2000.0]);
    ann.viewport_id = Some(vp_id);
    drawing.annotations.push(ann);

    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("<line"), "Leader line should be in SVG");
    assert!(svg.contains("<polygon"), "Leader arrowhead should be in SVG");
    assert!(svg.contains("With Leader"), "Annotation text should be in SVG");
}

#[test]
fn test_drawing_svg_entities_at_garden_coords() {
    // Simulate a real garden: entities centered around (5000, 4000) mm
    // with model_center set to the design centroid
    use crate::garden::hardscape::WallData;

    let mut project = make_project();
    let layer_id = project.layers.layers[0].id;

    // Garden boundary wall: 10m x 8m rectangle offset from origin
    let mut wall = Entity::cad(
        "boundary",
        Geometry::Polyline(PolylineData {
            vertices: vec![
                Vec2::new(1000.0, 1000.0),
                Vec2::new(9000.0, 1000.0),
                Vec2::new(9000.0, 7000.0),
                Vec2::new(1000.0, 7000.0),
            ],
            closed: true,
        }),
    );
    wall.domain = DomainData::Wall(WallData {
        width_mm: 215,
        ..Default::default()
    });
    wall.layer_id = layer_id;
    project.add_entity(wall);

    // Line across the garden
    let mut path_line = Entity::cad(
        "path_line",
        Geometry::Line(LineData {
            start: Vec2::new(3000.0, 1000.0),
            end: Vec2::new(3000.0, 7000.0),
        }),
    );
    path_line.layer_id = layer_id;
    path_line.colour = [100, 100, 100, 255];
    project.add_entity(path_line);

    // Test 1: model_center at design centroid, scale 1:100
    let mut drawing = crate::drawing::drawing::Drawing::new("01", "Garden Plan");
    drawing.paper_size = crate::drawing::layout::PaperSize::A3;
    drawing.show_title_block = false;
    let mut vp = crate::drawing::drawing::DrawingViewport::new("Main", 100);
    vp.model_center = [5000.0, 4000.0]; // centroid of the garden
    drawing.viewports.push(vp);

    let svg = export_drawing_svg(&project, &drawing, &SvgExportOptions::default()).unwrap();
    assert!(svg.contains("<polygon"), "Wall should render as polygon in viewport");
    assert!(svg.contains("<line"), "Line entity should render in viewport");
    assert!(svg.contains("model_center=(5000,4000)"), "Diagnostic should show correct model_center");
    assert!(svg.contains("2 visible entities"), "Both entities should be visible");

    // Test 2: model_center at origin (default) — entities at 5000mm offset should
    // still be in the viewport at scale 1:100 (5000/100 = 50mm from center, viewport is 200mm from center)
    let mut drawing2 = crate::drawing::drawing::Drawing::new("02", "Default Center");
    drawing2.paper_size = crate::drawing::layout::PaperSize::A3;
    drawing2.show_title_block = false;
    let mut vp2 = crate::drawing::drawing::DrawingViewport::new("Main", 100);
    vp2.model_center = [0.0, 0.0]; // default
    drawing2.viewports.push(vp2);

    let svg2 = export_drawing_svg(&project, &drawing2, &SvgExportOptions::default()).unwrap();
    assert!(svg2.contains("<polygon"), "Wall should still render with default model_center at 1:100");

    // Test 3: at scale 1:30 with model_center at origin, entities at (5000,4000)
    // would be at (5000/30=167, 4000/30=133) mm from viewport center
    // Viewport is 200mm from center horizontally, 150mm vertically — should fit
    let mut drawing3 = crate::drawing::drawing::Drawing::new("03", "Scale 30");
    drawing3.paper_size = crate::drawing::layout::PaperSize::A3;
    drawing3.show_title_block = false;
    let mut vp3 = crate::drawing::drawing::DrawingViewport::new("Main", 30);
    vp3.model_center = [5000.0, 4000.0]; // centered on design
    drawing3.viewports.push(vp3);

    let svg3 = export_drawing_svg(&project, &drawing3, &SvgExportOptions::default()).unwrap();
    assert!(svg3.contains("scale=1:30"), "Diagnostic should show scale 1:30");
    assert!(svg3.contains("<polygon"), "Wall should render at 1:30 with centered viewport");
}

#[test]
fn test_wall_polyline_renders_as_polygon_with_fill() {
    use crate::garden::hardscape::WallData;

    let mut project = make_project();
    let mut e = Entity::cad(
        "wall1",
        Geometry::Polyline(PolylineData {
            vertices: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1000.0, 0.0),
                Vec2::new(1000.0, 1000.0),
            ],
            closed: false,
        }),
    );
    e.domain = DomainData::Wall(WallData {
        width_mm: 215,
        ..Default::default()
    });
    e.layer_id = project.layers.layers[0].id;
    project.add_entity(e);

    let svg = export_svg(&project, &SvgExportOptions::default()).unwrap();
    // Wall should render as a filled polygon, not a plain polyline
    assert!(svg.contains("<polygon"), "Wall should render as <polygon>");
    assert!(svg.contains("fill-opacity=\"0.12\""), "Wall polygon should have fill opacity");
    // Should NOT contain a plain <polyline> for this entity
    assert!(!svg.contains("<polyline"), "Wall should not render as <polyline>");
}

#[test]
fn test_stroke_weight_range() {
    // No height → default 0.25
    let none_weight = stroke_weight_for_domain(&DomainData::None);
    assert!((none_weight - 0.25).abs() < 0.01, "None domain should get default weight");

    // Height 0 → min 0.18
    let ground_weight = stroke_weight_for_domain(&DomainData::Edging(
        crate::garden::hardscape::EdgingData { height_mm: 0, ..Default::default() },
    ));
    assert!((ground_weight - 0.18).abs() < 0.01, "Height 0 should get min weight");

    // Height 5000 → max 0.50
    let tree_weight = stroke_weight_for_domain(&DomainData::Plant(
        crate::garden::plant::PlantData {
            plant_id: uuid::Uuid::new_v4(),
            botanical_name: "Quercus robur".into(),
            common_name: "Oak".into(),
            category: crate::garden::plant::Category::Tree,
            height_mm: 5000,
            spread_mm: 4000,
            colour_code: "#4CAF50".into(),
            flower_colour: String::new(),
            foliage_colour: String::new(),
            evergreen: false,
            notes: String::new(),
        },
    ));
    assert!((tree_weight - 0.50).abs() < 0.01, "Height 5000 should get max weight");
}

#[test]
fn test_stroke_weight_house_override() {
    let house = DomainData::House(crate::garden::hardscape::HouseData::default());
    let weight = stroke_weight_for_domain(&house);
    assert!((weight - 0.70).abs() < 0.01, "House should always get 0.70mm weight");

    // Even with a different height, house still gets fixed weight
    let house_data = crate::garden::hardscape::HouseData {
        height_mm: 100,
        ..Default::default()
    };
    let weight2 = stroke_weight_for_domain(&DomainData::House(house_data));
    assert!((weight2 - 0.70).abs() < 0.01, "House weight should be fixed regardless of height");
}
