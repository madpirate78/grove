use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Types of snap points the system can detect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapType {
    Endpoint,
    Midpoint,
    Center,
    Grid,
    Intersection,
    Perpendicular,
    Nearest,
}

/// A snap result: the snapped position and what type it is.
#[derive(Debug, Clone, Copy)]
pub struct SnapResult {
    pub position: Vec2,
    pub snap_type: SnapType,
    pub distance: f32,
}

/// Configuration for the snap system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapConfig {
    pub enabled: bool,
    pub grid_enabled: bool,
    pub endpoint_enabled: bool,
    pub midpoint_enabled: bool,
    pub center_enabled: bool,
    pub intersection_enabled: bool,
    pub perpendicular_enabled: bool,
    pub snap_distance: f32,
    pub grid_spacing: f32,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            grid_enabled: true,
            endpoint_enabled: true,
            midpoint_enabled: true,
            center_enabled: true,
            intersection_enabled: true,
            perpendicular_enabled: true,
            snap_distance: 10.0,
            grid_spacing: 100.0, // 100mm grid
        }
    }
}

/// Snap a point to the nearest grid intersection.
pub fn snap_to_grid(point: Vec2, grid_spacing: f32) -> Vec2 {
    Vec2::new(
        (point.x / grid_spacing).round() * grid_spacing,
        (point.y / grid_spacing).round() * grid_spacing,
    )
}

/// Find the best snap from a list of candidates.
pub fn best_snap(candidates: &[SnapResult], max_distance: f32) -> Option<SnapResult> {
    candidates
        .iter()
        .filter(|s| s.distance <= max_distance)
        .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal))
        .copied()
}

/// A center point for center snapping.
#[derive(Debug, Clone, Copy)]
pub struct CenterPoint {
    pub position: Vec2,
}

/// Collect snap points from segments, centers, and grid.
pub fn snap_points_from_segments(
    cursor: Vec2,
    segments: &[(Vec2, Vec2)],
    config: &SnapConfig,
) -> Vec<SnapResult> {
    snap_points(cursor, segments, &[], config)
}

/// Full snap calculation with segments, centers, and intersection detection.
pub fn snap_points(
    cursor: Vec2,
    segments: &[(Vec2, Vec2)],
    centers: &[CenterPoint],
    config: &SnapConfig,
) -> Vec<SnapResult> {
    let mut results = Vec::new();

    for &(a, b) in segments {
        if config.endpoint_enabled {
            results.push(SnapResult {
                position: a,
                snap_type: SnapType::Endpoint,
                distance: (cursor - a).length(),
            });
            results.push(SnapResult {
                position: b,
                snap_type: SnapType::Endpoint,
                distance: (cursor - b).length(),
            });
        }
        if config.midpoint_enabled {
            let mid = (a + b) * 0.5;
            results.push(SnapResult {
                position: mid,
                snap_type: SnapType::Midpoint,
                distance: (cursor - mid).length(),
            });
        }
        if config.perpendicular_enabled {
            // Nearest point on segment (perpendicular foot)
            let foot = nearest_point_on_segment(cursor, a, b);
            // Only offer if it's not at an endpoint (those are already covered)
            let at_endpoint = (foot - a).length() < 0.01 || (foot - b).length() < 0.01;
            if !at_endpoint {
                results.push(SnapResult {
                    position: foot,
                    snap_type: SnapType::Perpendicular,
                    distance: (cursor - foot).length(),
                });
            }
        }
    }

    // Center snaps (circles, arcs)
    if config.center_enabled {
        for c in centers {
            results.push(SnapResult {
                position: c.position,
                snap_type: SnapType::Center,
                distance: (cursor - c.position).length(),
            });
        }
    }

    // Intersection snaps
    if config.intersection_enabled && segments.len() >= 2 {
        for i in 0..segments.len() {
            for j in (i + 1)..segments.len() {
                if let Some(pt) = segment_intersection(
                    segments[i].0, segments[i].1,
                    segments[j].0, segments[j].1,
                ) {
                    results.push(SnapResult {
                        position: pt,
                        snap_type: SnapType::Intersection,
                        distance: (cursor - pt).length(),
                    });
                }
            }
        }
    }

    if config.grid_enabled {
        let grid_pt = snap_to_grid(cursor, config.grid_spacing);
        results.push(SnapResult {
            position: grid_pt,
            snap_type: SnapType::Grid,
            distance: (cursor - grid_pt).length(),
        });
    }

    results
}

/// Nearest point on a line segment to a given point.
fn nearest_point_on_segment(point: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < f32::EPSILON {
        return a;
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    a + ab * t
}

/// Find intersection point of two line segments, if they intersect.
fn segment_intersection(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> Option<Vec2> {
    let d1 = a2 - a1;
    let d2 = b2 - b1;
    let cross = d1.x * d2.y - d1.y * d2.x;
    if cross.abs() < f32::EPSILON {
        return None; // Parallel
    }
    let d = b1 - a1;
    let t = (d.x * d2.y - d.y * d2.x) / cross;
    let u = (d.x * d1.y - d.y * d1.x) / cross;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(a1 + d1 * t)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_to_grid() {
        let p = snap_to_grid(Vec2::new(123.0, 267.0), 100.0);
        assert_eq!(p, Vec2::new(100.0, 300.0));
    }

    #[test]
    fn test_segment_intersection() {
        let pt = super::segment_intersection(
            Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0),
            Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0),
        );
        assert!(pt.is_some());
        let p = pt.unwrap();
        assert!((p.x - 5.0).abs() < 0.01);
        assert!((p.y - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_no_intersection_parallel() {
        let pt = super::segment_intersection(
            Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0),
            Vec2::new(0.0, 5.0), Vec2::new(10.0, 5.0),
        );
        assert!(pt.is_none());
    }

    #[test]
    fn test_nearest_point_on_segment() {
        let p = super::nearest_point_on_segment(
            Vec2::new(5.0, 5.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
        );
        assert!((p.x - 5.0).abs() < 0.01);
        assert!((p.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_center_snap() {
        let centers = vec![CenterPoint { position: Vec2::new(100.0, 100.0) }];
        let config = SnapConfig { center_enabled: true, ..Default::default() };
        let results = snap_points(Vec2::new(102.0, 100.0), &[], &centers, &config);
        let center_snaps: Vec<_> = results.iter()
            .filter(|r| r.snap_type == SnapType::Center)
            .collect();
        assert_eq!(center_snaps.len(), 1);
        assert!((center_snaps[0].distance - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_best_snap() {
        let candidates = vec![
            SnapResult {
                position: Vec2::new(0.0, 0.0),
                snap_type: SnapType::Grid,
                distance: 15.0,
            },
            SnapResult {
                position: Vec2::new(5.0, 0.0),
                snap_type: SnapType::Endpoint,
                distance: 5.0,
            },
        ];
        let best = best_snap(&candidates, 10.0).unwrap();
        assert_eq!(best.snap_type, SnapType::Endpoint);
    }
}
