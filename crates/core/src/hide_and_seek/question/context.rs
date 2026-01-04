use std::sync::Arc;

use crate::{hide_and_seek::state::GameState, shape::types::Position, transit::TransitProvider};

pub struct CommercialAirport {
    pub name: Arc<str>,
    pub icao: Arc<str>,
    pub iata: Option<Arc<str>>,
    pub position: Position,
}

pub trait QuestionContext {
    fn game_state(&self) -> &GameState;
    fn transit_context(&self) -> &dyn TransitProvider;
    fn all_airports(&self) -> &[CommercialAirport];
    fn street_or_path(&self, osm_way_id: i64) -> Option<Vec<Position>>;
}
