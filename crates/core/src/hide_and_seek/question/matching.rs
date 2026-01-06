use std::sync::Arc;

use strum::EnumDiscriminants;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    hide_and_seek::question::context::QuestionContext,
    shape::{
        Shape,
        compiler::{Register, SdfCompiler},
        instruction::BoundaryOverlapResolution,
    },
    transit::StationIdentifier,
};

// TODO: change points to regions!
//       overlapping region resolution rules:
//         seeker:
//           ask the hider for the smaller region if you're inside multiple
//         hider:
//           answer 'generously', if the seeker asks for a region you're inside,
//           say yes even if it's not the largest region.
#[derive(EnumDiscriminants)]
pub enum MatchingTarget {
    // Transit
    // "Is your nearest commercial airport \"{}\"?"
    CommercialAirport {
        icao: Arc<str>,
        iata: Option<Arc<str>>,
    },

    // "Will {} stop at your station right now?"
    // NON-NULL
    TransitLine {
        scheduled_stations: Vec<StationIdentifier>,
    },

    // "Is the name of your station {} letters long?"
    // NON-NULL
    StationsNameLength(u32),

    // "Is your nearest street or path {:'the same as mine'}?"
    // NON-NULL
    StreetOrPath {
        osm_way_id: i64,
    },

    // Administrative Divisions
    // "Are you within the {} border for {}"
    // NON-NULL
    FirstAdministrativeDivision {
        osm_relation_id: i64,
    },

    // "Are you within the {} border for {}"
    // NON-NULL
    SecondAdministrativeDivision {
        osm_relation_id: i64,
    },

    // "Are you within the {} border for {}"
    // NON-NULL
    ThirdAdministrativeDivision {
        osm_relation_id: i64,
    },

    // "Are you within the {} border for {} "
    // NON-NULL
    FourthAdministrativeDivision {
        osm_relation_id: i64,
    },

    // Natural
    // "Is your nearest mountain {}?"
    Mountain {
        id: Arc<str>,
    },

    // "Is your nearest landmass {}?"
    // NON-NULL
    Landmass {
        landmass_id: Arc<str>,
    },

    // "Is your nearest park {}?"
    Park {
        osm_relation_park_id: i64,
    },

    // Places of Interest
    AmusementPark {
        osm_poi_theme_park_id: i64,
    },

    Zoo {
        osm_poi_zoo_id: i64,
    },

    Aquarium {
        osm_poi_aquarium_id: i64,
    },

    GolfCourse {
        osm_poi_golf_id: i64,
    },

    Museum {
        osm_poi_museum_id: i64,
    },

    MovieTheater {
        osm_poi_cinema_id: i64,
    },

    // Public Utilities
    Hospital {
        osm_poi_hospital_id: i64,
    },

    Library {
        osm_poi_library_id: i64,
    },

    ForeignConsulate {
        osm_poi_office_diplomatic_id: i64,
    },
}

// Is your nearest {category} the same as my nearest {category}?
pub struct MatchingQuestion {
    pub category: MatchingTarget,
}

pub enum MatchingQuestionAnswer {
    Yes,
    No,
}

// precondition: answer is non-null.
pub struct MatchingQuestionShape {
    pub question: MatchingQuestion,
    pub answer: MatchingQuestionAnswer,
    pub context: Box<dyn QuestionContext>,
}

