use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// EntourageCategory — palette grouping (derived from folder name)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntourageCategory {
    People,
    Vehicles,
    Furniture,
    Garden,
}

impl EntourageCategory {
    pub const ALL: &'static [EntourageCategory] = &[
        EntourageCategory::People,
        EntourageCategory::Vehicles,
        EntourageCategory::Furniture,
        EntourageCategory::Garden,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::People => "People",
            Self::Vehicles => "Vehicles",
            Self::Furniture => "Furniture",
            Self::Garden => "Garden",
        }
    }

    pub fn from_folder_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "people" => Some(Self::People),
            "vehicles" => Some(Self::Vehicles),
            "furniture" => Some(Self::Furniture),
            "garden" => Some(Self::Garden),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// EntourageData — domain data stored on each entity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntourageData {
    /// Display name of the symbol.
    pub name: String,
    /// Original source file name (for reference).
    pub source_file: String,
    /// Width in mm (real-world size on the plan).
    pub width_mm: f32,
    /// Height in mm (real-world size on the plan).
    pub height_mm: f32,
    /// Rasterized image data (PNG bytes) — embedded for portability.
    #[serde(with = "base64_bytes")]
    pub image_data: Vec<u8>,
    /// Pixel dimensions of the rasterized image.
    pub image_width: u32,
    pub image_height: u32,
}

