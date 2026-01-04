use crate::pbf::{MemberRole, MultipolygonRelation, NodeCache, Way, WayRef};
use geo::{Coord, LineString, MultiPolygon, Polygon};
use geo::algorithm::contains::Contains;
use hashbrown::HashMap;

/// Key for coordinate-based endpoint matching
/// Uses fixed-point representation to avoid floating point issues
fn coord_key(coord: &Coord<f64>) -> (i64, i64) {
    // 7 decimal places precision (sub-meter)
    let x = (coord.x * 10_000_000.0).round() as i64;
    let y = (coord.y * 10_000_000.0).round() as i64;
    (x, y)
}

/// Result of ring assembly
pub struct AssemblyResult {
    pub closed_rings: Vec<LineString<f64>>,
    pub open_rings: Vec<Vec<Coord<f64>>>,
}

/// Assemble ways into closed rings
pub fn assemble_rings(ways: Vec<Way>) -> AssemblyResult {
    if ways.is_empty() {
        return AssemblyResult {
            closed_rings: Vec::new(),
            open_rings: Vec::new(),
        };
    }

    // Build endpoint indices for efficient matching
    // Maps coordinate key -> list of way indices that start/end at that coord
    let mut start_index: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let mut end_index: HashMap<(i64, i64), Vec<usize>> = HashMap::new();

    for (idx, way) in ways.iter().enumerate() {
        if way.coords.is_empty() {
            continue;
        }
        let start = coord_key(way.coords.first().unwrap());
        let end = coord_key(way.coords.last().unwrap());
        start_index.entry(start).or_default().push(idx);
        end_index.entry(end).or_default().push(idx);
    }

    let mut closed_rings = Vec::new();
    let mut open_rings = Vec::new();
    let mut used = vec![false; ways.len()];

    for seed_idx in 0..ways.len() {
        if used[seed_idx] || ways[seed_idx].coords.is_empty() {
            continue;
        }

        // Start a new ring with this way
        let mut ring_coords: Vec<Coord<f64>> = ways[seed_idx].coords.clone();
        used[seed_idx] = true;

        // Track the original start for closure detection
        let ring_start = coord_key(ring_coords.first().unwrap());

        // Try to extend the ring
        loop {
            let current_end = coord_key(ring_coords.last().unwrap());

            // Check if ring is closed
            if ring_coords.len() >= 4 && current_end == ring_start {
                closed_rings.push(LineString::new(ring_coords.clone()));
                break;
            }

            // Find a way that connects to the end of our ring
            let mut found = false;

            // Look for a way that starts where we end
            if let Some(candidates) = start_index.get(&current_end) {
                for &next_idx in candidates {
                    if !used[next_idx] && !ways[next_idx].coords.is_empty() {
                        // Append way (skip first coord as it's the connection point)
                        ring_coords.extend(ways[next_idx].coords.iter().skip(1).copied());
                        used[next_idx] = true;
                        found = true;
                        break;
                    }
                }
            }

            if found {
                continue;
            }

            // Look for a way that ends where we end (need to reverse it)
            if let Some(candidates) = end_index.get(&current_end) {
                for &next_idx in candidates {
                    if !used[next_idx] && !ways[next_idx].coords.is_empty() {
                        // Append reversed way (skip last coord as it's the connection point)
                        let reversed: Vec<Coord<f64>> =
                            ways[next_idx].coords.iter().rev().skip(1).copied().collect();
                        ring_coords.extend(reversed);
                        used[next_idx] = true;
                        found = true;
                        break;
                    }
                }
            }

            if !found {
                // Could not extend - this is an open ring
                open_rings.push(ring_coords);
                break;
            }
        }
    }

    log::debug!(
        "Ring assembly: {} closed, {} open",
        closed_rings.len(),
        open_rings.len()
    );

    AssemblyResult {
        closed_rings,
        open_rings,
    }
}

/// Convert closed rings to simple polygons (no holes)
pub fn rings_to_simple_polygons(rings: Vec<LineString<f64>>) -> Vec<Polygon<f64>> {
    rings
        .into_iter()
        .filter(|ring| ring.0.len() >= 4)
        .map(|ring| Polygon::new(ring, vec![]))
        .collect()
}

