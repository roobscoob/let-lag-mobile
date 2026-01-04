use anyhow::{bail, Context, Result};
use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use geo::algorithm::area::Area;
use geojson::GeoJson;
use std::path::Path;

/// Read a clip polygon from a GeoJSON file
/// Accepts Polygon, MultiPolygon, Feature, or FeatureCollection (uses first polygon found)
pub fn read_clip_polygon(path: &Path) -> Result<Polygon<f64>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read clip file: {}", path.display()))?;

    let geojson: GeoJson = content
        .parse()
        .with_context(|| format!("Failed to parse GeoJSON from: {}", path.display()))?;

    extract_polygon_from_geojson(geojson)
        .with_context(|| format!("No valid polygon found in: {}", path.display()))
}

/// Extract a polygon from various GeoJSON structures
fn extract_polygon_from_geojson(geojson: GeoJson) -> Result<Polygon<f64>> {
    match geojson {
        GeoJson::Geometry(geom) => geometry_to_polygon(geom.value),
        GeoJson::Feature(feature) => {
            if let Some(geom) = feature.geometry {
                geometry_to_polygon(geom.value)
            } else {
                bail!("Feature has no geometry")
            }
        }
        GeoJson::FeatureCollection(fc) => {
            for feature in fc.features {
                if let Some(geom) = feature.geometry {
                    if let Ok(poly) = geometry_to_polygon(geom.value) {
                        return Ok(poly);
                    }
                }
            }
            bail!("No polygon found in FeatureCollection")
        }
    }
}

/// Convert a GeoJSON geometry value to a geo Polygon
fn geometry_to_polygon(value: geojson::Value) -> Result<Polygon<f64>> {
    match value {
        geojson::Value::Polygon(rings) => {
            if rings.is_empty() {
                bail!("Polygon has no rings");
            }
            let exterior = coords_to_linestring(&rings[0]);
            let interiors: Vec<LineString<f64>> =
                rings.iter().skip(1).map(|r| coords_to_linestring(r)).collect();
            Ok(Polygon::new(exterior, interiors))
        }
        geojson::Value::MultiPolygon(polygons) => {
            // Return the first polygon
            if let Some(rings) = polygons.first() {
                if rings.is_empty() {
                    bail!("First polygon has no rings");
                }
                let exterior = coords_to_linestring(&rings[0]);
                let interiors: Vec<LineString<f64>> =
                    rings.iter().skip(1).map(|r| coords_to_linestring(r)).collect();
                Ok(Polygon::new(exterior, interiors))
            } else {
                bail!("MultiPolygon is empty")
            }
        }
        _ => bail!("Geometry is not a Polygon or MultiPolygon"),
    }
}

/// Convert GeoJSON coordinate array to LineString
fn coords_to_linestring(coords: &[Vec<f64>]) -> LineString<f64> {
    LineString::new(
        coords
            .iter()
            .map(|c| Coord {
                x: c.get(0).copied().unwrap_or(0.0),
                y: c.get(1).copied().unwrap_or(0.0),
            })
            .collect(),
    )
}

/// An endpoint of an open ring on the clip boundary
#[derive(Debug, Clone)]
struct BoundaryEndpoint {
    /// Position along the boundary (0.0 to 1.0, normalized)
    position: f64,
    /// The projected point on the boundary
    point: Coord<f64>,
    /// Index of the open ring this endpoint belongs to
    ring_idx: usize,
    /// True if this is the START of the ring (coastline enters clip area)
    /// False if this is the END of the ring (coastline exits clip area)
    is_start: bool,
}

