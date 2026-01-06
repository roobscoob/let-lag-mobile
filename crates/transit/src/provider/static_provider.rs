//! In-memory transit provider backed by compiled bundles.
//!
//! This is the core implementation that stores all transit data in memory
//! with spatial indices for fast queries.

use std::collections::HashMap;
use std::sync::Arc;

use geo::{Point, LineString};
use chrono::NaiveDate;
use rstar::RTree;

use crate::identifiers::*;
use crate::models::{types::*, traits::*, calendar::ServiceCalendar};
use crate::spatial::index::{StationNode, RouteSegmentNode};

// ============================================================================
// Concrete Implementations of Traits
// ============================================================================

#[derive(Clone, Debug)]
pub struct StationImpl {
    pub id: StationIdentifier,
    pub name: Arc<str>,
    pub location: Point,
    pub complex_id: ComplexIdentifier,
}

impl TransitStation for StationImpl {
    fn id(&self) -> &StationIdentifier {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn location(&self) -> Point {
        self.location
    }

    fn complex_id(&self) -> &ComplexIdentifier {
        &self.complex_id
    }
}

#[derive(Clone, Debug)]
pub struct ComplexImpl {
    pub id: ComplexIdentifier,
    pub name: Arc<str>,
    pub station_ids: Vec<StationIdentifier>,
    pub center: Point,
}

impl TransitComplex for ComplexImpl {
    fn id(&self) -> &ComplexIdentifier {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn station_ids(&self) -> &[StationIdentifier] {
        &self.station_ids
    }

    fn center(&self) -> Point {
        self.center
    }
}

#[derive(Clone, Debug)]
pub struct TripImpl {
    pub id: TripIdentifier,
    pub route_id: RouteIdentifier,
    pub stop_events: Vec<StopEvent>,
    pub service_calendar: Arc<ServiceCalendar>,
    pub direction_id: DirectionId,
    pub headsign: Arc<str>,
}

impl Trip for TripImpl {
    fn id(&self) -> &TripIdentifier {
        &self.id
    }

    fn route_id(&self) -> &RouteIdentifier {
        &self.route_id
    }

    fn stop_events(&self) -> &[StopEvent] {
        &self.stop_events
    }

    fn runs_on(&self, date: NaiveDate) -> bool {
        self.service_calendar.runs_on(date)
    }

    fn direction_id(&self) -> DirectionId {
        self.direction_id
    }

    fn headsign(&self) -> &str {
        &self.headsign
    }

    fn service_calendar(&self) -> &ServiceCalendar {
        &self.service_calendar
    }
}

#[derive(Clone)]
pub struct RouteImpl {
    pub id: RouteIdentifier,
    pub route_type: RouteType,
    pub short_name: Arc<str>,
    pub long_name: Arc<str>,
    pub color: Option<Arc<str>>,
    pub text_color: Option<Arc<str>>,
    pub geometry: Option<LineString>,
    pub trips: Vec<Arc<dyn Trip>>,
}

impl Route for RouteImpl {
    fn id(&self) -> &RouteIdentifier {
        &self.id
    }

    fn route_type(&self) -> RouteType {
        self.route_type
    }

    fn short_name(&self) -> &str {
        &self.short_name
    }

    fn long_name(&self) -> &str {
        &self.long_name
    }

    fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    fn text_color(&self) -> Option<&str> {
        self.text_color.as_deref()
    }

    fn geometry(&self) -> Option<&LineString> {
        self.geometry.as_ref()
    }

    fn trips(&self) -> &[Arc<dyn Trip>] {
        &self.trips
    }
}

// ============================================================================
// Static Provider
// ============================================================================

/// In-memory transit provider with spatial indexing
///
/// This type is cheap to clone since all data is stored in `Arc`s.
#[derive(Clone)]
pub struct StaticTransitProvider {
    // Core data
    stations: Vec<Arc<StationImpl>>,
    complexes: Vec<Arc<ComplexImpl>>,
    routes: Vec<Arc<RouteImpl>>,

    // Lookup maps
    station_map: HashMap<StationIdentifier, Arc<StationImpl>>,
    complex_map: HashMap<ComplexIdentifier, Arc<ComplexImpl>>,
    route_map: HashMap<RouteIdentifier, Arc<RouteImpl>>,
    trip_map: HashMap<TripIdentifier, Arc<dyn Trip>>,

    // Spatial indices
    station_tree: RTree<StationNode>,
    route_tree: RTree<RouteSegmentNode>,
}

impl StaticTransitProvider {
    /// Create a new empty provider
    pub fn new() -> Self {
        Self {
            stations: Vec::new(),
            complexes: Vec::new(),
            routes: Vec::new(),
            station_map: HashMap::new(),
            complex_map: HashMap::new(),
            route_map: HashMap::new(),
            trip_map: HashMap::new(),
            station_tree: RTree::new(),
            route_tree: RTree::new(),
        }
    }

