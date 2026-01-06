use crate::shape::types::Centimeters;

#[derive(Clone, Copy)]
pub struct GameState {}

impl GameState {
    // hardcoded for now; will be made configurable later
    /// The radius within which a seeker can move around their hiding spot
    pub fn seeker_hiding_radius(&self) -> Centimeters {
        Centimeters::from_meters(402.336) // 0.25 miles
    }

    // hardcoded for now; will be made configurable later
    pub fn hider_max_distance_to_street_or_path(&self) -> Centimeters {
        Centimeters::from_millimeters(3048)
    }
}
