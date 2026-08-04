use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::entity::{DomainData, Geometry};

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

/// Point-in-polygon test accounting for holes.
/// Returns true if point is inside the outer polygon and outside all holes.
pub fn point_in_polygon_with_holes(point: Vec2, outer: &[Vec2], holes: &[Vec<Vec2>]) -> bool {
    if !point_in_polygon(point, outer) {
        return false;
    }
    for hole in holes {
        if point_in_polygon(point, hole) {
            return false;
        }
    }
    true
}

/// Check if all vertices of `inner` are inside `outer`.
/// Used to validate that a cutout hole is fully contained within the parent polygon.
pub fn polygon_contains_polygon(outer: &[Vec2], inner: &[Vec2]) -> bool {
    inner.iter().all(|p| point_in_polygon(*p, outer))
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

/// Triangulate a polygon with holes using ear-clipping.
/// Returns a list of triangle vertex positions suitable for rendering.
/// Each group of 3 consecutive Vec2 values forms one triangle.
pub fn triangulate_polygon_with_holes(
    outer: &[Vec2],
    holes: &[Vec<Vec2>],
) -> Vec<[Vec2; 3]> {
    if outer.len() < 3 {
        return Vec::new();
    }

    // Normalize winding: outer must be CCW (negative signed area), holes must be CW (positive)
    // This is the convention earcutr expects.
    let mut outer_normalized: Vec<Vec2> = outer.to_vec();
    if polygon_area(&outer_normalized) > 0.0 {
        outer_normalized.reverse();
    }
    let mut holes_normalized: Vec<Vec<Vec2>> = holes.to_vec();
    for hole in &mut holes_normalized {
        if polygon_area(hole) < 0.0 {
            hole.reverse();
        }
    }

    // Flatten all coordinates into a single array for earcutr
    let mut coords: Vec<f64> = Vec::with_capacity((outer_normalized.len() + holes_normalized.iter().map(|h| h.len()).sum::<usize>()) * 2);
    for v in &outer_normalized {
        coords.push(v.x as f64);
        coords.push(v.y as f64);
    }

    let mut hole_indices: Vec<usize> = Vec::new();
    for hole in &holes_normalized {
        hole_indices.push(coords.len() / 2);
        for v in hole {
            coords.push(v.x as f64);
            coords.push(v.y as f64);
        }
    }

    let indices = earcutr::earcut(&coords, &hole_indices, 2).unwrap_or_default();

    // Build all vertices list for indexing
    let mut all_verts: Vec<Vec2> = outer_normalized;
    for hole in holes_normalized {
        all_verts.extend_from_slice(&hole);
    }

    let mut triangles = Vec::with_capacity(indices.len() / 3);
    for tri in indices.chunks(3) {
        if tri.len() == 3 {
            triangles.push([all_verts[tri[0]], all_verts[tri[1]], all_verts[tri[2]]]);
        }
    }
    triangles
}

// ---------------------------------------------------------------------------
// Edge trace support
// ---------------------------------------------------------------------------

/// A visible boundary polyline extracted from an entity for edge tracing.
pub struct VisibleEdge {
    pub vertices: Vec<Vec2>,
    pub closed: bool,
}

/// Extract visible boundary polylines for an entity.
///
/// Width-offset entities (Wall, Path, Edging, Step, Surface with width, House)
/// return both left and right offset sides as open polylines.
/// Other geometry types return their natural boundary.
pub fn visible_edges(geom: &Geometry, domain: &DomainData) -> Vec<VisibleEdge> {
    // Width-offset entities: compute offset sides from centerline
    if let Some(width) = domain.width_mm() {
        if let Some(center_pts) = geom.centerline_points() {
            if center_pts.len() >= 2 {
                let half_w = width / 2.0;
                let (left, right) = offset_polyline_sides(&center_pts, half_w);
                return vec![
                    VisibleEdge { vertices: left, closed: false },
                    VisibleEdge { vertices: right, closed: false },
                ];
            }
        }
    }

    match geom {
        Geometry::Polygon(pg) => {
            let mut edges = vec![VisibleEdge {
                vertices: pg.vertices.clone(),
                closed: true,
            }];
            for hole in &pg.holes {
                edges.push(VisibleEdge {
                    vertices: hole.clone(),
                    closed: true,
                });
            }
            edges
        }
        Geometry::Polyline(pl) => {
            vec![VisibleEdge {
                vertices: pl.vertices.clone(),
                closed: pl.closed,
            }]
        }
        Geometry::Rectangle(r) => {
            let half = r.half_size;
            let local = [
                Vec2::new(-half.x, -half.y),
                Vec2::new(half.x, -half.y),
                Vec2::new(half.x, half.y),
                Vec2::new(-half.x, half.y),
            ];
            let corners: Vec<Vec2> = if r.rotation.abs() < f32::EPSILON {
                local.iter().map(|c| r.center + *c).collect()
            } else {
                let (sin, cos) = r.rotation.sin_cos();
                local
                    .iter()
                    .map(|c| {
                        r.center + Vec2::new(c.x * cos - c.y * sin, c.x * sin + c.y * cos)
                    })
                    .collect()
            };
            vec![VisibleEdge { vertices: corners, closed: true }]
        }
        Geometry::Circle(c) => {
            let n = 64;
            let vertices: Vec<Vec2> = (0..n)
                .map(|i| {
                    let angle = (i as f32) * std::f32::consts::TAU / (n as f32);
                    c.center + Vec2::new(angle.cos(), angle.sin()) * c.radius
                })
                .collect();
            vec![VisibleEdge { vertices, closed: true }]
        }
        Geometry::Arc(a) => {
            let n = 32;
            let sweep = a.end_angle - a.start_angle;
            let vertices: Vec<Vec2> = (0..=n)
                .map(|i| {
                    let angle = a.start_angle + sweep * (i as f32) / (n as f32);
                    a.center + Vec2::new(angle.cos(), angle.sin()) * a.radius
                })
                .collect();
            vec![VisibleEdge { vertices, closed: false }]
        }
        Geometry::BezierPath(bp) => {
            let flat = bp.flatten(16);
            vec![VisibleEdge { vertices: flat, closed: bp.closed }]
        }
        Geometry::Line(l) => {
            vec![VisibleEdge {
                vertices: vec![l.start, l.end],
                closed: false,
            }]
        }
        _ => Vec::new(),
    }
}

/// Find the closest point on a polyline to a given point.
/// Returns `(projected_point, segment_index, t_along_segment)`.
pub fn closest_point_on_polyline(
    point: Vec2,
    vertices: &[Vec2],
    closed: bool,
) -> (Vec2, usize, f32) {
    let n = vertices.len();
    if n == 0 {
        return (Vec2::ZERO, 0, 0.0);
    }
    if n == 1 {
        return (vertices[0], 0, 0.0);
    }

    let seg_count = if closed { n } else { n - 1 };
    let mut best_dist = f32::MAX;
    let mut best_point = vertices[0];
    let mut best_seg = 0usize;
    let mut best_t = 0.0f32;

    for i in 0..seg_count {
        let a = vertices[i];
        let b = vertices[(i + 1) % n];
        let ab = b - a;
        let len_sq = ab.length_squared();
        let t = if len_sq < f32::EPSILON {
            0.0
        } else {
            ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0)
        };
        let proj = a + ab * t;
        let dist = (point - proj).length();
        if dist < best_dist {
            best_dist = dist;
            best_point = proj;
            best_seg = i;
            best_t = t;
        }
    }

    (best_point, best_seg, best_t)
}

