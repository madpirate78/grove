use glam::Vec2;

/// Manages the 2D view transform: pan offset + zoom level.
/// Converts between world coordinates (mm) and screen coordinates (pixels).
#[derive(Debug, Clone)]
pub struct ViewTransform {
    /// Pan offset in screen pixels.
    pub offset: Vec2,
    /// Zoom level: pixels per mm.
    pub zoom: f32,
    /// Minimum zoom level.
    pub min_zoom: f32,
    /// Maximum zoom level.
    pub max_zoom: f32,
}

impl ViewTransform {
    pub fn new() -> Self {
        Self {
            offset: Vec2::ZERO,
            zoom: 1.0,
            min_zoom: 0.001,
            max_zoom: 10.0,
        }
    }

    /// Create a ViewTransform with custom zoom limits.
    pub fn with_zoom_limits(min_zoom: f32, max_zoom: f32) -> Self {
        Self {
            offset: Vec2::ZERO,
            zoom: 1.0,
            min_zoom,
            max_zoom,
        }
    }

    /// Convert world coordinates (mm) to screen coordinates (pixels).
    pub fn world_to_screen(&self, world: Vec2) -> egui::Pos2 {
        let screen = world * self.zoom + self.offset;
        egui::Pos2::new(screen.x, screen.y)
    }

    /// Convert screen coordinates (pixels) to world coordinates (mm).
    pub fn screen_to_world(&self, screen: egui::Pos2) -> Vec2 {
        let s = Vec2::new(screen.x, screen.y);
        (s - self.offset) / self.zoom
    }

    /// Pan by a screen-space delta.
    pub fn pan(&mut self, delta: egui::Vec2) {
        self.offset += Vec2::new(delta.x, delta.y);
    }

    /// Zoom centred on a screen-space point.
    pub fn zoom_at(&mut self, screen_center: egui::Pos2, factor: f32) {
        let new_zoom = (self.zoom * factor).clamp(self.min_zoom, self.max_zoom);
        if (new_zoom - self.zoom).abs() < f32::EPSILON {
            return;
        }

        // Adjust offset so the point under the cursor stays fixed
        let center = Vec2::new(screen_center.x, screen_center.y);
        let world_pos = (center - self.offset) / self.zoom;
        self.zoom = new_zoom;
        self.offset = center - world_pos * self.zoom;
    }

    /// Convert a world-space distance to screen pixels.
    pub fn world_to_screen_distance(&self, world_dist: f32) -> f32 {
        world_dist * self.zoom
    }

    /// Fit the view to show a world-space rectangle.
    pub fn fit_to_rect(&mut self, min: Vec2, max: Vec2, screen_size: egui::Vec2, margin: f32) {
        let world_size = max - min;
        if world_size.x < 1.0 || world_size.y < 1.0 {
            return;
        }
        let available = egui::Vec2::new(
            screen_size.x - margin * 2.0,
            screen_size.y - margin * 2.0,
        );
        let zoom_x = available.x / world_size.x;
        let zoom_y = available.y / world_size.y;
        self.zoom = zoom_x.min(zoom_y).clamp(self.min_zoom, self.max_zoom);

        let world_center = (min + max) * 0.5;
        let screen_center = Vec2::new(screen_size.x / 2.0, screen_size.y / 2.0);
        self.offset = screen_center - world_center * self.zoom;
    }
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_screen_roundtrip() {
        let vt = ViewTransform {
            offset: Vec2::new(100.0, 50.0),
            zoom: 2.0,
            ..Default::default()
        };
        let world = Vec2::new(50.0, 30.0);
        let screen = vt.world_to_screen(world);
        let back = vt.screen_to_world(screen);
        assert!((back.x - world.x).abs() < 0.01);
        assert!((back.y - world.y).abs() < 0.01);
    }

    #[test]
    fn test_zoom_clamping() {
        let mut vt = ViewTransform::new();
        vt.zoom_at(egui::Pos2::ZERO, 0.001);
        assert!(vt.zoom >= vt.min_zoom);
        vt.zoom_at(egui::Pos2::ZERO, 10000.0);
        assert!(vt.zoom <= vt.max_zoom);
    }
}