/// Calculate position along boundary (0.0 to 1.0)
fn boundary_position(boundary: &LineString<f64>, segment_idx: usize, point: &Coord<f64>) -> f64 {
    let n = boundary.0.len().saturating_sub(1);
    if n == 0 {
        return 0.0;
    }

    // Calculate cumulative lengths
    let mut total_length = 0.0;
    let mut segment_starts = Vec::with_capacity(n);

    for i in 0..n {
        segment_starts.push(total_length);
        let dx = boundary.0[i + 1].x - boundary.0[i].x;
        let dy = boundary.0[i + 1].y - boundary.0[i].y;
        total_length += (dx * dx + dy * dy).sqrt();
    }

    if total_length < 1e-12 {
        return 0.0;
    }

    // Position within segment
    let seg_start = &boundary.0[segment_idx];
    let seg_end = &boundary.0[(segment_idx + 1) % boundary.0.len()];
    let seg_dx = seg_end.x - seg_start.x;
    let seg_dy = seg_end.y - seg_start.y;
    let seg_len = (seg_dx * seg_dx + seg_dy * seg_dy).sqrt();

    let point_dx = point.x - seg_start.x;
    let point_dy = point.y - seg_start.y;
    let point_dist = (point_dx * point_dx + point_dy * point_dy).sqrt();

    let t = if seg_len > 1e-12 { point_dist / seg_len } else { 0.0 };

    let position_along = segment_starts[segment_idx] + t * seg_len;
    position_along / total_length
}

/// Close open rings against a clip boundary by building LAND polygons directly.
///
/// OSM coastlines have water on the RIGHT when walking in the way direction.
/// For an open coastline from START to END:
/// - Land is on the LEFT of the coastline
/// - To build land polygon: walk coastline START->END, then boundary CW from END to START
pub fn close_rings_against_boundary(
    open_rings: Vec<Vec<Coord<f64>>>,
    clip_polygon: &Polygon<f64>,
) -> Vec<Polygon<f64>> {
    if open_rings.is_empty() {
        return Vec::new();
    }

    let boundary = clip_polygon.exterior();
    let mut land_polygons: Vec<Polygon<f64>> = Vec::new();

    for (ring_idx, ring) in open_rings.iter().enumerate() {
        if ring.len() < 2 {
            continue;
        }

        let start = ring.first().unwrap();
        let end = ring.last().unwrap();

        let (start_seg_idx, start_point) = nearest_point_on_ring(start, boundary);
        let (end_seg_idx, end_point) = nearest_point_on_ring(end, boundary);

        // Skip degenerate rings
        let dist = ((start_point.x - end_point.x).powi(2) + (start_point.y - end_point.y).powi(2)).sqrt();
        if dist < 1e-9 {
            log::debug!("Skipping degenerate open ring {} (endpoints project to same point)", ring_idx);
            continue;
        }

        let start_pos = boundary_position(boundary, start_seg_idx, &start_point);
        let end_pos = boundary_position(boundary, end_seg_idx, &end_point);

        // Build land polygon directly:
        // 1. Start at the projected start point on boundary
        // 2. Walk coastline from START to END (land is on LEFT)
        // 3. Walk boundary CW from end_pos to start_pos (staying on land side)
        // 4. Close back to start

        let mut land_coords: Vec<Coord<f64>> = Vec::new();

        // Add start_point (projected onto boundary)
        land_coords.push(start_point);

        // Add coastline points (skip first since we added start_point)
        for coord in ring.iter().skip(1) {
            if let Some(last) = land_coords.last() {
                let d = ((coord.x - last.x).powi(2) + (coord.y - last.y).powi(2)).sqrt();
                if d < 1e-9 {
                    continue;
                }
            }
            land_coords.push(*coord);
        }

        // Add end_point if not already close
        if let Some(last) = land_coords.last() {
            let d = ((end_point.x - last.x).powi(2) + (end_point.y - last.y).powi(2)).sqrt();
            if d >= 1e-9 {
                land_coords.push(end_point);
            }
        }

        // Walk boundary CW from end_pos to start_pos
        // CW on the boundary exterior means decreasing position (with wraparound)
        let boundary_pts = walk_boundary_cw_positions(boundary, end_pos, start_pos);
        land_coords.extend(boundary_pts);

        // Close the polygon
        if let (Some(first), Some(last)) = (land_coords.first(), land_coords.last()) {
            if (first.x - last.x).abs() > 1e-9 || (first.y - last.y).abs() > 1e-9 {
                land_coords.push(*first);
            }
        }

        if land_coords.len() >= 4 {
            let poly = Polygon::new(LineString::new(land_coords), vec![]);
            let area = poly.unsigned_area();
            if area > 1e-10 {
                log::debug!("Created land polygon {} with {} points, area {:.6}",
                           ring_idx, poly.exterior().0.len(), area);
                land_polygons.push(poly);
            } else {
                log::debug!("Skipping land polygon {} with negligible area {:.10}", ring_idx, area);
            }
        }
    }

    log::debug!("Created {} land polygons from {} open rings", land_polygons.len(), open_rings.len());
    land_polygons
}

