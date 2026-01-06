//! R-tree nodes for spatial indexing.
//!
//! Wraps transit entities with geometric data for efficient spatial queries.
//!
//! ## Two-Stage Filtering
//!
//! The spatial queries use a two-stage filtering approach:
//! 1. **R-tree filter**: Uses Euclidean distance for fast approximate filtering
//! 2. **Haversine filter**: Applies accurate geodesic distance on filtered results
//!
//! This approach balances performance (fast Euclidean checks in the R-tree) with
//! accuracy (precise Haversine distance for final results), which is especially
//! important for geographic coordinates where Euclidean distance becomes
//! increasingly inaccurate over larger distances.

use std::sync::Arc;
use geo::{Point, Line};
use rstar::{RTreeObject, AABB, PointDistance};

// Forward declare the concrete types that will be in provider module
// This allows us to compile spatial independently
use crate::provider::static_provider::{StationImpl, RouteImpl};

// ============================================================================
// Station Spatial Node
// ============================================================================

#[derive(Clone)]
pub struct StationNode {
    pub station: Arc<StationImpl>,
    point: [f64; 2],
}

impl StationNode {
    pub fn new(location: Point, station: Arc<StationImpl>) -> Self {
        Self {
            station,
            point: [location.x(), location.y()],
        }
    }
}

impl RTreeObject for StationNode {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point(self.point)
    }
}

impl PointDistance for StationNode {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        let dx = self.point[0] - point[0];
        let dy = self.point[1] - point[1];
        dx * dx + dy * dy
    }
}

// ============================================================================
// Route Segment Spatial Node
// ============================================================================

#[derive(Clone)]
pub struct RouteSegmentNode {
    pub route: Arc<RouteImpl>,
    pub segment: Line,
    aabb: AABB<[f64; 2]>,
}

impl RouteSegmentNode {
    pub fn new(segment: Line, route: Arc<RouteImpl>) -> Self {
        let start = [segment.start.x, segment.start.y];
        let end = [segment.end.x, segment.end.y];

        let aabb = AABB::from_corners(start, end);

        Self {
            route,
            segment,
            aabb,
        }
    }
}

impl RTreeObject for RouteSegmentNode {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.aabb
    }
}

impl PointDistance for RouteSegmentNode {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        // Distance from point to line segment (squared Euclidean distance)
        let p = [point[0], point[1]];
        let a = [self.segment.start.x, self.segment.start.y];
        let b = [self.segment.end.x, self.segment.end.y];

        let ab = [b[0] - a[0], b[1] - a[1]];
        let ap = [p[0] - a[0], p[1] - a[1]];

        let ab_ab = ab[0] * ab[0] + ab[1] * ab[1];

        if ab_ab == 0.0 {
            // Segment is actually a point
            return ap[0] * ap[0] + ap[1] * ap[1];
        }

        let ab_ap = ab[0] * ap[0] + ab[1] * ap[1];
        let t = (ab_ap / ab_ab).clamp(0.0, 1.0);

        let closest = [a[0] + t * ab[0], a[1] + t * ab[1]];
        let dx = p[0] - closest[0];
        let dy = p[1] - closest[1];

        dx * dx + dy * dy
    }
}