/// Extract a section of a polyline between two boundary points.
///
/// `start_seg`/`start_t` and `end_seg`/`end_t` identify the two points on the polyline.
/// For closed polylines, `reverse` takes the other way around.
/// For open polylines, `reverse` is ignored (there's only one path).
pub fn extract_polyline_section(
    vertices: &[Vec2],
    closed: bool,
    start_seg: usize,
    start_t: f32,
    end_seg: usize,
    end_t: f32,
    reverse: bool,
) -> Vec<Vec2> {
    let n = vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let start_pt = vertices[start_seg]
        + (vertices[(start_seg + 1) % n] - vertices[start_seg]) * start_t;
    let end_pt =
        vertices[end_seg] + (vertices[(end_seg + 1) % n] - vertices[end_seg]) * end_t;

    if !closed {
        // Open polyline: always go from earlier to later
        let (s_seg, _s_t, s_pt, e_seg, e_t, e_pt) =
            if (start_seg, ordered_float(start_t)) <= (end_seg, ordered_float(end_t)) {
                (start_seg, start_t, start_pt, end_seg, end_t, end_pt)
            } else {
                (end_seg, end_t, end_pt, start_seg, start_t, start_pt)
            };

        let mut result = vec![s_pt];
        // Add interior vertices between start and end segments
        let first_interior = s_seg + 1;
        let last_interior = if e_t > 0.0 { e_seg + 1 } else { e_seg };
        for &v in vertices.iter().take(last_interior).skip(first_interior) {
            result.push(v);
        }
        if (e_pt - *result.last().unwrap()).length() > 0.01 {
            result.push(e_pt);
        }
        return result;
    }

    // Closed polyline: compute forward path
    let forward = extract_closed_forward(vertices, start_seg, start_pt, end_seg, end_pt);

    if !reverse {
        // Pick shorter path by default
        let backward = extract_closed_forward(vertices, end_seg, end_pt, start_seg, start_pt);
        let fwd_len = polyline_length(&forward);
        let bwd_len = polyline_length(&backward);
        if fwd_len <= bwd_len { forward } else { backward.into_iter().rev().collect() }
    } else {
        // Reverse: take the other way
        let backward = extract_closed_forward(vertices, end_seg, end_pt, start_seg, start_pt);
        backward.into_iter().rev().collect()
    }
}

