use geo::algorithm::area::Area;
use geo::{BooleanOps, LineString, MultiPolygon, Polygon};
use std::panic;

/// Check if a polygon is valid (basic checks)
pub fn is_valid_polygon(poly: &Polygon<f64>) -> bool {
    let exterior = poly.exterior();

    // Must have at least 4 points (3 unique + closing point)
    if exterior.0.len() < 4 {
        return false;
    }

    // Must be closed
    if let (Some(first), Some(last)) = (exterior.0.first(), exterior.0.last()) {
        if (first.x - last.x).abs() > 1e-9 || (first.y - last.y).abs() > 1e-9 {
            return false;
        }
    } else {
        return false;
    }

    // Area should be non-zero
    let area = poly.unsigned_area();
    if area < 1e-12 {
        return false;
    }

    true
}

/// Check if a polygon has counter-clockwise winding (exterior should be CCW for GeoJSON)
pub fn is_ccw(ring: &LineString<f64>) -> bool {
    // Shoelace formula - positive area means CCW
    let mut sum = 0.0;
    for i in 0..ring.0.len().saturating_sub(1) {
        let p1 = &ring.0[i];
        let p2 = &ring.0[i + 1];
        sum += (p2.x - p1.x) * (p2.y + p1.y);
    }
    sum < 0.0
}

/// Ensure polygon has correct winding order for GeoJSON
/// (exterior CCW, holes CW)
pub fn fix_winding(poly: Polygon<f64>) -> Polygon<f64> {
    let exterior = poly.exterior();
    let interiors = poly.interiors();

    // Fix exterior (should be CCW, but geo uses CW for exterior)
    // Actually geo's convention matches GeoJSON - exterior is CCW
    let fixed_exterior = if !is_ccw(exterior) {
        let mut coords = exterior.0.clone();
        coords.reverse();
        LineString::new(coords)
    } else {
        exterior.clone()
    };

    // Fix interiors (holes should be CW)
    let fixed_interiors: Vec<LineString<f64>> = interiors
        .iter()
        .map(|interior| {
            if is_ccw(interior) {
                let mut coords = interior.0.clone();
                coords.reverse();
                LineString::new(coords)
            } else {
                interior.clone()
            }
        })
        .collect();

    Polygon::new(fixed_exterior, fixed_interiors)
}

/// Attempt to repair an invalid polygon
pub fn repair_polygon(poly: Polygon<f64>) -> Option<MultiPolygon<f64>> {
    if !is_valid_polygon(&poly) {
        log::debug!(
            "Skipping invalid polygon with {} points",
            poly.exterior().0.len()
        );
        return None;
    }

    // Fix winding order
    let fixed = fix_winding(poly);

    Some(MultiPolygon::new(vec![fixed]))
}

/// Filter valid polygons from a collection
pub fn filter_valid_polygons(polygons: Vec<Polygon<f64>>) -> Vec<Polygon<f64>> {
    let initial_count = polygons.len();
    let valid: Vec<Polygon<f64>> = polygons
        .into_iter()
        .filter(|p| is_valid_polygon(p))
        .map(fix_winding)
        .collect();

    let filtered = initial_count - valid.len();
    if filtered > 0 {
        log::warn!("Filtered {} invalid polygons", filtered);
    }

    valid
}

/// Attempt to union two multipolygons, catching any panics
fn try_union(a: &MultiPolygon<f64>, b: &MultiPolygon<f64>) -> Option<MultiPolygon<f64>> {
    let a_clone = a.clone();
    let b_clone = b.clone();

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        a_clone.union(&b_clone)
    }));

    match result {
        Ok(mp) => Some(mp),
        Err(_) => {
            log::warn!("Union operation panicked, skipping problematic geometry");
            None
        }
    }
}

/// Union all polygons into a single MultiPolygon
/// Uses pairwise union for better performance: union pairs, then union those results, etc.
/// This keeps geometry sizes balanced rather than growing a single large geometry sequentially.
pub fn union_all(polygons: Vec<Polygon<f64>>) -> MultiPolygon<f64> {
    log::debug!("Unioning {} polygons", polygons.len());

    if polygons.is_empty() {
        return MultiPolygon::new(vec![]);
    }

    if polygons.len() == 1 {
        return MultiPolygon::new(polygons);
    }

    // Convert all polygons to MultiPolygons for uniform handling
    let mut current: Vec<MultiPolygon<f64>> = polygons
        .into_iter()
        .map(|p| MultiPolygon::new(vec![p]))
        .collect();

    let mut round = 0;
    while current.len() > 1 {
        round += 1;
        log::debug!(
            "  Union round {}: {} geometries -> {}",
            round,
            current.len(),
            (current.len() + 1) / 2
        );

        let mut next = Vec::with_capacity((current.len() + 1) / 2);

        // Union pairs
        let mut i = 0;
        while i + 1 < current.len() {
            match try_union(&current[i], &current[i + 1]) {
                Some(unioned) => next.push(unioned),
                None => {
                    // If union fails, try to keep at least one of them
                    // Prefer the one with more polygons (likely more complete)
                    if current[i].0.len() >= current[i + 1].0.len() {
                        next.push(current[i].clone());
                    } else {
                        next.push(current[i + 1].clone());
                    }
                }
            }
            i += 2;
        }

        // If odd number, carry the last one forward
        if i < current.len() {
            next.push(current.pop().unwrap());
        }

        current = next;
    }

    current.pop().unwrap_or_else(|| MultiPolygon::new(vec![]))
}

