use std::sync::Arc;

use crate::{
    hide_and_seek::question::context::QuestionContext,
    shape::{
        Shape,
        compiler::{Register, SdfCompiler},
        types::Centimeters,
    },
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
    WithinRadius { closest_id: Arc<str> },
}

pub struct TentacleQuestionShape {
    pub question: TentacleQuestion,
    pub answer: TentacleQuestionAnswer,
    pub context: Box<dyn QuestionContext>,
}

impl Shape for TentacleQuestionShape {
    fn build_into(&self, compiler: &mut SdfCompiler) -> Register {
        let TentacleQuestionAnswer::WithinRadius { closest_id } = self.answer else {
            let center = compiler.point(self.question.center);
            let circle = compiler.dilate(center, self.question.radius);
            return compiler.invert(circle);
        };
    }
}
