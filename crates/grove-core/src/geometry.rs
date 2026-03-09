use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: Vec2,
    pub max: Vec2,
}

impl BoundingBox {
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    pub fn from_points(points: &[Vec2]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }
        let mut min = points[0];
        let mut max = points[0];
        for &p in &points[1..] {
            min = min.min(p);
            max = max.max(p);
        }
        Some(Self { min, max })
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    pub fn expand(&self, amount: f32) -> Self {
        Self {
            min: self.min - Vec2::splat(amount),
            max: self.max + Vec2::splat(amount),
        }
    }

    pub fn union(&self, other: &BoundingBox) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}

/// Point-in-polygon test using ray casting algorithm.
pub fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let pi = polygon[i];
        let pj = polygon[j];
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Calculate area of a polygon using the shoelace formula.
/// Returns positive for counter-clockwise, negative for clockwise.
pub fn polygon_area(polygon: &[Vec2]) -> f32 {
    let n = polygon.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    let mut j = n - 1;
    for i in 0..n {
        area += (polygon[j].x + polygon[i].x) * (polygon[j].y - polygon[i].y);
        j = i;
    }
    area * 0.5
}

/// Calculate centroid of a polygon.
pub fn polygon_centroid(polygon: &[Vec2]) -> Option<Vec2> {
    let n = polygon.len();
    if n == 0 {
        return None;
    }
    let sum: Vec2 = polygon.iter().copied().sum();
    Some(sum / n as f32)
}

/// Distance from a point to a line segment.
pub fn point_to_segment_distance(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let ap = point - a;
    let len_sq = ab.length_squared();
    if len_sq < f32::EPSILON {
        return ap.length();
    }
    let t = (ap.dot(ab) / len_sq).clamp(0.0, 1.0);
    let projection = a + ab * t;
    (point - projection).length()
}

/// Closest point on a line segment to a given point.
pub fn closest_point_on_segment(point: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < f32::EPSILON {
        return a;
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    a + ab * t
}

/// Distance from a point to a polyline (minimum over all segments).
pub fn point_to_polyline_distance(point: Vec2, vertices: &[Vec2]) -> f32 {
    if vertices.len() < 2 {
        return if vertices.is_empty() {
            f32::MAX
        } else {
            (point - vertices[0]).length()
        };
    }
    vertices
        .windows(2)
        .map(|seg| point_to_segment_distance(point, seg[0], seg[1]))
        .fold(f32::MAX, f32::min)
}

/// Compute left and right offset sides of a polyline with miter joins.
/// Returns `(left, right)` arrays, each the same length as `vertices`.
/// Left is offset in the +normal direction, right in -normal.
pub fn offset_polyline_sides(vertices: &[Vec2], half_width: f32) -> (Vec<Vec2>, Vec<Vec2>) {
    let n = vertices.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    if n == 1 {
        let p = vertices[0];
        return (vec![p], vec![p]);
    }

    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);

    for i in 0..n {
        let normal = offset_normal(vertices, i);
        left.push(vertices[i] + normal * half_width);
        right.push(vertices[i] - normal * half_width);
    }

    (left, right)
}

/// Compute the offset normal at vertex `i` of a polyline (miter join for interior vertices).
fn offset_normal(vertices: &[Vec2], i: usize) -> Vec2 {
    let n = vertices.len();
    if n == 2 {
        let dir = (vertices[1] - vertices[0]).normalize_or_zero();
        return Vec2::new(-dir.y, dir.x);
    }
    if i == 0 {
        let dir = (vertices[1] - vertices[0]).normalize_or_zero();
        return Vec2::new(-dir.y, dir.x);
    }
    if i == n - 1 {
        let dir = (vertices[n - 1] - vertices[n - 2]).normalize_or_zero();
        return Vec2::new(-dir.y, dir.x);
    }
    // Interior vertex: miter join
    let d1 = (vertices[i] - vertices[i - 1]).normalize_or_zero();
    let d2 = (vertices[i + 1] - vertices[i]).normalize_or_zero();
    let n1 = Vec2::new(-d1.y, d1.x);
    let n2 = Vec2::new(-d2.y, d2.x);
    let avg = (n1 + n2) * 0.5;
    let len = avg.length();
    if len < 0.01 {
        n1
    } else {
        let miter = avg / len;
        let cos_half = miter.dot(n1);
        if cos_half.abs() < 0.01 {
            n1
        } else {
            let miter_ratio = 1.0 / cos_half;
            if miter_ratio.abs() > 2.0 {
                miter
            } else {
                miter * miter_ratio
            }
        }
    }
}