/// Compute landmass = land - water
pub fn compute_landmass(land: MultiPolygon<f64>, water: MultiPolygon<f64>) -> MultiPolygon<f64> {
    if water.0.is_empty() {
        log::debug!("No water bodies to subtract");
        return land;
    }

    log::debug!(
        "Computing landmass: {} land polygons - {} water polygons",
        land.0.len(),
        water.0.len()
    );

    let land_clone = land.clone();
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        land_clone.difference(&water)
    }));

    match result {
        Ok(mp) => {
            log::debug!("Result: {} polygons", mp.0.len());
            mp
        }
        Err(_) => {
            log::warn!("Water subtraction panicked, returning land without water subtraction");
            land
        }
    }
}

/// Calculate total area of a MultiPolygon in square meters (approximate)
pub fn calculate_area_sqm(mp: &MultiPolygon<f64>) -> f64 {
    // Use unsigned area (in square degrees) and convert to approximate square meters
    // At NYC latitude (~40.7), 1 degree latitude ≈ 111km, 1 degree longitude ≈ 85km
    let area_deg2 = mp.unsigned_area();

    // Rough conversion: 1 deg^2 ≈ 111km * 85km ≈ 9435 km^2 at NYC latitude
    // But this is very approximate; for accurate results use geodesic area
    let lat: f64 = 40.7; // NYC approximate latitude
    let m_per_deg_lat = 111_320.0;
    let m_per_deg_lon = 111_320.0 * lat.to_radians().cos();

    area_deg2 * m_per_deg_lat * m_per_deg_lon
}

/// Statistics about geometry processing
#[derive(Default)]
pub struct GeometryStats {
    pub land_polygon_count: usize,
    pub water_polygon_count: usize,
    pub landmass_polygon_count: usize,
    pub land_area_sqm: f64,
    pub water_area_sqm: f64,
    pub landmass_area_sqm: f64,
    pub invalid_polygons_skipped: usize,
}

impl GeometryStats {
    pub fn log_summary(&self) {
        log::info!("=== Geometry Statistics ===");
        log::info!("Land polygons: {}", self.land_polygon_count);
        log::info!("Water polygons: {}", self.water_polygon_count);
        log::info!(
            "Landmass polygons (result): {}",
            self.landmass_polygon_count
        );
        log::info!("Land area: {:.2} km²", self.land_area_sqm / 1_000_000.0);
        log::info!("Water area: {:.2} km²", self.water_area_sqm / 1_000_000.0);
        log::info!(
            "Landmass area (result): {:.2} km²",
            self.landmass_area_sqm / 1_000_000.0
        );
        if self.invalid_polygons_skipped > 0 {
            log::warn!(
                "Invalid polygons skipped: {}",
                self.invalid_polygons_skipped
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Coord;

    fn make_square(x: f64, y: f64, size: f64) -> Polygon<f64> {
        Polygon::new(
            LineString::new(vec![
                Coord { x, y },
                Coord { x: x + size, y },
                Coord {
                    x: x + size,
                    y: y + size,
                },
                Coord { x, y: y + size },
                Coord { x, y },
            ]),
            vec![],
        )
    }

    #[test]
    fn test_is_valid_polygon() {
        let valid = make_square(0.0, 0.0, 1.0);
        assert!(is_valid_polygon(&valid));

        // Too few points
        let invalid = Polygon::new(
            LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 1.0, y: 0.0 },
                Coord { x: 0.0, y: 0.0 },
            ]),
            vec![],
        );
        assert!(!is_valid_polygon(&invalid));
    }

    #[test]
    fn test_water_subtraction() {
        // Large land square
        let land = make_square(0.0, 0.0, 10.0);

        // Smaller water square inside
        let water = make_square(2.0, 2.0, 4.0);

        let land_mp = MultiPolygon::new(vec![land]);
        let water_mp = MultiPolygon::new(vec![water]);

        let result = compute_landmass(land_mp, water_mp);

        // Result should have one polygon with a hole
        assert_eq!(result.0.len(), 1);
        assert_eq!(result.0[0].interiors().len(), 1);
    }

    #[test]
    fn test_water_completely_outside() {
        let land = make_square(0.0, 0.0, 5.0);
        let water = make_square(10.0, 10.0, 2.0); // Completely outside

        let land_mp = MultiPolygon::new(vec![land.clone()]);
        let water_mp = MultiPolygon::new(vec![water]);

        let result = compute_landmass(land_mp, water_mp);

        // Result should be unchanged
        assert_eq!(result.0.len(), 1);
        assert!(result.0[0].interiors().is_empty());
    }

    #[test]
    fn test_union_all() {
        let p1 = make_square(0.0, 0.0, 2.0);
        let p2 = make_square(1.0, 0.0, 2.0); // Overlapping

        let result = union_all(vec![p1, p2]);

        // Should merge into one polygon
        assert_eq!(result.0.len(), 1);
    }
}