/// Walk the boundary CW (decreasing position) from start_pos to end_pos
fn walk_boundary_cw_positions(
    boundary: &LineString<f64>,
    start_pos: f64,
    end_pos: f64,
) -> Vec<Coord<f64>> {
    let n = boundary.0.len().saturating_sub(1);
    if n == 0 {
        return vec![];
    }

    // Calculate cumulative positions for each vertex
    let mut total_length = 0.0;
    let mut vertex_positions = Vec::with_capacity(n + 1);
    vertex_positions.push(0.0);

    for i in 0..n {
        let dx = boundary.0[i + 1].x - boundary.0[i].x;
        let dy = boundary.0[i + 1].y - boundary.0[i].y;
        total_length += (dx * dx + dy * dy).sqrt();
        vertex_positions.push(total_length);
    }

    if total_length < 1e-12 {
        return vec![];
    }

    // Normalize to 0-1
    for pos in vertex_positions.iter_mut() {
        *pos /= total_length;
    }

    // Collect vertices in our CW walk
    // CW means decreasing position. If end_pos > start_pos, we wrap through 0.
    let mut vertices_with_pos: Vec<(f64, Coord<f64>)> = Vec::new();

    for i in 0..n {
        let v_pos = vertex_positions[i + 1];
        let coord = boundary.0[(i + 1) % boundary.0.len()];

        // Check if this vertex is in our CW path from start_pos to end_pos
        let in_range = if end_pos <= start_pos {
            // Normal case: path goes from start_pos down to end_pos
            // Include vertices where end_pos < v_pos < start_pos
            v_pos > end_pos && v_pos < start_pos
        } else {
            // Wrap case: path goes from start_pos down through 0 to end_pos
            // Include vertices where v_pos < start_pos OR v_pos > end_pos
            v_pos < start_pos || v_pos > end_pos
        };

        if in_range {
            // For sorting: convert position to "distance from start going CW"
            let sort_key = if v_pos <= start_pos {
                start_pos - v_pos
            } else {
                start_pos + (1.0 - v_pos)
            };
            vertices_with_pos.push((sort_key, coord));
        }
    }

    // Sort by distance from start (ascending = CW order)
    vertices_with_pos.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    vertices_with_pos.into_iter().map(|(_, coord)| coord).collect()
}

/// Walk the boundary CCW from start_pos to end_pos
fn walk_boundary_ccw_between(
    boundary: &LineString<f64>,
    start_pos: f64,
    end_pos: f64,
) -> Vec<Coord<f64>> {
    let n = boundary.0.len().saturating_sub(1);
    if n == 0 {
        return vec![];
    }

    // Calculate cumulative positions for each vertex
    let mut total_length = 0.0;
    let mut vertex_positions = Vec::with_capacity(n + 1);
    vertex_positions.push(0.0);

    for i in 0..n {
        let dx = boundary.0[i + 1].x - boundary.0[i].x;
        let dy = boundary.0[i + 1].y - boundary.0[i].y;
        total_length += (dx * dx + dy * dy).sqrt();
        vertex_positions.push(total_length);
    }

    if total_length < 1e-12 {
        return vec![];
    }

    // Normalize to 0-1
    for pos in vertex_positions.iter_mut() {
        *pos /= total_length;
    }

    let mut result = Vec::new();

    // CCW means increasing position
    // If end_pos < start_pos, we wrap around
    let effective_end = if end_pos < start_pos { end_pos + 1.0 } else { end_pos };

    for i in 0..n {
        let mut v_pos = vertex_positions[i + 1];

        // Handle wraparound
        if v_pos < start_pos && effective_end > 1.0 {
            v_pos += 1.0;
        }

        if v_pos > start_pos && v_pos < effective_end {
            result.push(boundary.0[(i + 1) % boundary.0.len()]);
        }
    }

    result
}