impl Shape for MatchingQuestionShape {
    fn build_into(&self, compiler: &mut SdfCompiler) -> Register {
        let (other_points, question_point) = match &self.question.category {
            // precondition all_airports is non-empty because answer is non-null
            MatchingTarget::CommercialAirport { icao, .. } => {
                let other_points = self
                    .context
                    .get_all_pois("airport")
                    .unwrap()
                    .iter()
                    .filter_map(|airport| (*airport.id != **icao).then_some(airport.position))
                    .collect();

                let question_point = self
                    .context
                    .get_poi("airport", icao.as_ref())
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::TransitLine { scheduled_stations } => {
                let (question_stations, other_stations): (Vec<_>, Vec<_>) = self
                    .context
                    .transit_context()
                    .all_complexes()
                    .iter()
                    .partition(|c| {
                        c.all_stations()
                            .iter()
                            .any(|s| scheduled_stations.contains(&s.identifier()))
                    });

                let osp = compiler.point_cloud(other_stations.iter().map(|s| s.center()).collect());
                let qsp =
                    compiler.point_cloud(question_stations.iter().map(|s| s.center()).collect());

                (
                    compiler.dilate(osp, self.context.game_state().seeker_hiding_radius()),
                    compiler.dilate(qsp, self.context.game_state().seeker_hiding_radius()),
                )
            }

            MatchingTarget::StationsNameLength(target_length) => {
                let (question_stations, other_stations): (Vec<_>, Vec<_>) = self
                    .context
                    .transit_context()
                    .all_complexes()
                    .iter()
                    .flat_map(|c| c.all_stations().to_vec())
                    .partition(|s| s.name().graphemes(true).count() as u32 == *target_length);

                let osp = compiler.point_cloud(
                    other_stations
                        .iter()
                        .map(|s| s.complex().center())
                        .collect(),
                );

                let qsp = compiler.point_cloud(
                    question_stations
                        .iter()
                        .map(|s| s.complex().center())
                        .collect(),
                );

                (
                    compiler.dilate(osp, self.context.game_state().seeker_hiding_radius()),
                    compiler.dilate(qsp, self.context.game_state().seeker_hiding_radius()),
                )
            }

            MatchingTarget::StreetOrPath { osm_way_id } => {
                let way = self
                    .context
                    .street_or_path(*osm_way_id)
                    .expect("Invalid `osm_way_id`. Malicious player? TODO: Graceful handling.");

                let way = compiler.geodesic_string(way.positions);

                if matches!(self.answer, MatchingQuestionAnswer::Yes) {
                    return compiler.dilate(
                        way,
                        self.context
                            .game_state()
                            .hider_max_distance_to_street_or_path(),
                    );
                }

                let mut nearby: Vec<_> = self
                    .context
                    .nearby_streets_and_paths(
                        *osm_way_id,
                        self.context
                            .game_state()
                            .hider_max_distance_to_street_or_path(),
                    )
                    .into_iter()
                    .map(|way| compiler.geodesic_string(way.positions))
                    .collect();

                let not_way = compiler.invert(way);

                nearby.push(not_way);

                return compiler.union(nearby);
            }

            MatchingTarget::FirstAdministrativeDivision { osm_relation_id } => {
                let vdg = compiler.with_vdg(
                    self.context
                        .get_area(
                            "first_administrative_division",
                            format!("{}", osm_relation_id).as_str(),
                        )
                        .expect(
                            "Invalid `osm_relation_id`. Malicious player? TODO: Graceful handling.",
                        )
                        .boundary
                        .clone(),
                );

                return match self.answer {
                    MatchingQuestionAnswer::Yes => vdg,
                    MatchingQuestionAnswer::No => compiler.invert(vdg),
                };
            }

            MatchingTarget::SecondAdministrativeDivision { osm_relation_id } => {
                let vdg = compiler.with_vdg(
                    self.context
                        .get_area(
                            "second_administrative_division",
                            format!("{}", osm_relation_id).as_str(),
                        )
                        .expect(
                            "Invalid `osm_relation_id`. Malicious player? TODO: Graceful handling.",
                        )
                        .boundary
                        .clone(),
                );

                return match self.answer {
                    MatchingQuestionAnswer::Yes => vdg,
                    MatchingQuestionAnswer::No => compiler.invert(vdg),
                };
            }

            MatchingTarget::ThirdAdministrativeDivision { osm_relation_id } => {
                let vdg = compiler.with_vdg(
                    self.context
                        .get_area(
                            "third_administrative_division",
                            format!("{}", osm_relation_id).as_str(),
                        )
                        .expect(
                            "Invalid `osm_relation_id`. Malicious player? TODO: Graceful handling.",
                        )
                        .boundary
                        .clone(),
                );

                return match self.answer {
                    MatchingQuestionAnswer::Yes => vdg,
                    MatchingQuestionAnswer::No => compiler.invert(vdg),
                };
            }

            MatchingTarget::FourthAdministrativeDivision { osm_relation_id } => {
                let vdg = compiler.with_vdg(
                    self.context
                        .get_area(
                            "fourth_administrative_division",
                            format!("{}", osm_relation_id).as_str(),
                        )
                        .expect(
                            "Invalid `osm_relation_id`. Malicious player? TODO: Graceful handling.",
                        )
                        .boundary
                        .clone(),
                );

                return match self.answer {
                    MatchingQuestionAnswer::Yes => vdg,
                    MatchingQuestionAnswer::No => compiler.invert(vdg),
                };
            }

            MatchingTarget::Mountain { id } => {
                let other_points = self
                    .context
                    .get_all_pois("mountain")
                    .unwrap()
                    .iter()
                    .filter_map(|mountain| (*mountain.id != **id).then_some(mountain.position))
                    .collect();

                let question_point = self
                    .context
                    .get_poi("mountain", id.as_ref())
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::Landmass { landmass_id } => {
                let other_landmasses = self
                    .context
                    .get_all_areas("landmass")
                    .unwrap()
                    .iter()
                    .filter(|landmass| landmass.id.as_ref() != landmass_id.as_ref())
                    .map(|landmass| compiler.with_vdg(landmass.boundary.clone()))
                    .collect::<Vec<_>>();

                let question_landmass = self
                    .context
                    .get_area("landmass", landmass_id.as_ref())
                    .unwrap();

                (
                    compiler.union(other_landmasses),
                    compiler.with_vdg(question_landmass.boundary.clone()),
                )
            }

            MatchingTarget::Park {
                osm_relation_park_id,
            } => {
                let park_id = format!("{}", osm_relation_park_id);

                let other_points = self
                    .context
                    .get_all_pois("park")
                    .unwrap()
                    .iter()
                    .filter_map(|park| (*park.id != park_id).then_some(park.position))
                    .collect();

                let question_point = self
                    .context
                    .get_poi("park", park_id.as_str())
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::AmusementPark {
                osm_poi_theme_park_id,
            } => {
                let id = format!("{}", osm_poi_theme_park_id);

                let other_points = self
                    .context
                    .get_all_pois("amusement_park")
                    .unwrap()
                    .iter()
                    .filter_map(|poi| (*poi.id != id).then_some(poi.position))
                    .collect();

                let question_point = self
                    .context
                    .get_poi("amusement_park", &id)
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::Zoo { osm_poi_zoo_id } => {
                let id = format!("{}", osm_poi_zoo_id);

                let other_points = self
                    .context
                    .get_all_pois("zoo")
                    .unwrap()
                    .iter()
                    .filter_map(|poi| (*poi.id != id).then_some(poi.position))
                    .collect();

                let question_point = self.context.get_poi("zoo", &id).unwrap().position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::Aquarium {
                osm_poi_aquarium_id,
            } => {
                let id = format!("{}", osm_poi_aquarium_id);

                let other_points = self
                    .context
                    .get_all_pois("aquarium")
                    .unwrap()
                    .iter()
                    .filter_map(|poi| (*poi.id != id).then_some(poi.position))
                    .collect();

                let question_point = self.context.get_poi("aquarium", &id).unwrap().position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::GolfCourse { osm_poi_golf_id } => {
                let id = format!("{}", osm_poi_golf_id);

                let other_points = self
                    .context
                    .get_all_pois("golf_course")
                    .unwrap()
                    .iter()
                    .filter_map(|poi| (*poi.id != id).then_some(poi.position))
                    .collect();

                let question_point = self.context.get_poi("golf_course", &id).unwrap().position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::Museum { osm_poi_museum_id } => {
                let id = format!("{}", osm_poi_museum_id);

                let other_points = self
                    .context
                    .get_all_pois("museum")
                    .unwrap()
                    .iter()
                    .filter_map(|poi| (*poi.id != id).then_some(poi.position))
                    .collect();

                let question_point = self.context.get_poi("museum", &id).unwrap().position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::MovieTheater { osm_poi_cinema_id } => {
                let id = format!("{}", osm_poi_cinema_id);

                let other_points = self
                    .context
                    .get_all_pois("movie_theater")
                    .unwrap()
                    .iter()
                    .filter_map(|poi| (*poi.id != id).then_some(poi.position))
                    .collect();

                let question_point = self.context.get_poi("movie_theater", &id).unwrap().position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::Hospital {
                osm_poi_hospital_id,
            } => {
                let id = format!("{}", osm_poi_hospital_id);

                let other_points = self
                    .context
                    .get_all_pois("hospital")
                    .unwrap()
                    .iter()
                    .filter_map(|poi| (*poi.id != id).then_some(poi.position))
                    .collect();

                let question_point = self.context.get_poi("hospital", &id).unwrap().position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::Library { osm_poi_library_id } => {
                let id = format!("{}", osm_poi_library_id);

                let other_points = self
                    .context
                    .get_all_pois("library")
                    .unwrap()
                    .iter()
                    .filter_map(|poi| (*poi.id != id).then_some(poi.position))
                    .collect();

                let question_point = self.context.get_poi("library", &id).unwrap().position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }

            MatchingTarget::ForeignConsulate {
                osm_poi_office_diplomatic_id,
            } => {
                let id = format!("{}", osm_poi_office_diplomatic_id);

                let other_points = self
                    .context
                    .get_all_pois("foreign_consulate")
                    .unwrap()
                    .iter()
                    .filter_map(|poi| (*poi.id != id).then_some(poi.position))
                    .collect();

                let question_point = self
                    .context
                    .get_poi("foreign_consulate", &id)
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point),
                )
            }
        };

        match self.answer {
            MatchingQuestionAnswer::Yes => compiler.boundary(
                question_point,
                other_points,
                BoundaryOverlapResolution::Inside,
            ),

            MatchingQuestionAnswer::No => compiler.boundary(
                other_points,
                question_point,
                BoundaryOverlapResolution::Inside,
            ),
        }
    }
}
