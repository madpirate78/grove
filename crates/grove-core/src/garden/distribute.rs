use glam::Vec2;
use rand::Rng;

use crate::geometry::{point_in_polygon, BoundingBox};

/// Distribute points on a regular grid within a polygon boundary.
pub fn grid_distribution(boundary: &[Vec2], spacing: f32) -> Vec<Vec2> {
    let Some(bbox) = BoundingBox::from_points(boundary) else {
        return Vec::new();
    };

    let mut points = Vec::new();
    let mut y = bbox.min.y + spacing / 2.0;
    while y < bbox.max.y {
        let mut x = bbox.min.x + spacing / 2.0;
        while x < bbox.max.x {
            let p = Vec2::new(x, y);
            if point_in_polygon(p, boundary) {
                points.push(p);
            }
            x += spacing;
        }
        y += spacing;
    }
    points
}

/// Distribute points on an offset (hexagonal) grid within a polygon boundary.
pub fn offset_grid_distribution(boundary: &[Vec2], spacing: f32) -> Vec<Vec2> {
    let Some(bbox) = BoundingBox::from_points(boundary) else {
        return Vec::new();
    };

    let row_height = spacing * 0.866; // sqrt(3)/2
    let mut points = Vec::new();
    let mut row = 0;
    let mut y = bbox.min.y + spacing / 2.0;
    while y < bbox.max.y {
        let offset = if row % 2 == 1 { spacing / 2.0 } else { 0.0 };
        let mut x = bbox.min.x + spacing / 2.0 + offset;
        while x < bbox.max.x {
            let p = Vec2::new(x, y);
            if point_in_polygon(p, boundary) {
                points.push(p);
            }
            x += spacing;
        }
        y += row_height;
        row += 1;
    }
    points
}

/// Poisson disk sampling within a polygon boundary (Bridson's algorithm).
pub fn poisson_disk_distribution(boundary: &[Vec2], min_distance: f32, seed: u64) -> Vec<Vec2> {
    let Some(bbox) = BoundingBox::from_points(boundary) else {
        return Vec::new();
    };

    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let cell_size = min_distance / std::f32::consts::SQRT_2;
    let grid_w = ((bbox.size().x / cell_size).ceil() as usize).max(1);
    let grid_h = ((bbox.size().y / cell_size).ceil() as usize).max(1);

    let mut grid: Vec<Option<usize>> = vec![None; grid_w * grid_h];
    let mut points: Vec<Vec2> = Vec::new();
    let mut active: Vec<usize> = Vec::new();

    let k = 30; // candidates per point

    // Find initial point inside boundary
    let mut initial = None;
    for _ in 0..1000 {
        let p = Vec2::new(
            rng.gen_range(bbox.min.x..bbox.max.x),
            rng.gen_range(bbox.min.y..bbox.max.y),
        );
        if point_in_polygon(p, boundary) {
            initial = Some(p);
            break;
        }
    }

    let Some(start) = initial else {
        return Vec::new();
    };

    let grid_index = |p: Vec2| -> (usize, usize) {
        let gx = ((p.x - bbox.min.x) / cell_size) as usize;
        let gy = ((p.y - bbox.min.y) / cell_size) as usize;
        (gx.min(grid_w - 1), gy.min(grid_h - 1))
    };

    let idx = grid_index(start);
    grid[idx.1 * grid_w + idx.0] = Some(0);
    points.push(start);
    active.push(0);

    while !active.is_empty() {
        let active_idx = rng.gen_range(0..active.len());
        let point_idx = active[active_idx];
        let center = points[point_idx];
        let mut found = false;

        for _ in 0..k {
            let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
            let dist: f32 = rng.gen_range(min_distance..min_distance * 2.0);
            let candidate = center + Vec2::new(angle.cos() * dist, angle.sin() * dist);

            if !bbox.contains(candidate) || !point_in_polygon(candidate, boundary) {
                continue;
            }

            let (gx, gy) = grid_index(candidate);
            let mut too_close = false;

            let search_radius = 2usize;
            let y_start = gy.saturating_sub(search_radius);
            let y_end = (gy + search_radius + 1).min(grid_h);
            let x_start = gx.saturating_sub(search_radius);
            let x_end = (gx + search_radius + 1).min(grid_w);

            'outer: for ny in y_start..y_end {
                for nx in x_start..x_end {
                    if let Some(pi) = grid[ny * grid_w + nx] {
                        if (points[pi] - candidate).length() < min_distance {
                            too_close = true;
                            break 'outer;
                        }
                    }
                }
            }

            if !too_close {
                let new_idx = points.len();
                grid[gy * grid_w + gx] = Some(new_idx);
                points.push(candidate);
                active.push(new_idx);
                found = true;
                break;
            }
        }

        if !found {
            active.swap_remove(active_idx);
        }
    }

    points
}

