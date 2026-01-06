//! Type-safe, efficient identifiers for transit entities.
//!
//! All identifiers use Arc<str> for cheap cloning and minimal memory overhead.

use std::sync::Arc;
use std::fmt;
use std::hash::{Hash, Hasher};

macro_rules! impl_identifier {
    ($name:ident) => {
        #[derive(Clone, Debug)]
        pub struct $name(Arc<str>);

        impl $name {
            pub fn new(s: impl AsRef<str>) -> Self {
                Self(s.as_ref().into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
            }
        }

        impl Eq for $name {}

        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self::new(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self::new(s)
            }
        }
    };
}

impl_identifier!(StationIdentifier);
impl_identifier!(RouteIdentifier);
impl_identifier!(TripIdentifier);
impl_identifier!(ComplexIdentifier);
impl_identifier!(ServiceIdentifier);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identifier_equality() {
        let id1 = StationIdentifier::new("station_123");
        let id2 = StationIdentifier::new("station_123");
        let id3 = id1.clone();

        assert_eq!(id1, id2);
        assert_eq!(id1, id3);
        assert!(Arc::ptr_eq(&id1.0, &id3.0)); // Clone shares Arc
    }

    #[test]
    fn test_identifier_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert(StationIdentifier::new("test"), 42);

        assert_eq!(map.get(&StationIdentifier::new("test")), Some(&42));
    }

    #[test]
    fn test_identifier_display() {
        let id = RouteIdentifier::new("route_1");
        assert_eq!(format!("{}", id), "route_1");
    }

    #[test]
    fn test_identifier_conversions() {
        let _id1: TripIdentifier = "trip_1".into();
        let _id2: TripIdentifier = String::from("trip_2").into();
    }
}
