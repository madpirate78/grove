use glam::Vec2;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;

use crate::geometry::polygon_area;

/// Result of a boolean difference operation on a polygon.
#[derive(Debug, Clone)]
pub enum DifferenceResult {
    /// Cutout had no overlap — polygon unchanged.
    Unchanged,
    /// Polygon was modified (boundary changed and/or holes added).
    Modified {
        outer: Vec<Vec2>,
        holes: Vec<Vec<Vec2>>,
    },
}

/// Perform boolean difference: `outer - cutout`, accounting for existing holes.
///
/// Returns `Unchanged` if the cutout doesn't overlap the polygon.
/// Returns `Modified` with the new outer boundary and holes otherwise.
/// If the operation splits the polygon into multiple pieces, keeps the largest by area.
pub fn polygon_difference(
    outer: &[Vec2],
    holes: &[Vec<Vec2>],
    cutout: &[Vec2],
) -> DifferenceResult {
    if outer.len() < 3 || cutout.len() < 3 {
        return DifferenceResult::Unchanged;
    }

    // Build subject: first contour is outer, rest are holes
    let mut subj: Vec<Vec<[f32; 2]>> = Vec::with_capacity(1 + holes.len());
    subj.push(vec2_slice_to_f32_array(outer));
    for hole in holes {
        subj.push(vec2_slice_to_f32_array(hole));
    }

    let clip: Vec<[f32; 2]> = vec2_slice_to_f32_array(cutout);

    let result = subj.overlay(&clip, OverlayRule::Difference, FillRule::EvenOdd);

    if result.is_empty() {
        // Entire polygon was subtracted
        return DifferenceResult::Modified {
            outer: Vec::new(),
            holes: Vec::new(),
        };
    }

    // Find the largest shape by outer contour area
    let mut best_idx = 0;
    let mut best_area: f32 = 0.0;
    for (i, shape) in result.iter().enumerate() {
        if let Some(contour) = shape.first() {
            let verts = f32_array_to_vec2_slice(contour);
            let area = polygon_area(&verts).abs();
            if area > best_area {
                best_area = area;
                best_idx = i;
            }
        }
    }

    let shape = &result[best_idx];
    if shape.is_empty() {
        return DifferenceResult::Unchanged;
    }

    let new_outer = f32_array_to_vec2_slice(&shape[0]);
    let new_holes: Vec<Vec<Vec2>> = shape[1..]
        .iter()
        .map(|contour| f32_array_to_vec2_slice(contour))
        .collect();

    // Check if result is essentially unchanged:
    // same outer area, same vertex count, AND same number of holes
    let original_area = polygon_area(outer).abs();
    if (best_area - original_area).abs() < 0.01
        && new_outer.len() == outer.len()
        && new_holes.len() == holes.len()
    {
        return DifferenceResult::Unchanged;
    }

    DifferenceResult::Modified {
        outer: new_outer,
        holes: new_holes,
    }
}

/// Generate polygon vertices approximating a circle.
pub fn circle_to_polygon(center: Vec2, radius: f32, segments: u32) -> Vec<Vec2> {
    let segments = segments.max(8);
    let mut points = Vec::with_capacity(segments as usize);
    for i in 0..segments {
        let angle = 2.0 * std::f32::consts::PI * (i as f32) / (segments as f32);
        points.push(Vec2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        ));
    }
    points
}

/// Generate polygon vertices for an axis-aligned rectangle.
pub fn rect_to_polygon(center: Vec2, half_size: Vec2) -> Vec<Vec2> {
    vec![
        Vec2::new(center.x - half_size.x, center.y - half_size.y),
        Vec2::new(center.x + half_size.x, center.y - half_size.y),
        Vec2::new(center.x + half_size.x, center.y + half_size.y),
        Vec2::new(center.x - half_size.x, center.y + half_size.y),
    ]
}

fn vec2_slice_to_f32_array(verts: &[Vec2]) -> Vec<[f32; 2]> {
    verts.iter().map(|v| [v.x, v.y]).collect()
}