/// Find the previous START point (walking CW/backwards from end_pos)
/// Returns (ring_idx, start_position)
fn find_prev_start(
    endpoints: &[BoundaryEndpoint],
    end_pos: f64,
    used_rings: &[bool],
    first_ring_idx: usize,
) -> Option<(usize, f64)> {
    // Find START points, prefer ones with position < end_pos (walking backwards)
    // If none found, wrap around to largest position

    let mut best_before: Option<(usize, f64)> = None; // largest pos < end_pos
    let mut best_after: Option<(usize, f64)> = None;  // largest pos overall (for wrap)

    for ep in endpoints {
        if !ep.is_start {
            continue;
        }
        // Allow returning to the first ring to close the polygon
        if used_rings[ep.ring_idx] && ep.ring_idx != first_ring_idx {
            continue;
        }

        if ep.position < end_pos {
            match best_before {
                None => best_before = Some((ep.ring_idx, ep.position)),
                Some((_, best_pos)) if ep.position > best_pos => {
                    best_before = Some((ep.ring_idx, ep.position));
                }
                _ => {}
            }
        }

        match best_after {
            None => best_after = Some((ep.ring_idx, ep.position)),
            Some((_, best_pos)) if ep.position > best_pos => {
                best_after = Some((ep.ring_idx, ep.position));
            }
            _ => {}
        }
    }

    // Prefer the one before (no wrap needed), otherwise use the one after (wrap around)
    best_before.or(best_after)
}

/// Walk the boundary CW (backwards) from start_pos to end_pos
fn walk_boundary_cw_between(
    boundary: &LineString<f64>,
    start_pos: f64,
    end_pos: f64,
) -> Vec<Coord<f64>> {
    let n = boundary.0.len().saturating_sub(1);
    if n == 0 {
        return vec![];
    }

    // Calculate cumulative positions for each vertex
    let mut total_length = 0.0;
    let mut vertex_positions = Vec::with_capacity(n + 1);
    vertex_positions.push(0.0);

    for i in 0..n {
        let dx = boundary.0[i + 1].x - boundary.0[i].x;
        let dy = boundary.0[i + 1].y - boundary.0[i].y;
        total_length += (dx * dx + dy * dy).sqrt();
        vertex_positions.push(total_length);
    }

    if total_length < 1e-12 {
        return vec![];
    }

    // Normalize to 0-1
    for pos in vertex_positions.iter_mut() {
        *pos /= total_length;
    }

    let mut result = Vec::new();

    // Walking CW means decreasing position
    // Handle wraparound: if end_pos > start_pos, we wrap around through 0
    let effective_end = if end_pos > start_pos { end_pos - 1.0 } else { end_pos };

    // Collect vertices in the range (end_pos, start_pos) going backwards
    // Then reverse them at the end
    let mut vertices_to_add = Vec::new();

    for i in 0..n {
        let v_pos = vertex_positions[i + 1];

        // Check if this vertex is in our CW path
        // CW from start_pos to end_pos means positions in (end_pos, start_pos)
        let in_range = if end_pos < start_pos {
            // Normal case: end_pos < v_pos < start_pos
            v_pos > end_pos && v_pos < start_pos
        } else {
            // Wrap case: v_pos > end_pos (before wrap) or v_pos < start_pos (after wrap)
            v_pos > end_pos || v_pos < start_pos
        };

        if in_range {
            vertices_to_add.push((v_pos, boundary.0[(i + 1) % boundary.0.len()]));
        }
    }

    // Sort by position descending (CW order) and extract coords
    vertices_to_add.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    result = vertices_to_add.into_iter().map(|(_, coord)| coord).collect();

    result
}

