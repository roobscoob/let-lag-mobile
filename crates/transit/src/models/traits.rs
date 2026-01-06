//! Core traits for transit entities.
//!
//! These traits define the public interface for transit data.
//! Implementations can be in-memory, database-backed, or remote.

use chrono::NaiveDate;
use geo::{LineString, Point};
use std::sync::Arc;

use crate::identifiers::*;
use crate::models::types::*;
use crate::models::calendar::ServiceCalendar;

// ============================================================================
// Core Entity Traits
// ============================================================================

/// A single transit trip (vehicle run with specific stops and times)
pub trait Trip: Send + Sync {
    fn id(&self) -> &TripIdentifier;
    fn route_id(&self) -> &RouteIdentifier;

    /// Ordered stop events for this trip
    fn stop_events(&self) -> &[StopEvent];

    /// Which days does this trip run?
    fn runs_on(&self, date: NaiveDate) -> bool;

    /// Direction (e.g., Northbound vs Southbound)
    fn direction_id(&self) -> DirectionId;

    /// Display name (e.g., "Downtown", "To City Center")
    fn headsign(&self) -> &str;

    /// Service calendar (for querying availability)
    fn service_calendar(&self) -> &ServiceCalendar;
}

/// A transit route (e.g., "Red Line", "Route 66")
pub trait Route: Send + Sync {
    fn id(&self) -> &RouteIdentifier;

    /// Type of transportation
    fn route_type(&self) -> RouteType;

    /// Short name (e.g., "1", "A", "Red")
    fn short_name(&self) -> &str;

    /// Long name (e.g., "Broadway-7th Ave Local")
    fn long_name(&self) -> &str;

    /// Optional text color for display (hex RGB, e.g., "FF0000")
    fn color(&self) -> Option<&str> {
        None
    }

    /// Optional text color for display (hex RGB)
    fn text_color(&self) -> Option<&str> {
        None
    }

    /// Physical path the route takes (for spatial queries)
    /// May be empty if geometry unavailable
    fn geometry(&self) -> Option<&LineString>;

    /// All trips on this route
    fn trips(&self) -> &[Arc<dyn Trip>];
}

/// A transit station (single boarding location)
pub trait TransitStation: Send + Sync {
    fn id(&self) -> &StationIdentifier;
    fn name(&self) -> &str;
    fn location(&self) -> Point;

    /// Parent complex (if part of a multi-station complex)
    fn complex_id(&self) -> &ComplexIdentifier;
}

/// A complex of connected stations (e.g., Times Square, Union Station)
///
/// Multiple physical stops/platforms that are considered the same location
/// for transfer purposes.
pub trait TransitComplex: Send + Sync {
    fn id(&self) -> &ComplexIdentifier;
    fn name(&self) -> &str;

    /// All stations within this complex
    fn station_ids(&self) -> &[StationIdentifier];

    /// Approximate center point for the complex
    fn center(&self) -> Point;
}

// ============================================================================
// Provider Trait
// ============================================================================

/// Provider of all transit data with lookup and query methods
pub trait TransitProvider: Send + Sync {
    // ---- Lookups ----
    fn get_station(&self, id: &StationIdentifier) -> Option<Arc<dyn TransitStation>>;
    fn get_complex(&self, id: &ComplexIdentifier) -> Option<Arc<dyn TransitComplex>>;
    fn get_route(&self, id: &RouteIdentifier) -> Option<Arc<dyn Route>>;
    fn get_trip(&self, id: &TripIdentifier) -> Option<Arc<dyn Trip>>;

    // ---- Collections ----
    fn all_stations(&self) -> Vec<Arc<dyn TransitStation>>;
    fn all_complexes(&self) -> Vec<Arc<dyn TransitComplex>>;
    fn all_routes(&self) -> Vec<Arc<dyn Route>>;

    // ---- Spatial queries ----

    /// Find stations within radius (meters)
    fn stations_near(&self, point: Point, radius_m: f64) -> Vec<Arc<dyn TransitStation>>;

    /// Find routes within radius (meters)
    fn routes_near(&self, point: Point, radius_m: f64) -> Vec<Arc<dyn Route>>;

    /// Find the N nearest stations to a point
    fn nearest_stations(&self, point: Point, n: usize) -> Vec<Arc<dyn TransitStation>>;
}
