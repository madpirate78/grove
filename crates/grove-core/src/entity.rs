use glam::{Affine2, Vec2};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::garden::{
    hardscape::*,
    levels::*,
    plant::{PlantData, PlantGroupData},
};
use crate::drawing::{dimension::DimensionData, layout::LegendData, title_block::TitleBlockData};
use crate::geometry::BoundingBox;

// ---------------------------------------------------------------------------
// Geometry — the CAD shape (what the canvas renders and hit-tests)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineData {
    pub start: Vec2,
    pub end: Vec2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolylineData {
    pub vertices: Vec<Vec2>,
    pub closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolygonData {
    pub vertices: Vec<Vec2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleData {
    pub center: Vec2,
    pub radius: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcData {
    pub center: Vec2,
    pub radius: f32,
    pub start_angle: f32,
    pub end_angle: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RectData {
    pub center: Vec2,
    pub half_size: Vec2,
    pub rotation: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextData {
    pub position: Vec2,
    pub text: String,
    pub font_size: f32,
    pub rotation: f32,
}

/// A cubic bezier segment: 4 control points.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CubicBezierSegment {
    pub p0: Vec2,
    pub p1: Vec2,
    pub p2: Vec2,
    pub p3: Vec2,
}

impl CubicBezierSegment {
    /// Evaluate the bezier curve at parameter t in 0..1.
    pub fn point_at(&self, t: f32) -> Vec2 {
        let u = 1.0 - t;
        self.p0 * (u * u * u)
            + self.p1 * (3.0 * u * u * t)
            + self.p2 * (3.0 * u * t * t)
            + self.p3 * (t * t * t)
    }
}

/// An anchor point on a bezier path, with optional tangent handles.
/// Click = corner (no handles). Click-drag = smooth (symmetric handles).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BezierAnchor {
    /// The on-curve anchor point.
    pub point: Vec2,
    /// Control point entering this anchor (absolute position).
    pub handle_in: Option<Vec2>,
    /// Control point leaving this anchor (absolute position).
    pub handle_out: Option<Vec2>,
}

/// A bezier path stored as anchors with explicit tangent handles.
/// Each pair of consecutive anchors defines a cubic bezier segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BezierPathData {
    pub anchors: Vec<BezierAnchor>,
    pub closed: bool,
}

impl BezierPathData {
    /// Convert anchors to cubic bezier segments.
    pub fn to_bezier_segments(&self) -> Vec<CubicBezierSegment> {
        let n = self.anchors.len();
        if n < 2 {
            return Vec::new();
        }

        let mut segments = Vec::new();
        let count = if self.closed { n } else { n - 1 };

        for i in 0..count {
            let a = &self.anchors[i];
            let b = &self.anchors[(i + 1) % n];

            let p0 = a.point;
            let p1 = a.handle_out.unwrap_or(a.point);
            let p2 = b.handle_in.unwrap_or(b.point);
            let p3 = b.point;

            segments.push(CubicBezierSegment { p0, p1, p2, p3 });
        }

        segments
    }

    /// Flatten the bezier path to line segments for rendering and hit-testing.
    pub fn flatten(&self, steps_per_segment: usize) -> Vec<Vec2> {
        let beziers = self.to_bezier_segments();
        let mut points = Vec::new();
        for (i, seg) in beziers.iter().enumerate() {
            let start = if i == 0 { 0 } else { 1 };
            for j in start..=steps_per_segment {
                let t = j as f32 / steps_per_segment as f32;
                points.push(seg.point_at(t));
            }
        }
        points
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Geometry {
    None,
    Point(Vec2),
    Line(LineData),
    Polyline(PolylineData),
    Polygon(PolygonData),
    Circle(CircleData),
    Arc(ArcData),
    Rectangle(RectData),
    Text(TextData),
    BezierPath(BezierPathData),
    Dimension(DimensionData),
}

impl Geometry {
    /// Calculate the bounding box of this geometry.
    pub fn bounding_box(&self) -> Option<BoundingBox> {
        match self {
            Geometry::None => None,
            Geometry::Point(p) => Some(BoundingBox::new(*p, *p)),
            Geometry::Line(l) => {
                BoundingBox::from_points(&[l.start, l.end])
            }
            Geometry::Polyline(pl) => BoundingBox::from_points(&pl.vertices),
            Geometry::Polygon(pg) => BoundingBox::from_points(&pg.vertices),
            Geometry::Circle(c) => Some(BoundingBox::new(
                c.center - Vec2::splat(c.radius),
                c.center + Vec2::splat(c.radius),
            )),
            Geometry::Arc(a) => Some(BoundingBox::new(
                a.center - Vec2::splat(a.radius),
                a.center + Vec2::splat(a.radius),
            )),
            Geometry::Rectangle(r) => {
                let half = r.half_size;
                // For rotated rects, compute corners
                if r.rotation.abs() < f32::EPSILON {
                    Some(BoundingBox::new(r.center - half, r.center + half))
                } else {
                    let (sin, cos) = r.rotation.sin_cos();
                    let corners = [
                        Vec2::new(-half.x, -half.y),
                        Vec2::new(half.x, -half.y),
                        Vec2::new(half.x, half.y),
                        Vec2::new(-half.x, half.y),
                    ];
                    let rotated: Vec<Vec2> = corners
                        .iter()
                        .map(|c| {
                            Vec2::new(
                                c.x * cos - c.y * sin + r.center.x,
                                c.x * sin + c.y * cos + r.center.y,
                            )
                        })
                        .collect();
                    BoundingBox::from_points(&rotated)
                }
            }
            Geometry::Text(t) => {
                // Approximate: use position as a point bbox
                Some(BoundingBox::new(t.position, t.position))
            }
            Geometry::BezierPath(bp) => {
                let flat = bp.flatten(16);
                BoundingBox::from_points(&flat)
            }
            Geometry::Dimension(d) => d.bounding_box(),
        }
    }

    /// Extract centerline points for width rendering.
    /// Returns None for geometry types that don't have a linear centerline.
    pub fn centerline_points(&self) -> Option<Vec<Vec2>> {
        match self {
            Geometry::Line(l) => Some(vec![l.start, l.end]),
            Geometry::Polyline(pl) if pl.vertices.len() >= 2 => Some(pl.vertices.clone()),
            Geometry::BezierPath(bp) => {
                let flat = bp.flatten(16);
                if flat.len() >= 2 { Some(flat) } else { None }
            }
            _ => None,
        }
    }

    /// Translate all coordinates by a delta.
    pub fn translate(&mut self, delta: Vec2) {
        match self {
            Geometry::None => {}
            Geometry::Point(p) => *p += delta,
            Geometry::Line(l) => {
                l.start += delta;
                l.end += delta;
            }
            Geometry::Polyline(pl) => {
                for v in &mut pl.vertices {
                    *v += delta;
                }
            }
            Geometry::Polygon(pg) => {
                for v in &mut pg.vertices {
                    *v += delta;
                }
            }
            Geometry::Circle(c) => c.center += delta,
            Geometry::Arc(a) => a.center += delta,
            Geometry::Rectangle(r) => r.center += delta,
            Geometry::Text(t) => t.position += delta,
            Geometry::BezierPath(bp) => {
                for a in &mut bp.anchors {
                    a.point += delta;
                    if let Some(ref mut h) = a.handle_in {
                        *h += delta;
                    }
                    if let Some(ref mut h) = a.handle_out {
                        *h += delta;
                    }
                }
            }
            Geometry::Dimension(d) => match d {
                DimensionData::Linear { start, end, .. } => {
                    *start += delta;
                    *end += delta;
                }
                DimensionData::Angular { center, .. } => *center += delta,
                DimensionData::Radial { center, .. } => *center += delta,
            },
        }
    }

    /// Rotate all coordinates around a center point by an angle (radians).
    pub fn rotate(&mut self, center: Vec2, angle: f32) {
        let rotate_pt = |p: &mut Vec2| {
            *p = rotate_point(*p, center, angle);
        };
        match self {
            Geometry::None => {}
            Geometry::Point(p) => rotate_pt(p),
            Geometry::Line(l) => {
                rotate_pt(&mut l.start);
                rotate_pt(&mut l.end);
            }
            Geometry::Polyline(pl) => {
                for v in &mut pl.vertices {
                    rotate_pt(v);
                }
            }
            Geometry::Polygon(pg) => {
                for v in &mut pg.vertices {
                    rotate_pt(v);
                }
            }
            Geometry::Circle(c) => rotate_pt(&mut c.center),
            Geometry::Arc(a) => {
                rotate_pt(&mut a.center);
                a.start_angle += angle;
                a.end_angle += angle;
            }
            Geometry::Rectangle(r) => {
                rotate_pt(&mut r.center);
                r.rotation += angle;
            }
            Geometry::Text(t) => {
                rotate_pt(&mut t.position);
                t.rotation += angle;
            }
            Geometry::BezierPath(bp) => {
                for a in &mut bp.anchors {
                    rotate_pt(&mut a.point);
                    if let Some(ref mut h) = a.handle_in {
                        rotate_pt(h);
                    }
                    if let Some(ref mut h) = a.handle_out {
                        rotate_pt(h);
                    }
                }
            }
            Geometry::Dimension(d) => match d {
                DimensionData::Linear { start, end, .. } => {
                    rotate_pt(start);
                    rotate_pt(end);
                }
                DimensionData::Angular { center, start_angle, end_angle, .. } => {
                    rotate_pt(center);
                    *start_angle += angle;
                    *end_angle += angle;
                }
                DimensionData::Radial { center, angle: a, .. } => {
                    rotate_pt(center);
                    *a += angle;
                }
            },
        }
    }

    /// Scale all coordinates from a center point by a factor.
    pub fn scale(&mut self, center: Vec2, factor: f32) {
        let scale_pt = |p: &mut Vec2| {
            *p = center + (*p - center) * factor;
        };
        match self {
            Geometry::None => {}
            Geometry::Point(p) => scale_pt(p),
            Geometry::Line(l) => {
                scale_pt(&mut l.start);
                scale_pt(&mut l.end);
            }
            Geometry::Polyline(pl) => {
                for v in &mut pl.vertices {
                    scale_pt(v);
                }
            }
            Geometry::Polygon(pg) => {
                for v in &mut pg.vertices {
                    scale_pt(v);
                }
            }
            Geometry::Circle(c) => {
                scale_pt(&mut c.center);
                c.radius *= factor;
            }
            Geometry::Arc(a) => {
                scale_pt(&mut a.center);
                a.radius *= factor;
            }
            Geometry::Rectangle(r) => {
                scale_pt(&mut r.center);
                r.half_size *= factor;
            }
            Geometry::Text(t) => {
                scale_pt(&mut t.position);
                t.font_size *= factor;
            }
            Geometry::BezierPath(bp) => {
                for a in &mut bp.anchors {
                    scale_pt(&mut a.point);
                    if let Some(ref mut h) = a.handle_in {
                        scale_pt(h);
                    }
                    if let Some(ref mut h) = a.handle_out {
                        scale_pt(h);
                    }
                }
            }
            Geometry::Dimension(d) => match d {
                DimensionData::Linear { start, end, offset, .. } => {
                    scale_pt(start);
                    scale_pt(end);
                    *offset *= factor;
                }
                DimensionData::Angular { center, radius, .. } => {
                    scale_pt(center);
                    *radius *= factor;
                }
                DimensionData::Radial { center, radius, .. } => {
                    scale_pt(center);
                    *radius *= factor;
                }
            },
        }
    }

    /// Hit test: is the given point within `tolerance` of this geometry?
    pub fn hit_test(&self, point: Vec2, tolerance: f32) -> bool {
        match self {
            Geometry::None => false,
            Geometry::Point(p) => (point - *p).length() <= tolerance,
            Geometry::Line(l) => {
                crate::geometry::point_to_segment_distance(point, l.start, l.end) <= tolerance
            }
            Geometry::Polyline(pl) => {
                crate::geometry::point_to_polyline_distance(point, &pl.vertices) <= tolerance
            }
            Geometry::Polygon(pg) => {
                crate::geometry::point_in_polygon(point, &pg.vertices)
            }
            Geometry::Circle(c) => {
                let dist = (point - c.center).length();
                dist <= c.radius + tolerance
            }
            Geometry::Arc(a) => {
                let diff = point - a.center;
                let dist = diff.length();
                if (dist - a.radius).abs() > tolerance {
                    return false;
                }
                let angle = diff.y.atan2(diff.x);
                angle_in_arc(angle, a.start_angle, a.end_angle)
            }
            Geometry::Rectangle(r) => {
                // Transform point into rect local space
                let local = if r.rotation.abs() < f32::EPSILON {
                    point - r.center
                } else {
                    let (sin, cos) = (-r.rotation).sin_cos();
                    let d = point - r.center;
                    Vec2::new(d.x * cos - d.y * sin, d.x * sin + d.y * cos)
                };
                local.x.abs() <= r.half_size.x + tolerance
                    && local.y.abs() <= r.half_size.y + tolerance
            }
            Geometry::Text(t) => {
                // Simple point proximity
                (point - t.position).length() <= tolerance + t.font_size
            }
            Geometry::BezierPath(bp) => {
                let flat = bp.flatten(16);
                crate::geometry::point_to_polyline_distance(point, &flat) <= tolerance
            }
            Geometry::Dimension(_) => {
                // Dimension hit testing delegated to bounding box for now
                if let Some(bb) = self.bounding_box() {
                    bb.expand(tolerance).contains(point)
                } else {
                    false
                }
            }
        }
    }
}

/// Rotate a point around a center by an angle (radians).
fn rotate_point(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    let d = point - center;
    center + Vec2::new(d.x * cos - d.y * sin, d.x * sin + d.y * cos)
}

fn angle_in_arc(angle: f32, start: f32, end: f32) -> bool {
    let a = angle.rem_euclid(std::f32::consts::TAU);
    let s = start.rem_euclid(std::f32::consts::TAU);
    let e = end.rem_euclid(std::f32::consts::TAU);
    if s <= e {
        a >= s && a <= e
    } else {
        a >= s || a <= e
    }
}

// ---------------------------------------------------------------------------
// DomainData — what the entity "means"
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainData {
    None,
    Plant(PlantData),
    PlantGroup(PlantGroupData),
    Wall(WallData),
    Path(PathData),
    Patio(PatioData),
    Step(StepData),
    Edging(EdgingData),
    RaisedBed(RaisedBedData),
    Fence(FenceData),
    Pergola(PergolaData),
    Decking(DeckingData),
    WaterFeature(WaterFeatureData),
    SpotHeight(SpotHeightData),
    Contour(ContourData),
    SlopeArrow(SlopeArrowData),
    DrainRun(DrainRunData),
    House(HouseData),
    TitleBlock(TitleBlockData),
    Legend(LegendData),
}

impl DomainData {
    /// Returns the width in mm for domain types that have a linear width
    /// (wall, path, step, edging). Used for width-based rendering.
    pub fn width_mm(&self) -> Option<f32> {
        match self {
            Self::Wall(w) => Some(w.width_mm as f32),
            Self::Path(p) => Some(p.width_mm as f32),
            Self::Step(s) => Some(s.width_mm as f32),
            Self::Edging(e) => Some(e.width_mm as f32),
            Self::House(h) => Some(h.wall_thickness_mm as f32),
            _ => None,
        }
    }

    /// Returns the height in mm for domain types that have a vertical dimension.
    /// Used for height-proportional stroke weights in SVG/PDF export.
    pub fn height_mm(&self) -> Option<i32> {
        match self {
            Self::Plant(p) => Some(p.height_mm),
            Self::Wall(w) => Some(w.height_mm),
            Self::Fence(f) => Some(f.height_mm),
            Self::Pergola(p) => Some(p.height_mm),
            Self::RaisedBed(r) => Some(r.height_mm),
            Self::Decking(d) => Some(d.deck_height_mm),
            Self::Step(s) => Some(s.riser_height_mm),
            Self::Edging(e) => Some(e.height_mm),
            Self::WaterFeature(w) => Some(w.depth_mm),
            Self::House(h) => Some(h.height_mm),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Entity — the universal scene node
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: Uuid,
    pub name: String,
    pub geometry: Geometry,
    pub domain: DomainData,
    pub layer_id: Uuid,
    pub children: Vec<Entity>,
    #[serde(skip)]
    pub local_transform: Affine2,
    #[serde(skip)]
    pub world_transform: Affine2,
    pub colour: [u8; 4],
    pub locked: bool,
}

impl Entity {
    pub fn new(name: impl Into<String>, geometry: Geometry, domain: DomainData) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            geometry,
            domain,
            layer_id: Uuid::nil(),
            children: Vec::new(),
            local_transform: Affine2::IDENTITY,
            world_transform: Affine2::IDENTITY,
            colour: [255, 255, 255, 255],
            locked: false,
        }
    }

    /// Create a pure CAD entity with no domain data.
    pub fn cad(name: impl Into<String>, geometry: Geometry) -> Self {
        Self::new(name, geometry, DomainData::None)
    }

    pub fn bounding_box(&self) -> Option<BoundingBox> {
        self.geometry.bounding_box()
    }

    pub fn hit_test(&self, point: Vec2, tolerance: f32) -> bool {
        if self.locked {
            return false;
        }
        self.geometry.hit_test(point, tolerance)
    }

    /// Recursively update world transforms from parent.
    pub fn update_world_transforms(&mut self, parent_transform: Affine2) {
        self.world_transform = parent_transform * self.local_transform;
        for child in &mut self.children {
            child.update_world_transforms(self.world_transform);
        }
    }
}

impl Default for Entity {
    fn default() -> Self {
        Self::cad("", Geometry::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circle_hit_test() {
        let geom = Geometry::Circle(CircleData {
            center: Vec2::new(100.0, 100.0),
            radius: 50.0,
        });
        assert!(geom.hit_test(Vec2::new(100.0, 100.0), 5.0));
        assert!(geom.hit_test(Vec2::new(145.0, 100.0), 5.0));
        assert!(!geom.hit_test(Vec2::new(200.0, 200.0), 5.0));
    }

    #[test]
    fn test_rect_hit_test() {
        let geom = Geometry::Rectangle(RectData {
            center: Vec2::new(50.0, 50.0),
            half_size: Vec2::new(25.0, 15.0),
            rotation: 0.0,
        });
        assert!(geom.hit_test(Vec2::new(50.0, 50.0), 1.0));
        assert!(geom.hit_test(Vec2::new(74.0, 50.0), 1.0));
        assert!(!geom.hit_test(Vec2::new(80.0, 50.0), 1.0));
    }

    #[test]
    fn test_line_hit_test() {
        let geom = Geometry::Line(LineData {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(100.0, 0.0),
        });
        assert!(geom.hit_test(Vec2::new(50.0, 2.0), 5.0));
        assert!(!geom.hit_test(Vec2::new(50.0, 10.0), 5.0));
    }

    #[test]
    fn test_translate_line() {
        let mut geom = Geometry::Line(LineData {
            start: Vec2::new(10.0, 20.0),
            end: Vec2::new(50.0, 60.0),
        });
        geom.translate(Vec2::new(5.0, -10.0));
        if let Geometry::Line(l) = &geom {
            assert!((l.start.x - 15.0).abs() < 0.01);
            assert!((l.start.y - 10.0).abs() < 0.01);
            assert!((l.end.x - 55.0).abs() < 0.01);
            assert!((l.end.y - 50.0).abs() < 0.01);
        } else {
            panic!("expected Line");
        }
    }

    #[test]
    fn test_translate_circle() {
        let mut geom = Geometry::Circle(CircleData {
            center: Vec2::new(100.0, 100.0),
            radius: 50.0,
        });
        geom.translate(Vec2::new(-30.0, 20.0));
        if let Geometry::Circle(c) = &geom {
            assert!((c.center.x - 70.0).abs() < 0.01);
            assert!((c.center.y - 120.0).abs() < 0.01);
            assert!((c.radius - 50.0).abs() < 0.01); // radius unchanged
        } else {
            panic!("expected Circle");
        }
    }

    #[test]
    fn test_rotate_point_90_degrees() {
        let mut geom = Geometry::Point(Vec2::new(10.0, 0.0));
        geom.rotate(Vec2::ZERO, std::f32::consts::FRAC_PI_2);
        if let Geometry::Point(p) = &geom {
            assert!((p.x - 0.0).abs() < 0.01);
            assert!((p.y - 10.0).abs() < 0.01);
        } else {
            panic!("expected Point");
        }
    }

    #[test]
    fn test_rotate_rectangle() {
        let mut geom = Geometry::Rectangle(RectData {
            center: Vec2::new(100.0, 0.0),
            half_size: Vec2::new(25.0, 15.0),
            rotation: 0.0,
        });
        geom.rotate(Vec2::ZERO, std::f32::consts::FRAC_PI_2);
        if let Geometry::Rectangle(r) = &geom {
            assert!((r.center.x - 0.0).abs() < 0.01);
            assert!((r.center.y - 100.0).abs() < 0.01);
            assert!((r.rotation - std::f32::consts::FRAC_PI_2).abs() < 0.01);
        } else {
            panic!("expected Rectangle");
        }
    }

    #[test]
    fn test_scale_circle() {
        let mut geom = Geometry::Circle(CircleData {
            center: Vec2::new(100.0, 100.0),
            radius: 50.0,
        });
        geom.scale(Vec2::ZERO, 2.0);
        if let Geometry::Circle(c) = &geom {
            assert!((c.center.x - 200.0).abs() < 0.01);
            assert!((c.center.y - 200.0).abs() < 0.01);
            assert!((c.radius - 100.0).abs() < 0.01);
        } else {
            panic!("expected Circle");
        }
    }

    #[test]
    fn test_scale_polygon() {
        let mut geom = Geometry::Polygon(PolygonData {
            vertices: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(10.0, 0.0),
                Vec2::new(10.0, 10.0),
            ],
        });
        geom.scale(Vec2::new(5.0, 5.0), 2.0);
        if let Geometry::Polygon(pg) = &geom {
            assert!((pg.vertices[0].x - -5.0).abs() < 0.01);
            assert!((pg.vertices[0].y - -5.0).abs() < 0.01);
            assert!((pg.vertices[1].x - 15.0).abs() < 0.01);
            assert!((pg.vertices[2].y - 15.0).abs() < 0.01);
        } else {
            panic!("expected Polygon");
        }
    }

    #[test]
    fn test_entity_locked_not_hittable() {
        let mut e = Entity::cad(
            "test",
            Geometry::Circle(CircleData {
                center: Vec2::ZERO,
                radius: 50.0,
            }),
        );
        assert!(e.hit_test(Vec2::ZERO, 5.0));
        e.locked = true;
        assert!(!e.hit_test(Vec2::ZERO, 5.0));
    }
}