    /// Build provider from raw data (used by deserializer)
    pub fn from_data(
        stations: Vec<StationImpl>,
        complexes: Vec<ComplexImpl>,
        routes: Vec<RouteImpl>,
    ) -> Self {
        let stations: Vec<Arc<StationImpl>> = stations.into_iter().map(Arc::new).collect();
        let complexes: Vec<Arc<ComplexImpl>> = complexes.into_iter().map(Arc::new).collect();
        let routes: Vec<Arc<RouteImpl>> = routes.into_iter().map(Arc::new).collect();

        // Build lookup maps
        let station_map: HashMap<_, _> = stations
            .iter()
            .map(|s| (s.id.clone(), s.clone()))
            .collect();

        let complex_map: HashMap<_, _> = complexes
            .iter()
            .map(|c| (c.id.clone(), c.clone()))
            .collect();

        let route_map: HashMap<_, _> = routes
            .iter()
            .map(|r| (r.id.clone(), r.clone()))
            .collect();

        // Build trip map
        let mut trip_map = HashMap::new();
        for route in &routes {
            for trip in route.trips() {
                trip_map.insert(trip.id().clone(), trip.clone());
            }
        }

        // Build spatial indices
        let station_tree = RTree::bulk_load(
            stations
                .iter()
                .map(|s| StationNode::new(s.location, s.clone()))
                .collect(),
        );

        let mut route_segments = Vec::new();
        for route in &routes {
            if let Some(geom) = &route.geometry {
                for segment in geom.lines() {
                    route_segments.push(RouteSegmentNode::new(segment, route.clone()));
                }
            }
        }
        let route_tree = RTree::bulk_load(route_segments);

        Self {
            stations,
            complexes,
            routes,
            station_map,
            complex_map,
            route_map,
            trip_map,
            station_tree,
            route_tree,
        }
    }
}

impl Default for StaticTransitProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TransitProvider for StaticTransitProvider {
    fn get_station(&self, id: &StationIdentifier) -> Option<Arc<dyn TransitStation>> {
        self.station_map.get(id).map(|s| s.clone() as Arc<dyn TransitStation>)
    }

    fn get_complex(&self, id: &ComplexIdentifier) -> Option<Arc<dyn TransitComplex>> {
        self.complex_map.get(id).map(|c| c.clone() as Arc<dyn TransitComplex>)
    }

    fn get_route(&self, id: &RouteIdentifier) -> Option<Arc<dyn Route>> {
        self.route_map.get(id).map(|r| r.clone() as Arc<dyn Route>)
    }

    fn get_trip(&self, id: &TripIdentifier) -> Option<Arc<dyn Trip>> {
        self.trip_map.get(id).cloned()
    }

    fn all_stations(&self) -> Vec<Arc<dyn TransitStation>> {
        self.stations
            .iter()
            .map(|s| s.clone() as Arc<dyn TransitStation>)
            .collect()
    }

    fn all_complexes(&self) -> Vec<Arc<dyn TransitComplex>> {
        self.complexes
            .iter()
            .map(|c| c.clone() as Arc<dyn TransitComplex>)
            .collect()
    }

    fn all_routes(&self) -> Vec<Arc<dyn Route>> {
        self.routes
            .iter()
            .map(|r| r.clone() as Arc<dyn Route>)
            .collect()
    }

    fn stations_near(&self, point: Point, radius_m: f64) -> Vec<Arc<dyn TransitStation>> {
        use crate::spatial::queries::haversine_distance;

        // Validate radius is positive
        if radius_m <= 0.0 || !radius_m.is_finite() {
            return Vec::new();
        }

        self.station_tree
            .locate_within_distance([point.x(), point.y()], radius_m)
            .filter(|node| {
                haversine_distance(point, node.station.location) <= radius_m
            })
            .map(|node| node.station.clone() as Arc<dyn TransitStation>)
            .collect()
    }

    fn routes_near(&self, point: Point, radius_m: f64) -> Vec<Arc<dyn Route>> {
        use crate::spatial::queries::haversine_distance_to_line;

        // Validate radius is positive
        if radius_m <= 0.0 || !radius_m.is_finite() {
            return Vec::new();
        }

        let mut seen = std::collections::HashSet::new();
        self.route_tree
            .locate_within_distance([point.x(), point.y()], radius_m)
            .filter(|node| {
                haversine_distance_to_line(point, node.segment) <= radius_m
            })
            .filter(|node| seen.insert(node.route.id.clone()))
            .map(|node| node.route.clone() as Arc<dyn Route>)
            .collect()
    }

    fn nearest_stations(&self, point: Point, n: usize) -> Vec<Arc<dyn TransitStation>> {
        self.station_tree
            .nearest_neighbor_iter(&[point.x(), point.y()])
            .take(n)
            .map(|node| node.station.clone() as Arc<dyn TransitStation>)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_provider() {
        let provider = StaticTransitProvider::new();
        assert_eq!(provider.all_stations().len(), 0);
        assert_eq!(provider.all_routes().len(), 0);
    }

    #[test]
    fn test_provider_lookups() {
        let station = StationImpl {
            id: StationIdentifier::new("s1"),
            name: "Test Station".into(),
            location: Point::new(-74.0, 40.7),
            complex_id: ComplexIdentifier::new("c1"),
        };

        let complex = ComplexImpl {
            id: ComplexIdentifier::new("c1"),
            name: "Test Complex".into(),
            station_ids: vec![StationIdentifier::new("s1")],
            center: Point::new(-74.0, 40.7),
        };

        let provider = StaticTransitProvider::from_data(vec![station], vec![complex], vec![]);

        assert!(provider.get_station(&StationIdentifier::new("s1")).is_some());
        assert!(provider.get_complex(&ComplexIdentifier::new("c1")).is_some());
    }
}
