//! Core data types and enums for transit data.

use crate::identifiers::*;

// ============================================================================
// Enums
// ============================================================================

/// GTFS route types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RouteType {
    Tram = 0,
    Subway = 1,
    Rail = 2,
    Bus = 3,
    Ferry = 4,
    CableTram = 5,
    AerialLift = 6,
    Funicular = 7,
}

impl RouteType {
    pub fn from_gtfs(value: u16) -> Option<Self> {
        match value {
            0 => Some(Self::Tram),
            1 => Some(Self::Subway),
            2 => Some(Self::Rail),
            3 => Some(Self::Bus),
            4 => Some(Self::Ferry),
            5 => Some(Self::CableTram),
            6 => Some(Self::AerialLift),
            7 => Some(Self::Funicular),
            _ => None,
        }
    }
}

/// Trip direction (0 = outbound, 1 = inbound per GTFS)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DirectionId {
    Outbound = 0,
    Inbound = 1,
}

// ============================================================================
// Data Structures
// ============================================================================

/// A single stop event in a trip (arrival/departure at a station)
///
/// Times are stored as seconds since midnight of the service day.
/// Per GTFS spec, times can exceed 24 hours for trips past midnight
/// (e.g., 25:30:00 = 91800 seconds for 1:30am the next day).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopEvent {
    pub station_id: StationIdentifier,
    pub arrival: u32,      // Seconds since midnight (service day start)
    pub departure: u32,    // Seconds since midnight (service day start)
    pub stop_sequence: u32,
}

impl StopEvent {
    pub fn new(
        station_id: StationIdentifier,
        arrival: u32,
        departure: u32,
        stop_sequence: u32,
    ) -> Self {
        Self {
            station_id,
            arrival,
            departure,
            stop_sequence,
        }
    }

    /// Apply a delay in seconds (for realtime updates)
    ///
    /// Returns `Err` if the delay would cause the departure time to be before the arrival time.
    pub fn with_delay(&self, delay_seconds: i32) -> Result<Self> {
        let new_arrival = (self.arrival as i32 + delay_seconds).max(0) as u32;
        let new_departure = (self.departure as i32 + delay_seconds).max(0) as u32;

        if new_departure < new_arrival {
            return Err(TransitError::InvalidData(
                format!("Delay of {}s would cause departure ({}) before arrival ({})",
                    delay_seconds, new_departure, new_arrival)
            ));
        }

        Ok(Self {
            arrival: new_arrival,
            departure: new_departure,
            ..self.clone()
        })
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum TransitError {
    #[error("Station not found: {0}")]
    StationNotFound(StationIdentifier),

    #[error("Route not found: {0}")]
    RouteNotFound(RouteIdentifier),

    #[error("Trip not found: {0}")]
    TripNotFound(TripIdentifier),

    #[error("Complex not found: {0}")]
    ComplexNotFound(ComplexIdentifier),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

pub type Result<T> = std::result::Result<T, TransitError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_event_delay() {
        let event = StopEvent::new(
            StationIdentifier::new("station_1"),
            1000,
            1020,
            1,
        );

        let delayed = event.with_delay(300).unwrap();
        assert_eq!(delayed.arrival, 1300);
        assert_eq!(delayed.departure, 1320);

        // Negative delay shouldn't go below 0
        let early = event.with_delay(-2000).unwrap();
        assert_eq!(early.arrival, 0);
        assert_eq!(early.departure, 0);

        // Very large negative delay that would cause departure < arrival should error
        let event2 = StopEvent::new(
            StationIdentifier::new("station_1"),
            1000,
            1050,
            1,
        );
        // Delay of -1030 would make arrival = 0, departure = 20, which is valid
        assert!(event2.with_delay(-1030).is_ok());

        // But delay of -1100 would make arrival = 0, but departure would want to be negative
        // so it gets clamped to 0, making departure = arrival, which is valid
        assert!(event2.with_delay(-1100).is_ok());
    }

    #[test]
    fn test_route_type_from_gtfs() {
        assert_eq!(RouteType::from_gtfs(1), Some(RouteType::Subway));
        assert_eq!(RouteType::from_gtfs(3), Some(RouteType::Bus));
        assert_eq!(RouteType::from_gtfs(99), None);
    }
}
