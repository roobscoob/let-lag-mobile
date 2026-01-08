use std::sync::Arc;

use jet_lag_transit::TransitProvider;

use crate::{
    hide_and_seek::state::GameState,
    shape::{contour_texture::ContourTexture, types::Centimeters},
};

pub struct Poi {
    pub name: Option<Arc<str>>,
    pub id: Arc<str>,
    pub position: geo::Point,
}

pub struct Area {
    pub name: Option<Arc<str>>,
    pub id: Arc<str>,
    pub boundary: Arc<boostvoronoi::prelude::Diagram>,
}

pub struct PathSegment {
    pub id: i64,
    pub name: Option<Arc<str>>,
    pub positions: geo::LineString,
}

pub trait QuestionContext: Send {
    fn game_state(&self) -> &GameState;
    fn transit_context(&self) -> &dyn TransitProvider;

    fn street_or_path(&self, osm_way_id: i64) -> Option<PathSegment>;
    fn has_street_or_path_data(&self) -> bool;

    /// Find nearby streets and paths for which a capsule with radius {intersection_distance} tracing the given
    /// street/path would intersect with a capsule tracing {osm_way_id}.
    fn nearby_streets_and_paths(
        &self,
        osm_way_id: i64,
        intersection_distance: Centimeters,
    ) -> Vec<PathSegment>;

    fn get_all_pois(&self, category: &str) -> Option<&[Poi]>;
    fn get_poi(&self, category: &str, id: &str) -> Option<&Poi>;
    fn has_poi_category(&self, category: &str) -> bool;
    fn get_all_areas(&self, category: &str) -> Option<&[Area]>;
    fn get_all_areas_as_vdg(&self, category: &str) -> Option<Arc<boostvoronoi::prelude::Diagram>>;
    fn get_area(&self, category: &str, id: &str) -> Option<&Area>;
    fn has_area_category(&self, category: &str) -> bool;

    fn sea_level_contour_texture(&self) -> Option<Arc<ContourTexture>>;
    fn has_sea_level_contour_texture(&self) -> bool;

    fn high_speed_rail_lines(&self) -> Option<&[PathSegment]>;
    fn has_high_speed_rail_line_data(&self) -> bool;
}