use rand::SeedableRng;

/// Assign species indices to points based on weighted proportions.
///
/// Returns a `Vec<usize>` where each element is a species index (into the
/// `proportions` slice). The distribution is random but respects the given
/// weights, which are normalized internally.
pub fn assign_species_proportional(
    count: usize,
    proportions: &[f32],
    seed: u64,
) -> Vec<usize> {
    if proportions.is_empty() || count == 0 {
        return vec![0; count];
    }

    let total: f32 = proportions.iter().sum();
    if total <= 0.0 {
        return vec![0; count];
    }

    // Build cumulative distribution
    let cumulative: Vec<f32> = proportions
        .iter()
        .scan(0.0f32, |acc, &w| {
            *acc += w / total;
            Some(*acc)
        })
        .collect();

    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    (0..count)
        .map(|_| {
            let r: f32 = rng.gen();
            cumulative
                .iter()
                .position(|&c| r < c)
                .unwrap_or(proportions.len() - 1)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_boundary() -> Vec<Vec2> {
        vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1000.0, 0.0),
            Vec2::new(1000.0, 1000.0),
            Vec2::new(0.0, 1000.0),
        ]
    }

    #[test]
    fn test_grid_distribution() {
        let points = grid_distribution(&test_boundary(), 200.0);
        assert!(!points.is_empty());
        // 1000/200 = 5 rows x 5 cols = 25 points
        assert_eq!(points.len(), 25);
        for p in &points {
            assert!(point_in_polygon(*p, &test_boundary()));
        }
    }

    #[test]
    fn test_offset_grid_distribution() {
        let points = offset_grid_distribution(&test_boundary(), 200.0);
        assert!(!points.is_empty());
        for p in &points {
            assert!(point_in_polygon(*p, &test_boundary()));
        }
    }

    #[test]
    fn test_assign_species_proportional() {
        let assignments = assign_species_proportional(100, &[3.0, 1.0], 42);
        assert_eq!(assignments.len(), 100);

        let count_0 = assignments.iter().filter(|&&x| x == 0).count();
        let count_1 = assignments.iter().filter(|&&x| x == 1).count();
        assert_eq!(count_0 + count_1, 100);
        // With 3:1 ratio, species 0 should dominate (~75%)
        assert!(count_0 > 50, "Expected species 0 to be majority, got {count_0}");
        assert!(count_1 > 10, "Expected species 1 to appear, got {count_1}");

        // Deterministic with same seed
        let again = assign_species_proportional(100, &[3.0, 1.0], 42);
        assert_eq!(assignments, again);
    }

    #[test]
    fn test_poisson_disk() {
        let points = poisson_disk_distribution(&test_boundary(), 100.0, 42);
        assert!(!points.is_empty());
        // Check minimum distance constraint
        for i in 0..points.len() {
            for j in (i + 1)..points.len() {
                let dist = (points[i] - points[j]).length();
                assert!(dist >= 99.0, "Points too close: {dist}"); // small tolerance
            }
        }
    }
}
