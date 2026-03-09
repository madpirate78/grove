use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How the background image colours are processed for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColourMode {
    Normal,
    Inverted,
    Desaturated,
    Tinted([u8; 3]),
}

/// A background reference image that sits behind all entities on the canvas.
/// Not selectable, not part of the entity system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundImage {
    pub id: Uuid,
    pub name: String,
    #[serde(skip)]
    pub image_data: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
    pub position: [f32; 2],
    pub pixels_per_mm: f32,
    pub rotation_rad: f32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub opacity: f32,
    pub visible: bool,
    pub colour_mode: ColourMode,
}

impl BackgroundImage {
    /// Create a new background image from raw file bytes.
    /// Decodes just enough to get dimensions.
    pub fn from_bytes(name: String, data: Vec<u8>) -> Result<Self, String> {
        let (width_px, height_px) = image_dimensions(&data)?;
        Ok(Self {
            id: Uuid::new_v4(),
            name,
            image_data: data,
            width_px,
            height_px,
            position: [0.0, 0.0],
            pixels_per_mm: 10.0,
            rotation_rad: 0.0,
            flip_h: false,
            flip_v: false,
            opacity: 0.3,
            visible: true,
            colour_mode: ColourMode::Normal,
        })
    }

    /// Size in world-space millimetres.
    pub fn size_mm(&self) -> [f32; 2] {
        [
            self.width_px as f32 / self.pixels_per_mm,
            self.height_px as f32 / self.pixels_per_mm,
        ]
    }

    /// Decode compressed bytes to RGBA8 pixels.
    pub fn decode_to_rgba(&self) -> Result<(Vec<u8>, u32, u32), String> {
        let img = image::load_from_memory(&self.image_data)
            .map_err(|e| format!("Failed to decode image: {e}"))?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        Ok((rgba.into_raw(), w, h))
    }
}

/// Get image dimensions without fully decoding.
fn image_dimensions(data: &[u8]) -> Result<(u32, u32), String> {
    let reader = image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("Cannot determine image format: {e}"))?;
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| format!("Cannot read image dimensions: {e}"))?;
    Ok((w, h))
}

/// Apply a colour mode transformation to RGBA pixel data (in-place).
pub fn apply_colour_mode(pixels: &mut [u8], mode: &ColourMode) {
    match mode {
        ColourMode::Normal => {}
        ColourMode::Inverted => {
            for chunk in pixels.chunks_exact_mut(4) {
                chunk[0] = 255 - chunk[0];
                chunk[1] = 255 - chunk[1];
                chunk[2] = 255 - chunk[2];
            }
        }
        ColourMode::Desaturated => {
            for chunk in pixels.chunks_exact_mut(4) {
                let grey = ((chunk[0] as u16 * 77 + chunk[1] as u16 * 150 + chunk[2] as u16 * 29) >> 8) as u8;
                chunk[0] = grey;
                chunk[1] = grey;
                chunk[2] = grey;
            }
        }
        ColourMode::Tinted(tint) => {
            for chunk in pixels.chunks_exact_mut(4) {
                let grey = ((chunk[0] as u16 * 77 + chunk[1] as u16 * 150 + chunk[2] as u16 * 29) >> 8) as u16;
                chunk[0] = ((grey * tint[0] as u16) >> 8) as u8;
                chunk[1] = ((grey * tint[1] as u16) >> 8) as u8;
                chunk[2] = ((grey * tint[2] as u16) >> 8) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tiny_png() -> Vec<u8> {
        // Create a minimal 2x2 red PNG
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        let pixels: [u8; 16] = [
            255, 0, 0, 255,  // red
            0, 255, 0, 255,  // green
            0, 0, 255, 255,  // blue
            255, 255, 0, 255, // yellow
        ];
        image::ImageEncoder::write_image(
            encoder,
            &pixels,
            2,
            2,
            image::ColorType::Rgba8.into(),
        )
        .unwrap();
        buf
    }

    #[test]
    fn test_from_bytes() {
        let png = make_tiny_png();
        let img = BackgroundImage::from_bytes("test.png".into(), png).unwrap();
        assert_eq!(img.width_px, 2);
        assert_eq!(img.height_px, 2);
        assert_eq!(img.opacity, 0.3);
        assert_eq!(img.pixels_per_mm, 10.0);
    }

    #[test]
    fn test_size_mm() {
        let png = make_tiny_png();
        let img = BackgroundImage::from_bytes("test.png".into(), png).unwrap();
        let size = img.size_mm();
        assert!((size[0] - 0.2).abs() < 0.01);
        assert!((size[1] - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_decode_roundtrip() {
        let png = make_tiny_png();
        let img = BackgroundImage::from_bytes("test.png".into(), png).unwrap();
        let (pixels, w, h) = img.decode_to_rgba().unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(pixels.len(), 16);
        assert_eq!(&pixels[0..4], &[255, 0, 0, 255]); // red pixel
    }

    #[test]
    fn test_colour_mode_invert() {
        let mut pixels = vec![100, 50, 200, 255, 0, 255, 128, 255];
        apply_colour_mode(&mut pixels, &ColourMode::Inverted);
        assert_eq!(pixels[0], 155);
        assert_eq!(pixels[1], 205);
        assert_eq!(pixels[2], 55);
        assert_eq!(pixels[3], 255); // alpha unchanged
        assert_eq!(pixels[4], 255);
        assert_eq!(pixels[5], 0);
        assert_eq!(pixels[6], 127);
    }

    #[test]
    fn test_colour_mode_desaturate() {
        let mut pixels = vec![255, 0, 0, 255];
        apply_colour_mode(&mut pixels, &ColourMode::Desaturated);
        // 255*77/256 ≈ 76
        assert!(pixels[0] > 70 && pixels[0] < 80);
        assert_eq!(pixels[0], pixels[1]);
        assert_eq!(pixels[1], pixels[2]);
        assert_eq!(pixels[3], 255);
    }
}
