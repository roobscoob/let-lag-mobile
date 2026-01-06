use std::sync::Arc;

use itertools::Itertools;

use crate::{
    hide_and_seek::question::{Question, context::QuestionContext},
    shape::{
        Shape,
        builtin::circle::Circle,
        compiler::{Register, SdfCompiler},
        instruction::BoundaryOverlapResolution,
        types::Centimeters,
    },
    transit::TripIdentifier,
};

pub enum TentacleTarget {
    Museum,
    Library,
    MovieTheater,
    Hospital,
    MetroLine,
    Zoo,
    Aquarium,
    AmusementPark,
}

pub struct TentacleQuestion {
    pub center: geo::Point,
    pub radius: Centimeters,
    pub target: TentacleTarget,
}

pub enum TentacleQuestionAnswer {
    OutOfRadius,
    Null,
    WithinRadius { closest_id: Arc<str> },
}

pub struct TentacleQuestionShape {
    pub question: TentacleQuestion,
    pub answer: TentacleQuestionAnswer,
    pub context: Box<dyn QuestionContext>,
}

impl Shape for TentacleQuestionShape {
    fn build_into(&self, compiler: &mut SdfCompiler) -> Register {
        let TentacleQuestionAnswer::WithinRadius { ref closest_id } = self.answer else {
            let circle = compiler.with(&Circle::new(self.question.center, self.question.radius));

            if matches!(self.answer, TentacleQuestionAnswer::Null) {
                return circle;
            }

            return compiler.invert(circle);
        };

        let (other, tentacle) = match self.question.target {
            // this will require special handling :yipee:
            TentacleTarget::MetroLine => {
                let transit = self.context.transit_context();
                let complexes = transit
                    .get_trip(&TripIdentifier::new(closest_id))
                    .unwrap()
                    .stop_events()
                    .iter()
                    .filter_map(|e| {
                        let station = transit.get_station(&e.station_id)?;
                        transit.get_complex(station.complex_id())
                    })
                    .unique_by(|v| v.id().clone())
                    .collect::<Vec<_>>();

                let all_complexes = transit.all_complexes();
                let other = all_complexes
                    .iter()
                    .filter_map(|c| {
                        (!complexes.iter().any(|cc| cc.id() == c.id()))
                            .then_some(c.center())
                    });

                let question = complexes.iter().map(|c| c.center()).collect::<Vec<_>>();

                let osp = compiler.point_cloud(other.collect());
                let qsp = compiler.point_cloud(question);

                (
                    compiler.dilate(osp, self.context.game_state().seeker_hiding_radius()),
                    compiler.dilate(qsp, self.context.game_state().seeker_hiding_radius()),
                )
            }

            TentacleTarget::Museum => {
                let other = self
                    .context
                    .get_all_pois("museum")
                    .unwrap()
                    .iter()
                    .filter_map(|v| (*v.id != **closest_id).then_some(v.position));

                let question = self
                    .context
                    .get_poi("museum", &**closest_id)
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other.collect()),
                    compiler.point(question),
                )
            }

            TentacleTarget::Library => {
                let other = self
                    .context
                    .get_all_pois("library")
                    .unwrap()
                    .iter()
                    .filter_map(|v| (*v.id != **closest_id).then_some(v.position));

                let question = self
                    .context
                    .get_poi("library", &**closest_id)
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other.collect()),
                    compiler.point(question),
                )
            }

            TentacleTarget::MovieTheater => {
                let other = self
                    .context
                    .get_all_pois("movie_theater")
                    .unwrap()
                    .iter()
                    .filter_map(|v| (*v.id != **closest_id).then_some(v.position));

                let question = self
                    .context
                    .get_poi("movie_theater", &**closest_id)
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other.collect()),
                    compiler.point(question),
                )
            }

            TentacleTarget::Hospital => {
                let other = self
                    .context
                    .get_all_pois("hospital")
                    .unwrap()
                    .iter()
                    .filter_map(|v| (*v.id != **closest_id).then_some(v.position));

                let question = self
                    .context
                    .get_poi("hospital", &**closest_id)
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other.collect()),
                    compiler.point(question),
                )
            }

            TentacleTarget::Zoo => {
                let other = self
                    .context
                    .get_all_pois("zoo")
                    .unwrap()
                    .iter()
                    .filter_map(|v| (*v.id != **closest_id).then_some(v.position));

                let question = self.context.get_poi("zoo", &**closest_id).unwrap().position;

                (
                    compiler.point_cloud(other.collect()),
                    compiler.point(question),
                )
            }

            TentacleTarget::Aquarium => {
                let other = self
                    .context
                    .get_all_pois("aquarium")
                    .unwrap()
                    .iter()
                    .filter_map(|v| (*v.id != **closest_id).then_some(v.position));

                let question = self
                    .context
                    .get_poi("aquarium", &**closest_id)
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other.collect()),
                    compiler.point(question),
                )
            }

            TentacleTarget::AmusementPark => {
                let other = self
                    .context
                    .get_all_pois("amusement_park")
                    .unwrap()
                    .iter()
                    .filter_map(|v| (*v.id != **closest_id).then_some(v.position));

                let question = self
                    .context
                    .get_poi("amusement_park", &**closest_id)
                    .unwrap()
                    .position;

                (
                    compiler.point_cloud(other.collect()),
                    compiler.point(question),
                )
            }
        };

        compiler.boundary(tentacle, other, BoundaryOverlapResolution::Inside)
    }
}

impl Question for TentacleQuestion {
    type Answer = TentacleQuestionAnswer;

    fn to_any(self) -> super::AnyQuestion {
        super::AnyQuestion::Tentacle(self)
    }

    fn to_shape(
        self,
        answer: Self::Answer,
        context: Box<dyn QuestionContext>,
    ) -> Result<Box<dyn Shape>, super::ShapeError> {
        if matches!(answer, TentacleQuestionAnswer::WithinRadius { .. }) {
            match self.target {
                TentacleTarget::MetroLine => {}

                TentacleTarget::Museum => {
                    if !context.has_poi_category("museum") {
                        return Err(super::ShapeError::missing_data("Museums"));
                    }
                }

                TentacleTarget::Library => {
                    if !context.has_poi_category("library") {
                        return Err(super::ShapeError::missing_data("Libraries"));
                    }
                }

                TentacleTarget::MovieTheater => {
                    if !context.has_poi_category("movie_theater") {
                        return Err(super::ShapeError::missing_data("Movie Theaters"));
                    }
                }

                TentacleTarget::Hospital => {
                    if !context.has_poi_category("hospital") {
                        return Err(super::ShapeError::missing_data("Hospitals"));
                    }
                }

                TentacleTarget::Zoo => {
                    if !context.has_poi_category("zoo") {
                        return Err(super::ShapeError::missing_data("Zoos"));
                    }
                }

                TentacleTarget::Aquarium => {
                    if !context.has_poi_category("aquarium") {
                        return Err(super::ShapeError::missing_data("Aquariums"));
                    }
                }

                TentacleTarget::AmusementPark => {
                    if !context.has_poi_category("amusement_park") {
                        return Err(super::ShapeError::missing_data("Amusement Parks"));
                    }
                }
            }
        }

        Ok(Box::new(TentacleQuestionShape {
            question: self,
            answer,
            context,
        }))
    }
}