/// Extract forward path on a closed polyline from start to end, going in index order.
fn extract_closed_forward(
    vertices: &[Vec2],
    start_seg: usize,
    start_pt: Vec2,
    end_seg: usize,
    end_pt: Vec2,
) -> Vec<Vec2> {
    let n = vertices.len();
    let mut result = vec![start_pt];

    let mut seg = start_seg;
    loop {
        let next_vert = (seg + 1) % n;
        if seg == end_seg {
            if (end_pt - *result.last().unwrap()).length() > 0.01 {
                result.push(end_pt);
            }
            break;
        }
        result.push(vertices[next_vert]);
        seg = next_vert;
        // Safety: if we've gone all the way around
        if seg == start_seg {
            if (end_pt - *result.last().unwrap()).length() > 0.01 {
                result.push(end_pt);
            }
            break;
        }
    }

    result
}

fn polyline_length(pts: &[Vec2]) -> f32 {
    pts.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

/// Helper to make f32 orderable for tuple comparison.
fn ordered_float(f: f32) -> u32 {
    f.to_bits()
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

    #[test]
    fn test_point_in_polygon_with_holes() {
        let outer = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 100.0),
        ];
        let hole = vec![
            Vec2::new(30.0, 30.0),
            Vec2::new(70.0, 30.0),
            Vec2::new(70.0, 70.0),
            Vec2::new(30.0, 70.0),
        ];
        let holes = vec![hole];

        // Point inside outer but outside hole
        assert!(point_in_polygon_with_holes(Vec2::new(10.0, 10.0), &outer, &holes));
        // Point inside hole
        assert!(!point_in_polygon_with_holes(Vec2::new(50.0, 50.0), &outer, &holes));
        // Point outside outer
        assert!(!point_in_polygon_with_holes(Vec2::new(150.0, 50.0), &outer, &holes));
    }

    #[test]
    fn test_polygon_contains_polygon() {
        let outer = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 100.0),
        ];
        let inner = vec![
            Vec2::new(20.0, 20.0),
            Vec2::new(80.0, 20.0),
            Vec2::new(80.0, 80.0),
            Vec2::new(20.0, 80.0),
        ];
        assert!(polygon_contains_polygon(&outer, &inner));

        // Inner extends beyond outer
        let outside = vec![
            Vec2::new(50.0, 50.0),
            Vec2::new(150.0, 50.0),
            Vec2::new(150.0, 150.0),
        ];
        assert!(!polygon_contains_polygon(&outer, &outside));
    }

    #[test]
    fn test_triangulate_polygon_with_holes() {
        let outer = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 100.0),
        ];
        let hole = vec![
            Vec2::new(30.0, 30.0),
            Vec2::new(70.0, 30.0),
            Vec2::new(70.0, 70.0),
            Vec2::new(30.0, 70.0),
        ];
        let triangles = triangulate_polygon_with_holes(&outer, &[hole]);
        // Should produce some triangles (exact count depends on ear-clipping)
        assert!(!triangles.is_empty());
        // Each triangle should have 3 vertices
        for tri in &triangles {
            assert_eq!(tri.len(), 3);
        }
    }

    #[test]
    fn test_triangulate_no_holes() {
        let outer = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        let triangles = triangulate_polygon_with_holes(&outer, &[]);
        assert_eq!(triangles.len(), 2); // A quad splits into 2 triangles
    }

    // Edge trace tests

    #[test]
    fn test_closest_point_on_polyline_open() {
        let verts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
        ];
        // Point above midpoint of first segment
        let (pt, seg, t) = closest_point_on_polyline(Vec2::new(50.0, 10.0), &verts, false);
        assert_eq!(seg, 0);
        assert!((t - 0.5).abs() < 0.01);
        assert!((pt.x - 50.0).abs() < 0.01);
        assert!(pt.y.abs() < 0.01);
    }

    #[test]
    fn test_closest_point_on_polyline_closed() {
        // Square: closing edge from (0,10) back to (0,0)
        let verts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        // Point to the left of the closing edge
        let (pt, seg, _t) = closest_point_on_polyline(Vec2::new(-5.0, 5.0), &verts, true);
        assert_eq!(seg, 3); // closing segment: vertex 3 -> vertex 0
        assert!((pt.x - 0.0).abs() < 0.01);
        assert!((pt.y - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_extract_polyline_section_open() {
        let verts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(20.0, 0.0),
            Vec2::new(30.0, 0.0),
        ];
        // Extract from midpoint of seg 0 to midpoint of seg 2
        let section = extract_polyline_section(&verts, false, 0, 0.5, 2, 0.5, false);
        assert_eq!(section.len(), 4); // start_pt, v[1], v[2], end_pt
        assert!((section[0].x - 5.0).abs() < 0.01);
        assert!((section[1].x - 10.0).abs() < 0.01);
        assert!((section[2].x - 20.0).abs() < 0.01);
        assert!((section[3].x - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_extract_polyline_section_closed_shorter_path() {
        // Square: 0=(0,0), 1=(10,0), 2=(10,10), 3=(0,10)
        let verts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        // From midpoint of seg 0 to midpoint of seg 1 (short side, 2 edges)
        let section = extract_polyline_section(&verts, true, 0, 0.5, 1, 0.5, false);
        // Forward: (5,0) -> (10,0) -> (10,5): 3 points, length ~15
        // Backward: (5,0) -> (0,0) -> (0,10) -> (10,10) -> (10,5): 5 points, length ~35
        // Should pick forward (shorter)
        assert!(section.len() == 3);
        assert!((section[0].x - 5.0).abs() < 0.01);
        assert!((section[1].x - 10.0).abs() < 0.01);
        assert!((section[2] - Vec2::new(10.0, 5.0)).length() < 0.01);
    }

    #[test]
    fn test_extract_polyline_section_closed_reversed() {
        let verts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        let forward = extract_polyline_section(&verts, true, 0, 0.5, 1, 0.5, false);
        let reversed = extract_polyline_section(&verts, true, 0, 0.5, 1, 0.5, true);
        // Reversed should be the longer path
        assert!(reversed.len() > forward.len());
    }

    #[test]
    fn test_visible_edges_polygon() {
        use crate::entity::{PolygonData, Geometry, DomainData};
        let geom = Geometry::Polygon(PolygonData {
            vertices: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(10.0, 0.0),
                Vec2::new(10.0, 10.0),
            ],
            holes: Vec::new(),
        });
        let edges = visible_edges(&geom, &DomainData::None);
        assert_eq!(edges.len(), 1);
        assert!(edges[0].closed);
        assert_eq!(edges[0].vertices.len(), 3);
    }

    #[test]
    fn test_visible_edges_wall_offset() {
        use crate::entity::{Geometry, LineData, DomainData};
        use crate::garden::hardscape::WallData;
        let geom = Geometry::Line(LineData {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(100.0, 0.0),
        });
        let domain = DomainData::Wall(WallData {
            width_mm: 200,
            ..WallData::default()
        });
        let edges = visible_edges(&geom, &domain);
        // Should get 2 edges (left and right offset)
        assert_eq!(edges.len(), 2);
        assert!(!edges[0].closed);
        assert!(!edges[1].closed);
        // Left should be offset in one direction, right in the other
        let left_y = edges[0].vertices[0].y;
        let right_y = edges[1].vertices[0].y;
        assert!((left_y - (-100.0)).abs() < 1.0 || (left_y - 100.0).abs() < 1.0);
        assert!((right_y - (-100.0)).abs() < 1.0 || (right_y - 100.0).abs() < 1.0);
        assert!((left_y - right_y).abs() > 100.0); // they're on opposite sides
    }
}