/// Assemble a multipolygon relation
pub fn assemble_multipolygon(
    relation: &MultipolygonRelation,
    way_index: &HashMap<i64, Way>,
) -> Option<MultiPolygon<f64>> {
    // Collect outer and inner ways
    let mut outer_ways = Vec::new();
    let mut inner_ways = Vec::new();

    for member in &relation.members {
        if let Some(way) = way_index.get(&member.way_id) {
            match member.role {
                MemberRole::Outer => outer_ways.push(way.clone()),
                MemberRole::Inner => inner_ways.push(way.clone()),
                MemberRole::Unknown => {
                    // Default to outer if role is unknown
                    outer_ways.push(way.clone());
                }
            }
        }
    }

    if outer_ways.is_empty() {
        log::warn!("Relation {} has no outer ways", relation.id);
        return None;
    }

    // Assemble outer rings
    let outer_result = assemble_rings(outer_ways);
    if outer_result.closed_rings.is_empty() {
        log::warn!("Relation {} has no closed outer rings", relation.id);
        return None;
    }

    // Assemble inner rings (holes)
    let inner_result = assemble_rings(inner_ways);

    // Build polygons with proper hole assignment
    let mut polygons = Vec::new();

    for outer_ring in outer_result.closed_rings {
        // Create polygon without holes first
        let outer_poly = Polygon::new(outer_ring.clone(), vec![]);

        // Find holes that belong to this outer ring
        let holes: Vec<LineString<f64>> = inner_result
            .closed_rings
            .iter()
            .filter(|inner| {
                // Check if first point of inner ring is inside outer polygon
                if let Some(first_point) = inner.0.first() {
                    outer_poly.contains(&geo::Point::new(first_point.x, first_point.y))
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        polygons.push(Polygon::new(outer_ring, holes));
    }

    if polygons.is_empty() {
        None
    } else {
        Some(MultiPolygon::new(polygons))
    }
}

/// Build a way index from WayRefs and node cache
pub fn build_way_index(way_refs: &[WayRef], nodes: &NodeCache) -> HashMap<i64, Way> {
    let mut index = HashMap::new();

    for way_ref in way_refs {
        let coords: Option<Vec<Coord<f64>>> = way_ref
            .node_refs
            .iter()
            .map(|&id| nodes.get(id).copied())
            .collect();

        if let Some(c) = coords {
            index.insert(
                way_ref.id,
                Way {
                    id: way_ref.id,
                    coords: c,
                    tags: way_ref.tags.clone(),
                },
            );
        }
    }

    index
}

/// Convert closed WayRefs directly to polygons (for simple water bodies)
pub fn closed_ways_to_polygons(ways: &[Way]) -> Vec<Polygon<f64>> {
    ways.iter()
        .filter(|w| w.is_closed() && w.coords.len() >= 4)
        .map(|w| {
            let ring = LineString::new(w.coords.clone());
            Polygon::new(ring, vec![])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pbf::Tags;

    fn coord(x: f64, y: f64) -> Coord<f64> {
        Coord { x, y }
    }

    fn make_way(id: i64, coords: Vec<Coord<f64>>) -> Way {
        Way {
            id,
            coords,
            tags: Tags::new(),
        }
    }

    #[test]
    fn test_ring_assembly_simple_square() {
        // Create 4 ways that form a square
        let ways = vec![
            make_way(1, vec![coord(0.0, 0.0), coord(1.0, 0.0)]),
            make_way(2, vec![coord(1.0, 0.0), coord(1.0, 1.0)]),
            make_way(3, vec![coord(1.0, 1.0), coord(0.0, 1.0)]),
            make_way(4, vec![coord(0.0, 1.0), coord(0.0, 0.0)]),
        ];

        let result = assemble_rings(ways);
        assert_eq!(result.closed_rings.len(), 1);
        assert!(result.open_rings.is_empty());
    }

    #[test]
    fn test_ring_assembly_reversed_way() {
        // One way is reversed - should still connect
        let ways = vec![
            make_way(1, vec![coord(0.0, 0.0), coord(1.0, 0.0)]),
            make_way(2, vec![coord(1.0, 1.0), coord(1.0, 0.0)]), // reversed
            make_way(3, vec![coord(1.0, 1.0), coord(0.0, 0.0)]),
        ];

        let result = assemble_rings(ways);
        assert_eq!(result.closed_rings.len(), 1);
        assert!(result.open_rings.is_empty());
    }

    #[test]
    fn test_ring_assembly_single_closed_way() {
        // Already closed way
        let ways = vec![make_way(
            1,
            vec![
                coord(0.0, 0.0),
                coord(1.0, 0.0),
                coord(1.0, 1.0),
                coord(0.0, 0.0),
            ],
        )];

        let result = assemble_rings(ways);
        assert_eq!(result.closed_rings.len(), 1);
        assert!(result.open_rings.is_empty());
    }

    #[test]
    fn test_ring_assembly_open_ring() {
        // Ways that don't connect
        let ways = vec![
            make_way(1, vec![coord(0.0, 0.0), coord(1.0, 0.0)]),
            make_way(2, vec![coord(2.0, 0.0), coord(3.0, 0.0)]),
        ];

        let result = assemble_rings(ways);
        assert!(result.closed_rings.is_empty());
        assert_eq!(result.open_rings.len(), 2);
    }

    #[test]
    fn test_coord_key_precision() {
        let c1 = coord(0.1234567, 0.1234567);
        let c2 = coord(0.12345671, 0.12345671); // Very slightly different
        let c3 = coord(0.1234568, 0.1234568); // Different at 7th decimal

        assert_eq!(coord_key(&c1), coord_key(&c2)); // Should be same
        assert_ne!(coord_key(&c1), coord_key(&c3)); // Should be different
    }
}