/// Walk the boundary CCW from start_pos to end_pos, returning intermediate vertices
fn walk_boundary_between(
    boundary: &LineString<f64>,
    start_pos: f64,
    end_pos: f64,
) -> Vec<Coord<f64>> {
    let n = boundary.0.len().saturating_sub(1);
    if n == 0 {
        return vec![];
    }

    // Calculate cumulative positions for each vertex
    let mut total_length = 0.0;
    let mut vertex_positions = Vec::with_capacity(n + 1);
    vertex_positions.push(0.0);

    for i in 0..n {
        let dx = boundary.0[i + 1].x - boundary.0[i].x;
        let dy = boundary.0[i + 1].y - boundary.0[i].y;
        total_length += (dx * dx + dy * dy).sqrt();
        vertex_positions.push(total_length);
    }

    if total_length < 1e-12 {
        return vec![];
    }

    // Normalize to 0-1
    for pos in vertex_positions.iter_mut() {
        *pos /= total_length;
    }

    let mut result = Vec::new();

    // Handle wraparound: if end_pos < start_pos, we wrap around
    let effective_end = if end_pos < start_pos { end_pos + 1.0 } else { end_pos };

    // Add vertices between start_pos and end_pos
    for i in 0..n {
        let mut v_pos = vertex_positions[i + 1];

        // Handle wraparound
        if v_pos < start_pos && effective_end > 1.0 {
            v_pos += 1.0;
        }

        if v_pos > start_pos && v_pos < effective_end {
            result.push(boundary.0[(i + 1) % boundary.0.len()]);
        }
    }

    result
}

/// Find the nearest point on a ring boundary to a given point
/// Returns (segment_index, nearest_point)
fn nearest_point_on_ring(point: &Coord<f64>, ring: &LineString<f64>) -> (usize, Coord<f64>) {
    let mut best_idx = 0;
    let mut best_point = ring.0[0];
    let mut best_dist = f64::MAX;

    for i in 0..ring.0.len().saturating_sub(1) {
        let (nearest, dist) = nearest_point_on_segment(point, &ring.0[i], &ring.0[i + 1]);
        if dist < best_dist {
            best_dist = dist;
            best_point = nearest;
            best_idx = i;
        }
    }

    (best_idx, best_point)
}

/// Find nearest point on a line segment to a given point
fn nearest_point_on_segment(point: &Coord<f64>, a: &Coord<f64>, b: &Coord<f64>) -> (Coord<f64>, f64) {
    let ab = Coord {
        x: b.x - a.x,
        y: b.y - a.y,
    };
    let ap = Coord {
        x: point.x - a.x,
        y: point.y - a.y,
    };

    let ab_len_sq = ab.x * ab.x + ab.y * ab.y;
    if ab_len_sq < 1e-12 {
        // Degenerate segment
        let dist = ((point.x - a.x).powi(2) + (point.y - a.y).powi(2)).sqrt();
        return (*a, dist);
    }

    let t = (ap.x * ab.x + ap.y * ab.y) / ab_len_sq;
    let t_clamped = t.clamp(0.0, 1.0);

    let nearest = Coord {
        x: a.x + t_clamped * ab.x,
        y: a.y + t_clamped * ab.y,
    };

    let dist = ((point.x - nearest.x).powi(2) + (point.y - nearest.y).powi(2)).sqrt();
    (nearest, dist)
}