/// Compute an offset polygon from a polyline with a given half-width.
/// Returns left side going forward + right side going backward = closed polygon.
/// Uses miter joins with clamping to avoid spikes at sharp corners.
pub fn offset_polyline(vertices: &[Vec2], half_width: f32) -> Vec<Vec2> {
    if vertices.is_empty() {
        return Vec::new();
    }
    if vertices.len() == 1 {
        let p = vertices[0];
        let hw = half_width;
        return vec![
            p + Vec2::new(-hw, -hw),
            p + Vec2::new(hw, -hw),
            p + Vec2::new(hw, hw),
            p + Vec2::new(-hw, hw),
        ];
    }

    let (left, mut right) = offset_polyline_sides(vertices, half_width);
    right.reverse();
    let mut result = left;
    result.extend(right);
    result
}

/// Test whether two line segments intersect (proper crossing, not touching at endpoints).
pub fn segments_intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
    let d1 = a2 - a1;
    let d2 = b2 - b1;
    let cross = d1.x * d2.y - d1.y * d2.x;
    if cross.abs() < f32::EPSILON {
        return false; // Parallel or collinear
    }
    let d = b1 - a1;
    let t = (d.x * d2.y - d.y * d2.x) / cross;
    let u = (d.x * d1.y - d.y * d1.x) / cross;
    // Use a small epsilon inset to ignore endpoint-touching (shared vertices)
    let eps = 1e-4;
    t > eps && t < 1.0 - eps && u > eps && u < 1.0 - eps
}

