use crate::{
    hide_and_seek::question::{
        context::QuestionContext, matching::MatchingQuestion, measuring::MeasuringQuestion,
        radar::RadarQuestion, tentacle::TentacleQuestion, thermometer::ThermometerQuestion,
    },
    shape::compiler::Register,
};

pub mod context;
pub mod matching;
pub mod measuring;
pub mod radar;
pub mod tentacle;
pub mod thermometer;

pub enum AnyQuestion {
    Matching(MatchingQuestion),
    Measuring(MeasuringQuestion),
    Thermometer(ThermometerQuestion),
    Radar(RadarQuestion),
    Tentacle(TentacleQuestion),
    // Photo(PhotoQuestion),
}

pub enum ShapeErrorClass {
    Uncomputable,
    MissingData,
}

pub struct ShapeError {
    pub message: String,
    pub class: ShapeErrorClass,
}

pub trait Question {
    type Answer;

    fn as_any(&self) -> AnyQuestion;
    fn to_shape(
        &self,
        answer: Self::Answer,
        context: Box<dyn QuestionContext>,
    ) -> Result<Register, ShapeError>;
}

// the questions are:
// 1. Is your nearest <FIELD: CATEGORY> the same as my nearest <FIELD: CATEGORY>?
// 2. Compared to me are you closer or further from <FIELD: CATEGORY>?
// 3. I've just traveled <FIELD: DISTANCE>. Am I hotter or colder?
// 4. Are you within <FIELD: DISTANCE> of me?
// 5. Of all the <FIELD: CATEGORY> within <FIELD: DISTANCE> of me, which are you closest to?
// 6. Send a photo of <FIELD: SUBJECT>?