// ---------------------------------------------------------------------------
// EntourageLibraryItem — an available symbol in the library
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EntourageLibraryItem {
    pub name: String,
    pub category: EntourageCategory,
    pub source_file: String,
    /// Width in mm based on SVG viewBox.
    pub width_mm: f32,
    /// Height in mm based on SVG viewBox.
    pub height_mm: f32,
    /// Raw file bytes (SVG or PNG source).
    pub file_data: Vec<u8>,
    /// Rasterized RGBA pixels for display.
    pub raster_rgba: Vec<u8>,
    pub raster_width: u32,
    pub raster_height: u32,
    /// PNG-encoded bytes for embedding in entities.
    pub png_data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// EntourageLibrary — scans data/entourage/ and caches items
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct EntourageLibrary {
    pub items: Vec<EntourageLibraryItem>,
}

impl EntourageLibrary {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Scan a directory for entourage SVG/PNG files.
    /// Expected structure: `base_dir/{category}/{file}.svg|png`
    pub fn load_from_directory(base_dir: &std::path::Path) -> Self {
        let mut items = Vec::new();

        for cat in EntourageCategory::ALL {
            let cat_dir = base_dir.join(cat.label().to_lowercase());
            if !cat_dir.is_dir() {
                continue;
            }

            let Ok(entries) = std::fs::read_dir(&cat_dir) else {
                continue;
            };

            let mut cat_items: Vec<EntourageLibraryItem> = Vec::new();

            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if ext != "svg" && ext != "png" {
                    continue;
                }

                let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let name = path.file_stem().unwrap_or_default().to_string_lossy()
                    .replace('_', " ")
                    .replace('-', " ");
                // Title-case the name
                let name = name.split_whitespace()
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().to_string() + &c.as_str().to_lowercase(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                let Ok(file_data) = std::fs::read(&path) else {
                    log::warn!("Failed to read entourage file: {}", path.display());
                    continue;
                };

                match ext.as_str() {
                    "svg" => {
                        if let Some(item) = load_svg_item(&name, *cat, &file_name, &file_data) {
                            cat_items.push(item);
                        }
                    }
                    "png" => {
                        if let Some(item) = load_png_item(&name, *cat, &file_name, &file_data) {
                            cat_items.push(item);
                        }
                    }
                    _ => {}
                }
            }

            cat_items.sort_by(|a, b| a.name.cmp(&b.name));
            items.extend(cat_items);
        }

        log::info!("Loaded {} entourage items from {}", items.len(), base_dir.display());
        Self { items }
    }

    pub fn items_for_category(&self, cat: EntourageCategory) -> Vec<&EntourageLibraryItem> {
        self.items.iter().filter(|i| i.category == cat).collect()
    }
}

impl Default for EntourageLibrary {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SVG loading: parse viewBox for real-world dimensions, rasterize with resvg
// ---------------------------------------------------------------------------

fn load_svg_item(
    name: &str,
    category: EntourageCategory,
    file_name: &str,
    svg_data: &[u8],
) -> Option<EntourageLibraryItem> {
    let svg_str = std::str::from_utf8(svg_data).ok()?;

    // Parse with usvg (already a dependency)
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg_str, &opts).ok()?;

    let size = tree.size();
    let svg_w = size.width() as f32;
    let svg_h = size.height() as f32;

    // Determine real-world size from viewBox
    // SVG viewBox units map to mm in our system
    // The viewBox represents the full extent in mm
    let (width_mm, height_mm) = extract_viewbox_mm(svg_str, svg_w, svg_h);

    // Rasterize at a reasonable resolution for display
    let render_scale = 256.0 / svg_w.max(svg_h);
    let rw = (svg_w * render_scale).ceil() as u32;
    let rh = (svg_h * render_scale).ceil() as u32;

    let mut pixmap = tiny_skia::Pixmap::new(rw, rh)?;
    let transform = tiny_skia::Transform::from_scale(
        rw as f32 / svg_w,
        rh as f32 / svg_h,
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let rgba_data = pixmap.data().to_vec();

    // Encode as PNG for entity embedding
    let png_data = pixmap.encode_png().ok()?;

    Some(EntourageLibraryItem {
        name: name.to_string(),
        category,
        source_file: file_name.to_string(),
        width_mm,
        height_mm,
        file_data: svg_data.to_vec(),
        raster_rgba: rgba_data,
        raster_width: rw,
        raster_height: rh,
        png_data,
    })
}

/// Extract real-world dimensions in mm from SVG viewBox.
/// The viewBox attribute contains the coordinate space; we treat those as mm.
fn extract_viewbox_mm(svg_str: &str, fallback_w: f32, fallback_h: f32) -> (f32, f32) {
    // Look for viewBox="minX minY width height"
    if let Some(vb_start) = svg_str.find("viewBox=\"") {
        let vb_str = &svg_str[vb_start + 9..];
        if let Some(end) = vb_str.find('"') {
            let vb = &vb_str[..end];
            let parts: Vec<f32> = vb.split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if parts.len() == 4 {
                return (parts[2].abs(), parts[3].abs());
            }
        }
    }
    (fallback_w, fallback_h)
}

// ---------------------------------------------------------------------------
// PNG loading: decode dimensions, use directly
// ---------------------------------------------------------------------------

fn load_png_item(
    name: &str,
    category: EntourageCategory,
    file_name: &str,
    png_data: &[u8],
) -> Option<EntourageLibraryItem> {
    let img = image::load_from_memory(png_data).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());

    // For PNGs without explicit size info, assume 10 pixels per mm
    let pixels_per_mm = 10.0;
    let width_mm = w as f32 / pixels_per_mm;
    let height_mm = h as f32 / pixels_per_mm;

    // Create thumbnail for palette display
    let thumb_size = 256u32;
    let scale = thumb_size as f32 / w.max(h) as f32;
    let tw = (w as f32 * scale).ceil() as u32;
    let th = (h as f32 * scale).ceil() as u32;
    let thumb = image::imageops::resize(&rgba, tw, th, image::imageops::FilterType::Lanczos3);

    Some(EntourageLibraryItem {
        name: name.to_string(),
        category,
        source_file: file_name.to_string(),
        width_mm,
        height_mm,
        file_data: png_data.to_vec(),
        raster_rgba: thumb.into_raw(),
        raster_width: tw,
        raster_height: th,
        png_data: png_data.to_vec(),
    })
}

// ---------------------------------------------------------------------------
// Base64 serde helper for image_data in EntourageData
// ---------------------------------------------------------------------------

mod base64_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(data: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        encoded.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        use base64::Engine;
        let s = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_from_folder() {
        assert_eq!(EntourageCategory::from_folder_name("people"), Some(EntourageCategory::People));
        assert_eq!(EntourageCategory::from_folder_name("FURNITURE"), Some(EntourageCategory::Furniture));
        assert_eq!(EntourageCategory::from_folder_name("unknown"), None);
    }

    #[test]
    fn extract_viewbox() {
        let svg = r#"<svg viewBox="-100 -200 600 400" width="100" height="80"></svg>"#;
        let (w, h) = extract_viewbox_mm(svg, 100.0, 80.0);
        assert_eq!(w, 600.0);
        assert_eq!(h, 400.0);
    }

    #[test]
    fn extract_viewbox_fallback() {
        let svg = r#"<svg width="100" height="80"></svg>"#;
        let (w, h) = extract_viewbox_mm(svg, 100.0, 80.0);
        assert_eq!(w, 100.0);
        assert_eq!(h, 80.0);
    }

    #[test]
    fn load_svg_library_from_data_dir() {
        // Try multiple relative paths (test runs from different working dirs)
        let paths = ["data/entourage", "../data/entourage", "../../data/entourage"];
        let mut lib = EntourageLibrary::new();
        for path in &paths {
            let p = std::path::Path::new(path);
            if p.is_dir() {
                lib = EntourageLibrary::load_from_directory(p);
                break;
            }
        }
        // Should find our 12 SVG files across 4 categories
        assert!(lib.items.len() >= 12, "Expected at least 12 items, got {}", lib.items.len());
        assert!(!lib.items_for_category(EntourageCategory::People).is_empty());
        assert!(!lib.items_for_category(EntourageCategory::Vehicles).is_empty());
        assert!(!lib.items_for_category(EntourageCategory::Furniture).is_empty());
        assert!(!lib.items_for_category(EntourageCategory::Garden).is_empty());

        // Each item should have valid rasterized data
        for item in &lib.items {
            assert!(item.raster_width > 0);
            assert!(item.raster_height > 0);
            assert!(!item.png_data.is_empty(), "{} has empty PNG data", item.name);
            assert!(item.width_mm > 0.0, "{} has zero width", item.name);
            assert!(item.height_mm > 0.0, "{} has zero height", item.name);
        }
    }

    #[test]
    fn entourage_data_roundtrip() {
        let data = EntourageData {
            name: "Test".to_string(),
            source_file: "test.svg".to_string(),
            width_mm: 1000.0,
            height_mm: 800.0,
            image_data: vec![1, 2, 3, 4],
            image_width: 256,
            image_height: 200,
        };
        let json = serde_json::to_string(&data).unwrap();
        let loaded: EntourageData = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.name, "Test");
        assert_eq!(loaded.image_data, vec![1, 2, 3, 4]);
    }
}
