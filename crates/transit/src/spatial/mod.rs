//! Spatial indexing and query utilities.

pub mod index;
pub mod queries;

pub use queries::{haversine_distance, haversine_distance_to_line};
