> [!WARNING]
> This was made with AI.
> It probably sucks

# jet-lag-transit

Offline-first transit data management for Rust applications.

## Features

- 🚇 **Offline-first**: All transit data stored in memory with efficient spatial indexing
- 🗺️ **Spatial indexing**: R-tree powered spatial queries for fast proximity searches
- ⚡ **Type-safe identifiers**: Arc-based identifiers for cheap cloning and memory efficiency
- 🌐 **Multi-network ready**: Designed to support multiple transit networks
- 🔌 **Pluggable networking**: Bring your own HTTP client and storage layer
- 🦀 **Pure Rust**: No unsafe code, full type safety

## Architecture

The crate is designed with a **functional core, imperative shell** philosophy:

- **Core logic is pure**: No IO operations in business logic
- **Traits for external integration**: Implement `BundleManager`, `DataFetcher`, and `StorageLoader` to integrate with your application
- **In-memory provider**: Fast lookups with spatial indexing
- **Extensible**: Easy to add new data sources and providers

## Quick Start

```rust
use jet_lag_transit::prelude::*;
use geo::Point;

// Create a provider with test data
let station = StationImpl {
    id: StationIdentifier::new("grand_central"),
    name: "Grand Central".into(),
    location: Point::new(-73.9772, 40.7527),
    complex_id: ComplexIdentifier::new("midtown"),
};

let complex = ComplexImpl {
    id: ComplexIdentifier::new("midtown"),
    name: "Midtown Manhattan".into(),
    station_ids: vec![StationIdentifier::new("grand_central")],
    center: Point::new(-73.9772, 40.7527),
};

let provider = StaticTransitProvider::from_data(vec![station], vec![complex], vec![]);

// Query stations near a point
let point = Point::new(-73.98, 40.75);
let nearby_stations = provider.stations_near(point, 500.0); // 500m radius

// Find nearest stations
let nearest = provider.nearest_stations(point, 5);

// Look up by ID
let station = provider.get_station(&StationIdentifier::new("grand_central"));
```

## Core Types

### Identifiers

All identifiers use `Arc<str>` internally for efficient cloning:

- `StationIdentifier` - Unique station ID
- `RouteIdentifier` - Unique route ID
- `TripIdentifier` - Unique trip ID
- `ComplexIdentifier` - Station complex ID
- `ServiceIdentifier` - Service calendar ID

### Traits

The crate defines several core traits:

- `TransitStation` - A single boarding location
- `TransitComplex` - A group of connected stations
- `Route` - A transit route (e.g., "Red Line", "Route 66")
- `Trip` - A vehicle run with specific stops and times
- `TransitProvider` - Main interface for querying transit data

### Spatial Queries

The `StaticTransitProvider` includes R-tree based spatial indexing for:

- `stations_near(point, radius_m)` - Find stations within a radius
- `routes_near(point, radius_m)` - Find routes within a radius
- `nearest_stations(point, n)` - Find N nearest stations

## Project Structure

```
transit/
├── src/
│   ├── identifiers.rs     # Type-safe ID types
│   ├── models/
│   │   ├── types.rs       # Core data types and enums
│   │   ├── traits.rs      # Core trait definitions
│   │   └── calendar.rs    # Service calendar logic
│   ├── provider/
│   │   └── static_provider.rs  # In-memory provider implementation
│   ├── spatial/
│   │   ├── index.rs       # R-tree spatial nodes
│   │   └── queries.rs     # Distance calculations
│   └── network/
│       └── traits.rs      # Pluggable network traits
└── tests/
```

## Optional Features

The crate includes several optional features that can be enabled:

- `compiler` - GTFS feed compilation (for server-side processing)
- `serde` - Serialization support

## Design Principles

1. **No IO in core logic** - All file/network operations go through trait boundaries
2. **Functional core** - Immutable data structures where possible
3. **Memory efficient** - Use `Arc<T>` for shared data
4. **Zero-cost abstractions** - Traits compile to direct method calls

## Integration

This crate is designed to be integrated into larger applications:

- **Mobile apps**: Implement `BundleManager` to download and cache transit bundles
- **Servers**: Use the `compiler` feature to process GTFS feeds
- **Games**: Query transit data for spatial gameplay mechanics

## License

MIT OR Apache-2.0
