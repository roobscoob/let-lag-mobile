use std::sync::Arc;

use strum::EnumDiscriminants;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    hide_and_seek::question::context::QuestionContext,
    shape::{
        Shape,
        compiler::{Register, SdfCompiler},
        instruction::BoundaryOverlapResolution,
        types::Position,
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
        id: Option<Arc<str>>,
        name: Arc<str>,
    },

    // "Is your nearest landmass {}?"
    // NON-NULL
    Landmass {
        // todo
    },

    // "Is your nearest park {}?"
    Park {
        osm_poi_park_id: i64,
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
            MatchingTarget::CommercialAirport { icao, iata } => {
                let mut other_points = self
                    .context
                    .all_airports()
                    .iter()
                    .filter_map(|airport| {
                        if airport.icao.as_ref() == icao.as_ref()
                            && (iata.is_none() || airport.iata.as_ref() == iata.as_ref())
                        {
                            Some(airport.position)
                        } else {
                            None
                        }
                    })
                    .collect();

                let question_point = self.context.all_airports().iter().find(|airport| {
                    airport.icao.as_ref() == icao.as_ref()
                        && (iata.is_none() || airport.iata.as_ref() == iata.as_ref())
                });

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
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

                // early return custom shape.

                let way = compiler.line_string(way);

                return compiler.dilate(
                    way,
                    self.context
                        .game_state()
                        .hider_max_distance_to_street_or_path(),
                );
            }

            _ => unimplemented!(),
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
