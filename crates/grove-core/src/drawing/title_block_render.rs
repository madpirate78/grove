use svg::node::element::{self, Group};

use super::layout::{Orientation, PaperSize};
use super::title_block::TitleBlockData;

/// Render a title block as an SVG group, positioned at the bottom-right of the sheet.
pub fn render_title_block_svg(
    data: &TitleBlockData,
    paper_size: PaperSize,
    orientation: Orientation,
) -> Group {
    let (pw, ph) = paper_size.dimensions_mm();
    let (paper_w, paper_h) = match orientation {
        Orientation::Landscape => (pw, ph),
        Orientation::Portrait => (ph, pw),
    };

    // Title block width scales with paper size (180mm on A1, proportionally smaller)
    let a1_width = 841.0_f32;
    let tb_width = (180.0 * paper_w / a1_width).max(80.0);
    let tb_height = 60.0_f32;
    let tb_x = paper_w - tb_width - 10.0;
    let tb_y = paper_h - tb_height - 10.0;

    let font_scale = paper_w / a1_width;
    let title_size = (14.0 * font_scale).max(6.0);
    let label_size = (8.0 * font_scale).max(4.0);
    let value_size = (10.0 * font_scale).max(5.0);

    let mut group = Group::new().set("id", "title-block");

    // Background rectangle
    let bg = element::Rectangle::new()
        .set("x", tb_x)
        .set("y", tb_y)
        .set("width", tb_width)
        .set("height", tb_height)
        .set("fill", "white")
        .set("stroke", "black")
        .set("stroke-width", 0.5);
    group = group.add(bg);

    // Border
    let border = element::Rectangle::new()
        .set("x", tb_x)
        .set("y", tb_y)
        .set("width", tb_width)
        .set("height", tb_height)
        .set("fill", "none")
        .set("stroke", "black")
        .set("stroke-width", 1.0);
    group = group.add(border);

    // Project name (top row)
    let project_text = element::Text::new(data.project_name.clone())
        .set("x", tb_x + 4.0)
        .set("y", tb_y + title_size + 2.0)
        .set("font-size", title_size)
        .set("font-weight", "bold")
        .set("fill", "black");
    group = group.add(project_text);

    // Drawing title (second row)
    let title_text = element::Text::new(data.drawing_title.clone())
        .set("x", tb_x + 4.0)
        .set("y", tb_y + title_size + value_size + 6.0)
        .set("font-size", value_size)
        .set("fill", "black");
    group = group.add(title_text);

    // Bottom row: drawn by, date, scale, sheet, revision
    let row_y = tb_y + tb_height - 4.0;
    let col_w = tb_width / 5.0;

    let fields = [
        ("Drawn", &data.drawn_by),
        ("Date", &data.date),
        ("Scale", &data.scale),
        ("Sheet", &data.sheet_number),
        ("Rev", &data.revision),
    ];

    for (i, (label, value)) in fields.iter().enumerate() {
        let x = tb_x + i as f32 * col_w + 2.0;

        let label_el = element::Text::new(label.to_string())
            .set("x", x)
            .set("y", row_y - value_size - 1.0)
            .set("font-size", label_size)
            .set("fill", "#666");
        group = group.add(label_el);

        let value_el = element::Text::new(value.to_string())
            .set("x", x)
            .set("y", row_y)
            .set("font-size", value_size)
            .set("fill", "black");
        group = group.add(value_el);
    }

    // Divider line above bottom row
    let div_y = row_y - value_size - label_size - 2.0;
    let div = element::Line::new()
        .set("x1", tb_x)
        .set("y1", div_y)
        .set("x2", tb_x + tb_width)
        .set("y2", div_y)
        .set("stroke", "black")
        .set("stroke-width", 0.5);
    group = group.add(div);

    group
}
