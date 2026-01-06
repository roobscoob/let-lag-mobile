//! # jet-lag-transit
//!
//! Offline-first transit data management with modular networking.
//!
//! ## Features
//!
//! - **Offline-first**: All transit data stored in compiled bundles
//! - **Spatial queries**: Fast R-tree based spatial indexing
//! - **Realtime overlay**: Apply GTFS-RT updates over static schedules (optional)
//! - **Multi-network**: Aggregate multiple transit networks (optional)
//! - **Pluggable networking**: Implement your own data fetching
//!
//! ## Example
//!
//! ```
//! use jet_lag_transit::prelude::*;
//! use geo::Point;
//!
//! // Create a provider with test data
//! let station = StationImpl {
//!     id: StationIdentifier::new("nyc_penn"),
//!     name: "Penn Station".into(),
//!     location: Point::new(-73.9935, 40.7505),
//!     complex_id: ComplexIdentifier::new("penn_complex"),
//! };
//!
//! let complex = ComplexImpl {
//!     id: ComplexIdentifier::new("penn_complex"),
//!     name: "Penn Station Complex".into(),
//!     station_ids: vec![StationIdentifier::new("nyc_penn")],
//!     center: Point::new(-73.9935, 40.7505),
//! };
//!
//! let provider = StaticTransitProvider::from_data(vec![station], vec![complex], vec![]);
//!
//! // Query stations
//! let point = Point::new(-74.0060, 40.7128); // NYC
//! let nearby = provider.stations_near(point, 5000.0); // 5km radius
//! assert_eq!(nearby.len(), 1);
//! ```

pub mod identifiers;
pub mod models;
pub mod provider;
pub mod spatial;
pub mod network;

// Re-exports for convenience
pub mod prelude {
    pub use crate::identifiers::*;
    pub use crate::models::{traits::*, types::*};
    pub use crate::provider::{
        static_provider::StaticTransitProvider,
        ComplexImpl, RouteImpl, StationImpl, TripImpl,
    };
    pub use crate::network::traits::*;
}

// Module declarations
pub use prelude::*;
