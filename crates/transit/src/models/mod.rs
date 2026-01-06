//! Transit data models, types, and traits.

pub mod calendar;
pub mod traits;
pub mod types;

// Re-exports for convenience
pub use calendar::{ServiceCalendar, WeekdayFlags};
pub use traits::{Route, TransitComplex, TransitProvider, TransitStation, Trip};
pub use types::{DirectionId, RouteType, StopEvent, TransitError, Result};
