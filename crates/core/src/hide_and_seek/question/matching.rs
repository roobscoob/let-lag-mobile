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
            MatchingTarget::CommercialAirport { icao, iata } => {
                let mut other_points = self
                    .context
                    .all_airports()
                    .iter()
                    .filter_map(|airport| {
                        if airport.icao.as_ref() == icao.as_ref()
                            && (iata.is_none() || airport.iata.as_ref() == iata.as_ref())
                        {
                            None
                        } else {
                            Some(airport.position)
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

                let way = compiler.line_string(way.positions);

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
                    .map(|way| compiler.line_string(way.positions))
                    .collect();

                let not_way = compiler.invert(way);

                nearby.push(not_way);

                return compiler.union(nearby);
            }

            MatchingTarget::FirstAdministrativeDivision { osm_relation_id } => {
                let vdg = compiler.with_vdg(Arc::new(
                    self.context
                        .first_administrative_division(*osm_relation_id)
                        .expect(
                            "Invalid `osm_relation_id`. Malicious player? TODO: Graceful handling.",
                        )
                        .boundary,
                ));

                return match self.answer {
                    MatchingQuestionAnswer::Yes => vdg,
                    MatchingQuestionAnswer::No => compiler.invert(vdg),
                };
            }

            MatchingTarget::SecondAdministrativeDivision { osm_relation_id } => {
                let vdg = compiler.with_vdg(Arc::new(
                    self.context
                        .second_administrative_division(*osm_relation_id)
                        .expect(
                            "Invalid `osm_relation_id`. Malicious player? TODO: Graceful handling.",
                        )
                        .boundary,
                ));

                return match self.answer {
                    MatchingQuestionAnswer::Yes => vdg,
                    MatchingQuestionAnswer::No => compiler.invert(vdg),
                };
            }

            MatchingTarget::ThirdAdministrativeDivision { osm_relation_id } => {
                let vdg = compiler.with_vdg(Arc::new(
                    self.context
                        .third_administrative_division(*osm_relation_id)
                        .expect(
                            "Invalid `osm_relation_id`. Malicious player? TODO: Graceful handling.",
                        )
                        .boundary,
                ));

                return match self.answer {
                    MatchingQuestionAnswer::Yes => vdg,
                    MatchingQuestionAnswer::No => compiler.invert(vdg),
                };
            }

            MatchingTarget::FourthAdministrativeDivision { osm_relation_id } => {
                let vdg = compiler.with_vdg(Arc::new(
                    self.context
                        .fourth_administrative_division(*osm_relation_id)
                        .expect(
                            "Invalid `osm_relation_id`. Malicious player? TODO: Graceful handling.",
                        )
                        .boundary,
                ));

                return match self.answer {
                    MatchingQuestionAnswer::Yes => vdg,
                    MatchingQuestionAnswer::No => compiler.invert(vdg),
                };
            }

            MatchingTarget::Mountain { id, name } => {
                let mut other_points = self
                    .context
                    .all_mountains()
                    .iter()
                    .filter_map(|mountain| {
                        if mountain.name.as_ref() == name.as_ref()
                            && (id.is_none() || mountain.id.as_ref() == id.as_ref())
                        {
                            None
                        } else {
                            Some(mountain.position)
                        }
                    })
                    .collect();

                let question_point = self.context.all_mountains().iter().find(|mountain| {
                    mountain.name.as_ref() == name.as_ref()
                        && (id.is_none() || mountain.id.as_ref() == id.as_ref())
                });

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
                )
            }

            MatchingTarget::Landmass { .. } => todo!(),

            MatchingTarget::Park {
                osm_relation_park_id,
            } => {
                let mut other_points = self
                    .context
                    .all_parks()
                    .iter()
                    .filter_map(|park| {
                        if park.osm_relation_id == *osm_relation_park_id {
                            None
                        } else {
                            Some(park)
                        }
                    })
                    .collect();

                let question_point = self
                    .context
                    .all_parks()
                    .iter()
                    .find(|park| park.osm_relation_id == *osm_relation_park_id);

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().boundary.centroid()),
                )
            }

            MatchingTarget::AmusementPark {
                osm_poi_theme_park_id,
            } => {
                let mut other_points = self
                    .context
                    .all_amusement_parks()
                    .iter()
                    .filter_map(|poi| {
                        if poi.osm_id == *osm_poi_theme_park_id {
                            None
                        } else {
                            Some(poi.position)
                        }
                    })
                    .collect();

                let question_point = self
                    .context
                    .all_amusement_parks()
                    .iter()
                    .find(|poi| poi.osm_id == *osm_poi_theme_park_id);

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
                )
            }

            MatchingTarget::Zoo { osm_poi_zoo_id } => {
                let mut other_points = self
                    .context
                    .all_zoos()
                    .iter()
                    .filter_map(|poi| {
                        if poi.osm_id == *osm_poi_zoo_id {
                            None
                        } else {
                            Some(poi.position)
                        }
                    })
                    .collect();

                let question_point = self
                    .context
                    .all_zoos()
                    .iter()
                    .find(|poi| poi.osm_id == *osm_poi_zoo_id);

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
                )
            }

            MatchingTarget::Aquarium {
                osm_poi_aquarium_id,
            } => {
                let mut other_points = self
                    .context
                    .all_aquariums()
                    .iter()
                    .filter_map(|poi| {
                        if poi.osm_id == *osm_poi_aquarium_id {
                            None
                        } else {
                            Some(poi.position)
                        }
                    })
                    .collect();

                let question_point = self
                    .context
                    .all_aquariums()
                    .iter()
                    .find(|poi| poi.osm_id == *osm_poi_aquarium_id);

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
                )
            }

            MatchingTarget::GolfCourse { osm_poi_golf_id } => {
                let mut other_points = self
                    .context
                    .all_golf_courses()
                    .iter()
                    .filter_map(|poi| {
                        if poi.osm_id == *osm_poi_golf_id {
                            None
                        } else {
                            Some(poi.position)
                        }
                    })
                    .collect();

                let question_point = self
                    .context
                    .all_golf_courses()
                    .iter()
                    .find(|poi| poi.osm_id == *osm_poi_golf_id);

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
                )
            }

            MatchingTarget::Museum { osm_poi_museum_id } => {
                let mut other_points = self
                    .context
                    .all_museums()
                    .iter()
                    .filter_map(|poi| {
                        if poi.osm_id == *osm_poi_museum_id {
                            None
                        } else {
                            Some(poi.position)
                        }
                    })
                    .collect();

                let question_point = self
                    .context
                    .all_museums()
                    .iter()
                    .find(|poi| poi.osm_id == *osm_poi_museum_id);

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
                )
            }

            MatchingTarget::MovieTheater { osm_poi_cinema_id } => {
                let mut other_points = self
                    .context
                    .all_movie_theaters()
                    .iter()
                    .filter_map(|poi| {
                        if poi.osm_id == *osm_poi_cinema_id {
                            None
                        } else {
                            Some(poi.position)
                        }
                    })
                    .collect();

                let question_point = self
                    .context
                    .all_movie_theaters()
                    .iter()
                    .find(|poi| poi.osm_id == *osm_poi_cinema_id);

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
                )
            }

            MatchingTarget::Hospital {
                osm_poi_hospital_id,
            } => {
                let mut other_points = self
                    .context
                    .all_hospitals()
                    .iter()
                    .filter_map(|poi| {
                        if poi.osm_id == *osm_poi_hospital_id {
                            None
                        } else {
                            Some(poi.position)
                        }
                    })
                    .collect();

                let question_point = self
                    .context
                    .all_hospitals()
                    .iter()
                    .find(|poi| poi.osm_id == *osm_poi_hospital_id);

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
                )
            }

            MatchingTarget::Library { osm_poi_library_id } => {
                let mut other_points = self
                    .context
                    .all_libraries()
                    .iter()
                    .filter_map(|poi| {
                        if poi.osm_id == *osm_poi_library_id {
                            None
                        } else {
                            Some(poi.position)
                        }
                    })
                    .collect();

                let question_point = self
                    .context
                    .all_libraries()
                    .iter()
                    .find(|poi| poi.osm_id == *osm_poi_library_id);

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
                )
            }

            MatchingTarget::ForeignConsulate {
                osm_poi_office_diplomatic_id,
            } => {
                let mut other_points = self
                    .context
                    .all_foreign_consulates()
                    .iter()
                    .filter_map(|poi| {
                        if poi.osm_id == *osm_poi_office_diplomatic_id {
                            None
                        } else {
                            Some(poi.position)
                        }
                    })
                    .collect();

                let question_point = self
                    .context
                    .all_foreign_consulates()
                    .iter()
                    .find(|poi| poi.osm_id == *osm_poi_office_diplomatic_id);

                (
                    compiler.point_cloud(other_points),
                    compiler.point(question_point.unwrap().position),
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
