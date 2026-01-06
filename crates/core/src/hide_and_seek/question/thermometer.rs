use geo::{Bearing, InterpolatePoint};

use crate::{
    hide_and_seek::question::context::QuestionContext,
    shape::{
        Shape,
        builtin::circle::Circle,
        compiler::{Register, SdfCompiler},
        types::Centimeters,
    },
};

/// Find the great circle that is equidistant from two points on the globe.
/// Returns the midpoint and the bearing that defines the great circle.
fn find_equidistant_great_circle(p1: geo::Point, p2: geo::Point) -> (geo::Point, f64) {
    // 1. Find the midpoint along the geodesic
    let midpoint = geo::Geodesic.point_at_ratio_between(p1, p2, 0.5);

    // 2. Find the bearing from p1 to p2
    let forward_bearing = geo::Geodesic.bearing(p1, p2);

    // 3. The perpendicular bearing (add 90°)
    let perpendicular_bearing = (forward_bearing + 90.0) % 360.0;

    // Return midpoint and bearing that defines the great circle
    (midpoint, perpendicular_bearing)
}

pub struct ThermometerQuestion {
    pub start: geo::Point,
    pub end: geo::Point,
}

pub enum ThermometerQuestionAnswer {
    Hotter,
    Colder,
}

pub struct ThermometerQuestionShape {
    pub question: ThermometerQuestion,
    pub answer: ThermometerQuestionAnswer,
    pub context: Box<dyn QuestionContext>,
}

impl Shape for ThermometerQuestionShape {
    fn build_into(&self, compiler: &mut SdfCompiler) -> Register {
        let (point, bearing) =
            find_equidistant_great_circle(self.question.start, self.question.end);

        match self.answer {
            ThermometerQuestionAnswer::Hotter => {
                compiler.great_circle(point, bearing, self.question.end)
            }
            ThermometerQuestionAnswer::Colder => {
                compiler.great_circle(point, bearing, self.question.start)
            }
        }
    }
}