/// Check whether adding `new_point` to the end of `existing_vertices` would cause
/// a self-intersection. If `closing` is true, also checks the closing edge from
/// `new_point` back to `existing_vertices[0]` (polygon mode).
pub fn would_self_intersect(existing_vertices: &[Vec2], new_point: Vec2, closing: bool) -> bool {
    let n = existing_vertices.len();
    if n < 2 {
        return false;
    }

    let last = existing_vertices[n - 1];
    // The new edge: last -> new_point
    // Test against all edges except the one ending at `last` (index n-2..n-1)
    for i in 0..n - 2 {
        if segments_intersect(
            existing_vertices[i],
            existing_vertices[i + 1],
            last,
            new_point,
        ) {
            return true;
        }
    }

    // If closing the polygon, also test the closing edge: new_point -> vertices[0]
    if closing && n >= 3 {
        // Test closing edge against all interior edges (skip first and last)
        for i in 1..n - 1 {
            if segments_intersect(
                existing_vertices[i],
                existing_vertices[i + 1],
                new_point,
                existing_vertices[0],
            ) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_in_polygon_square() {
        let square = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        assert!(point_in_polygon(Vec2::new(5.0, 5.0), &square));
        assert!(!point_in_polygon(Vec2::new(15.0, 5.0), &square));
        assert!(!point_in_polygon(Vec2::new(-1.0, -1.0), &square));
    }

    #[test]
    fn test_polygon_area() {
        // 10x10 square = area 100
        let square = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        let area = polygon_area(&square).abs();
        assert!((area - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_polygon_centroid() {
        let square = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        let c = polygon_centroid(&square).unwrap();
        assert!((c.x - 5.0).abs() < 0.01);
        assert!((c.y - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_bounding_box() {
        let points = vec![Vec2::new(1.0, 2.0), Vec2::new(5.0, 8.0), Vec2::new(3.0, 4.0)];
        let bb = BoundingBox::from_points(&points).unwrap();
        assert_eq!(bb.min, Vec2::new(1.0, 2.0));
        assert_eq!(bb.max, Vec2::new(5.0, 8.0));
        assert!(bb.contains(Vec2::new(3.0, 5.0)));
        assert!(!bb.contains(Vec2::new(0.0, 0.0)));
    }

    #[test]
    fn test_point_to_segment_distance() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(10.0, 0.0);
        // Point directly above the midpoint
        let dist = point_to_segment_distance(Vec2::new(5.0, 3.0), a, b);
        assert!((dist - 3.0).abs() < 0.01);
        // Point beyond endpoint
        let dist2 = point_to_segment_distance(Vec2::new(12.0, 0.0), a, b);
        assert!((dist2 - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_offset_polyline_straight() {
        let verts = vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)];
        let poly = offset_polyline(&verts, 10.0);
        // Should produce 4 points forming a rectangle
        assert_eq!(poly.len(), 4);
        // Left side should be at y = -10 (normal points up in screen coords)
        // Right side at y = 10
        let min_y = poly.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_y = poly.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        assert!((min_y - (-10.0)).abs() < 0.1);
        assert!((max_y - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_offset_polyline_right_angle() {
        let verts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
        ];
        let poly = offset_polyline(&verts, 10.0);
        // Should produce 6 points (3 left + 3 right)
        assert_eq!(poly.len(), 6);
    }

    #[test]
    fn test_offset_polyline_single_point() {
        let verts = vec![Vec2::new(50.0, 50.0)];
        let poly = offset_polyline(&verts, 5.0);
        assert_eq!(poly.len(), 4); // Small square
    }

    #[test]
    fn test_segments_intersect_crossing() {
        assert!(segments_intersect(
            Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0),
            Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0),
        ));
    }

    #[test]
    fn test_segments_intersect_no_crossing() {
        assert!(!segments_intersect(
            Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0),
            Vec2::new(6.0, 0.0), Vec2::new(10.0, 0.0),
        ));
    }

    #[test]
    fn test_segments_intersect_shared_endpoint_not_crossing() {
        // Two edges sharing an endpoint should NOT count as intersecting
        assert!(!segments_intersect(
            Vec2::new(0.0, 0.0), Vec2::new(5.0, 5.0),
            Vec2::new(5.0, 5.0), Vec2::new(10.0, 0.0),
        ));
    }

    #[test]
    fn test_would_self_intersect_figure_eight() {
        // L-shape so far: (0,0) -> (10,0) -> (10,10)
        // Adding (0,5) would cross the first edge
        let verts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
        ];
        // This new edge (10,10)->(0,5) doesn't cross existing edges
        assert!(!would_self_intersect(&verts, Vec2::new(0.0, 5.0), false));
        // But (10,10)->(-1,5) extended to cross first segment? Let's try a real crossing:
        // From (10,10) to (5,-5) would cross (0,0)-(10,0)
        assert!(would_self_intersect(&verts, Vec2::new(5.0, -5.0), false));
    }

    #[test]
    fn test_would_self_intersect_closing() {
        // Square drawn as: (0,0) -> (10,0) -> (10,10) -> (0,10) -- closing won't cross
        let verts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
        ];
        // Adding (0,10) and closing back to (0,0) should be fine
        assert!(!would_self_intersect(&verts, Vec2::new(0.0, 10.0), true));

        // Adding (5,5) and closing back to (0,0) — the closing edge would cross (10,0)-(10,10)? No.
        // Actually (5,5)->(0,0) doesn't cross (10,0)-(10,10). Let's do a real crossing:
        // Adding (5,-5) — edge from (10,10) to (5,-5) crosses (0,0)-(10,0)
        assert!(would_self_intersect(&verts, Vec2::new(5.0, -5.0), true));
    }
}
