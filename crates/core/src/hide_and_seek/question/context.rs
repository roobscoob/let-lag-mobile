use std::sync::Arc;

use crate::{
    hide_and_seek::state::GameState,
    shape::types::{Centimeters, Position},
    transit::TransitProvider,
};

pub struct CommercialAirport {
    pub name: Arc<str>,
    pub icao: Arc<str>,
    pub iata: Option<Arc<str>>,
    pub position: Position,
}

pub struct Mountain {
    pub name: Arc<str>,
    pub id: Option<Arc<str>>,
    pub position: Position,
}

pub struct OsmPoi {
    pub name: Arc<str>,
    pub osm_id: i64,
    pub position: Position,
}

pub struct OsmArea {
    pub osm_relation_id: i64,
    pub name: Option<Arc<str>>,
    pub boundary: boostvoronoi::prelude::Diagram,
}

pub struct OsmStreetOrPath {
    pub osm_way_id: i64,
    pub name: Option<Arc<str>>,
    pub positions: Vec<Position>,
}

pub trait QuestionContext {
    fn game_state(&self) -> &GameState;
    fn transit_context(&self) -> &dyn TransitProvider;
    fn all_airports(&self) -> &[CommercialAirport];
    fn street_or_path(&self, osm_way_id: i64) -> Option<OsmStreetOrPath>;

    /// Find nearby streets and paths for which a capsule with radius {intersection_distance} tracing the given
    /// street/path would intersect with a capsule tracing {osm_way_id}.
    fn nearby_streets_and_paths(
        &self,
        osm_way_id: i64,
        intersection_distance: Centimeters,
    ) -> Vec<OsmStreetOrPath>;

    fn first_administrative_division(&self, osm_relation_id: i64) -> Option<OsmArea>;
    fn second_administrative_division(&self, osm_relation_id: i64) -> Option<OsmArea>;
    fn third_administrative_division(&self, osm_relation_id: i64) -> Option<OsmArea>;
    fn fourth_administrative_division(&self, osm_relation_id: i64) -> Option<OsmArea>;

    fn all_mountains(&self) -> &[Mountain];
    fn all_parks(&self) -> &[OsmPoi];
    fn all_amusement_parks(&self) -> &[OsmPoi];
    fn all_zoos(&self) -> &[OsmPoi];
    fn all_aquariums(&self) -> &[OsmPoi];
    fn all_golf_courses(&self) -> &[OsmPoi];
    fn all_museums(&self) -> &[OsmPoi];
    fn all_movie_theaters(&self) -> &[OsmPoi];
    fn all_hospitals(&self) -> &[OsmPoi];
    fn all_libraries(&self) -> &[OsmPoi];
    fn all_foreign_consulates(&self) -> &[OsmPoi];
}