/// Walk along boundary counter-clockwise from start_idx to end_idx
/// Returns the boundary vertices (excluding the actual nearest points, which are added separately)
fn walk_boundary_ccw(
    ring: &LineString<f64>,
    start_idx: usize,
    end_idx: usize,
) -> Vec<Coord<f64>> {
    let n = ring.0.len().saturating_sub(1); // Exclude closing point
    if n == 0 {
        return vec![];
    }

    let mut result = Vec::new();

    // Walk counter-clockwise (decreasing indices, wrapping around)
    let mut idx = start_idx;
    loop {
        // Move to next vertex counter-clockwise
        idx = if idx == 0 { n - 1 } else { idx - 1 };

        if idx == end_idx {
            break;
        }

        result.push(ring.0[idx]);

        // Safety check to avoid infinite loop
        if result.len() > n + 1 {
            break;
        }
    }

    result
}

/// Walk along boundary clockwise from start_idx to end_idx
fn walk_boundary_cw(
    ring: &LineString<f64>,
    start_idx: usize,
    end_idx: usize,
) -> Vec<Coord<f64>> {
    let n = ring.0.len().saturating_sub(1); // Exclude closing point
    if n == 0 {
        return vec![];
    }

    let mut result = Vec::new();

    // Walk clockwise (increasing indices, wrapping around)
    let mut idx = start_idx;
    loop {
        // Move to next vertex clockwise
        idx = (idx + 1) % n;

        if idx == end_idx {
            break;
        }

        result.push(ring.0[idx]);

        // Safety check to avoid infinite loop
        if result.len() > n + 1 {
            break;
        }
    }

    result
}

/// Clip the final landmass to the clip polygon boundary
pub fn clip_to_boundary(landmass: MultiPolygon<f64>, clip_polygon: &Polygon<f64>) -> MultiPolygon<f64> {
    let clip_mp = MultiPolygon::new(vec![clip_polygon.clone()]);
    let landmass_clone = landmass.clone();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        landmass_clone.intersection(&clip_mp)
    }));

    match result {
        Ok(mp) => mp,
        Err(_) => {
            log::warn!("Clip intersection panicked, returning unclipped landmass");
            landmass
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rect(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Polygon<f64> {
        Polygon::new(
            LineString::new(vec![
                Coord { x: min_x, y: min_y },
                Coord { x: max_x, y: min_y },
                Coord { x: max_x, y: max_y },
                Coord { x: min_x, y: max_y },
                Coord { x: min_x, y: min_y },
            ]),
            vec![],
        )
    }

    #[test]
    fn test_nearest_point_on_segment() {
        let a = Coord { x: 0.0, y: 0.0 };
        let b = Coord { x: 10.0, y: 0.0 };
        let point = Coord { x: 5.0, y: 3.0 };

        let (nearest, dist) = nearest_point_on_segment(&point, &a, &b);
        assert!((nearest.x - 5.0).abs() < 1e-9);
        assert!((nearest.y - 0.0).abs() < 1e-9);
        assert!((dist - 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_nearest_point_at_endpoint() {
        let a = Coord { x: 0.0, y: 0.0 };
        let b = Coord { x: 10.0, y: 0.0 };
        let point = Coord { x: -5.0, y: 0.0 };

        let (nearest, _dist) = nearest_point_on_segment(&point, &a, &b);
        assert!((nearest.x - 0.0).abs() < 1e-9);
        assert!((nearest.y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_close_simple_ring() {
        let clip = make_rect(0.0, 0.0, 10.0, 10.0);

        // Open ring that enters from left edge and exits from bottom edge
        let open_ring = vec![
            Coord { x: 0.0, y: 5.0 },  // On left edge
            Coord { x: 5.0, y: 5.0 },  // Inside
            Coord { x: 5.0, y: 0.0 },  // On bottom edge
        ];

        let result = close_rings_against_boundary(vec![open_ring], &clip);
        assert_eq!(result.len(), 1);

        // The closed polygon should have at least 4 points
        assert!(result[0].exterior().0.len() >= 4);
    }
}
