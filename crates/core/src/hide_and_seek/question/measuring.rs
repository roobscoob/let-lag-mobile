use crate::{
    hide_and_seek::question::context::QuestionContext,
    shape::{
        Shape,
        compiler::{Register, SdfCompiler},
        types::Centimeters,
    },
};

pub enum MeasuringTarget {
    CommercialAirport,
    HighSpeedRailLine,
    RailStation,
    InternationalBorder,
    FirstAdministrativeDivisionBorder,
    SecondAdministrativeDivisionBorder,
    SeaLevel,
    BodyOfWater,
    Coastline,
    Mountain,
    Park,
    AmusementPark,
    Zoo,
    Aquarium,
    GolfCourse,
    Museum,
    MovieTheater,
    Hospital,
    Library,
    ForeignConsulate,
}

pub struct MeasuringQuestion {
    pub category: MeasuringTarget,

    // sometimes this is altitude for MeasuringTarget::SeaLevel because Ben and Adam hate me personally.
    pub distance: Centimeters,
}

pub enum MeasuringQuestionAnswer {
    Closer,
    Further,
}

pub struct MeasuringQuestionShape {
    pub question: MeasuringQuestion,
    pub answer: MeasuringQuestionAnswer,
    pub context: Box<dyn QuestionContext>,
}

impl Shape for MeasuringQuestionShape {
    fn build_into(&self, compiler: &mut SdfCompiler) -> Register {
        let vdf = match self.question.category {
            // atrociously special-cased.
            MeasuringTarget::SeaLevel => {
                let contour = compiler.with_contour_texture(
                    self.context.sea_level_contour_texture().unwrap(),
                    self.question.distance,
                );

                // if they answer they're "further" from sea level, then their elevation is *greater*
                // therefore: the hider area needs to be negative where the elevation is greater than the zero_value.

                return match self.answer {
                    MeasuringQuestionAnswer::Closer => contour,
                    MeasuringQuestionAnswer::Further => compiler.invert(contour),
                };
            }

            MeasuringTarget::CommercialAirport => compiler.point_cloud(
                self.context
                    .get_all_pois("airport")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::HighSpeedRailLine => todo!(),

            MeasuringTarget::RailStation => compiler.point_cloud(
                self.context
                    .transit_context()
                    .all_complexes()
                    .iter()
                    .map(|c| c.center())
                    .collect(),
            ),

            MeasuringTarget::InternationalBorder => {
                let shape = compiler.with_vdg(
                    self.context
                        .get_all_areas_as_vdg("international_border")
                        .unwrap(),
                );

                compiler.edge(shape)
            }

            MeasuringTarget::FirstAdministrativeDivisionBorder => {
                let shape = compiler.with_vdg(
                    self.context
                        .get_all_areas_as_vdg("first_administrative_division")
                        .unwrap(),
                );

                compiler.edge(shape)
            }

            MeasuringTarget::SecondAdministrativeDivisionBorder => {
                let shape = compiler.with_vdg(
                    self.context
                        .get_all_areas_as_vdg("second_administrative_division")
                        .unwrap(),
                );

                compiler.edge(shape)
            }

            MeasuringTarget::BodyOfWater => {
                let shape =
                    compiler.with_vdg(self.context.get_all_areas_as_vdg("water_body").unwrap());

                compiler.edge(shape)
            }

            MeasuringTarget::Coastline => {
                let shape =
                    compiler.with_vdg(self.context.get_all_areas_as_vdg("landmass").unwrap());

                compiler.edge(shape)
            }

            MeasuringTarget::Mountain => compiler.point_cloud(
                self.context
                    .get_all_pois("mountain")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::Park => compiler.point_cloud(
                self.context
                    .get_all_pois("park")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::AmusementPark => compiler.point_cloud(
                self.context
                    .get_all_pois("amusement_park")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::Zoo => compiler.point_cloud(
                self.context
                    .get_all_pois("zoo")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::Aquarium => compiler.point_cloud(
                self.context
                    .get_all_pois("aquarium")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::GolfCourse => compiler.point_cloud(
                self.context
                    .get_all_pois("golf_course")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::Museum => compiler.point_cloud(
                self.context
                    .get_all_pois("museum")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::MovieTheater => compiler.point_cloud(
                self.context
                    .get_all_pois("movie_theater")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::Hospital => compiler.point_cloud(
                self.context
                    .get_all_pois("hospital")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::Library => compiler.point_cloud(
                self.context
                    .get_all_pois("library")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),

            MeasuringTarget::ForeignConsulate => compiler.point_cloud(
                self.context
                    .get_all_pois("foreign_consulate")
                    .unwrap()
                    .iter()
                    .map(|a| a.position)
                    .collect(),
            ),
        };

        let dilated = compiler.dilate(vdf, self.question.distance);

        match self.answer {
            MeasuringQuestionAnswer::Closer => dilated,
            MeasuringQuestionAnswer::Further => compiler.invert(dilated),
        }
    }
}
