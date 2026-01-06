//! Spatial query utilities for distance calculations.
//!
//! Uses Haversine formula for accurate distances on Earth's surface.

use geo::{Point, Line, HaversineDistance, ClosestPoint, LineString};

/// Calculate Haversine distance between two points in meters
pub fn haversine_distance(p1: Point, p2: Point) -> f64 {
    p1.haversine_distance(&p2)
}

/// Calculate distance from point to line segment in meters
pub fn haversine_distance_to_line(point: Point, line: Line) -> f64 {
    // Convert line to LineString for ClosestPoint trait
    let line_string = LineString::from(vec![line.start, line.end]);

    match line_string.closest_point(&point) {
        geo::Closest::Intersection(p) | geo::Closest::SinglePoint(p) => {
            haversine_distance(point, p)
        }
        geo::Closest::Indeterminate => f64::INFINITY,
    }
}

/// Convert degrees to approximate meters at equator (for bounding box queries)
pub fn degrees_to_meters_approx(degrees: f64) -> f64 {
    degrees * 111_320.0 // meters per degree at equator
}

/// Convert meters to degrees at equator (for bounding box queries)
pub fn meters_to_degrees_approx(meters: f64) -> f64 {
    meters / 111_320.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_haversine_distance() {
        // Distance from NYC to LA is approximately 3,936 km
        let nyc = Point::new(-74.0060, 40.7128);
        let la = Point::new(-118.2437, 34.0522);

        let dist = haversine_distance(nyc, la);
        assert!((dist - 3_936_000.0).abs() < 50_000.0); // Within 50km
    }

    #[test]
    fn test_distance_to_line() {
        let point = Point::new(-74.0, 40.7);
        let line = Line::new(
            geo::Coord { x: -74.0, y: 40.6 },
            geo::Coord { x: -74.0, y: 40.8 },
        );

        // Point is on the line, distance should be near 0
        let dist = haversine_distance_to_line(point, line);
        assert!(dist < 100.0); // Within 100 meters
    }
}