fn f32_array_to_vec2_slice(arr: &[[f32; 2]]) -> Vec<Vec2> {
    arr.iter().map(|p| Vec2::new(p[0], p[1])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f32, y: f32, size: f32) -> Vec<Vec2> {
        vec![
            Vec2::new(x, y),
            Vec2::new(x + size, y),
            Vec2::new(x + size, y + size),
            Vec2::new(x, y + size),
        ]
    }

    #[test]
    fn test_full_interior_cutout_produces_hole() {
        let outer = square(0.0, 0.0, 100.0);
        let cutout = square(30.0, 30.0, 40.0);

        let result = polygon_difference(&outer, &[], &cutout);
        match result {
            DifferenceResult::Modified { outer: new_outer, holes } => {
                // Outer boundary should remain roughly the same
                let area = polygon_area(&new_outer).abs();
                assert!((area - 10000.0).abs() < 1.0, "outer area: {area}");
                // Should have one hole
                assert_eq!(holes.len(), 1, "expected 1 hole, got {}", holes.len());
                let hole_area = polygon_area(&holes[0]).abs();
                assert!((hole_area - 1600.0).abs() < 1.0, "hole area: {hole_area}");
            }
            DifferenceResult::Unchanged => panic!("expected Modified, got Unchanged"),
        }
    }

    #[test]
    fn test_partial_overlap_modifies_boundary() {
        let outer = square(0.0, 0.0, 100.0);
        // Cutout overlaps right edge: 80..120, 30..70
        let cutout = square(80.0, 30.0, 40.0);

        let result = polygon_difference(&outer, &[], &cutout);
        match result {
            DifferenceResult::Modified { outer: new_outer, holes } => {
                let area = polygon_area(&new_outer).abs();
                // Original 10000 minus overlap (20*40 = 800)
                assert!((area - 9200.0).abs() < 10.0, "area: {area}");
                // No holes — it's a notch
                assert_eq!(holes.len(), 0, "expected 0 holes, got {}", holes.len());
                // Boundary should have more vertices than original
                assert!(new_outer.len() > 4, "expected more than 4 vertices for notch");
            }
            DifferenceResult::Unchanged => panic!("expected Modified, got Unchanged"),
        }
    }

    #[test]
    fn test_full_exterior_cutout_unchanged() {
        let outer = square(0.0, 0.0, 100.0);
        let cutout = square(200.0, 200.0, 40.0);

        let result = polygon_difference(&outer, &[], &cutout);
        assert!(matches!(result, DifferenceResult::Unchanged));
    }

    #[test]
    fn test_circle_to_polygon() {
        let circle = circle_to_polygon(Vec2::new(50.0, 50.0), 25.0, 48);
        assert_eq!(circle.len(), 48);
        // All points should be ~25 from center
        for p in &circle {
            let dist = (*p - Vec2::new(50.0, 50.0)).length();
            assert!((dist - 25.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_rect_to_polygon() {
        let rect = rect_to_polygon(Vec2::new(50.0, 50.0), Vec2::new(20.0, 15.0));
        assert_eq!(rect.len(), 4);
        let area = polygon_area(&rect).abs();
        assert!((area - 1200.0).abs() < 0.01); // 40 * 30 = 1200
    }

    #[test]
    fn test_cutout_on_polygon_with_existing_holes() {
        let outer = square(0.0, 0.0, 100.0);
        let existing_hole = square(10.0, 10.0, 20.0);
        // New cutout in different area
        let cutout = square(60.0, 60.0, 20.0);

        let result = polygon_difference(&outer, &[existing_hole], &cutout);
        match result {
            DifferenceResult::Modified { holes, .. } => {
                // Should have 2 holes: existing + new
                assert_eq!(holes.len(), 2, "expected 2 holes, got {}", holes.len());
            }
            DifferenceResult::Unchanged => panic!("expected Modified"),
        }
    }
}
