use crate::hide_and_seek::question::matching::MatchingQuestion;

pub mod context;
pub mod matching;

pub enum AnyQuestion {
    Matching(MatchingQuestion),
    // Measuring(MeasuringQuestion),
    // Thermometer(ThermometerQuestion),
    // Radar(RadarQuestion),
    // Tentacle(TentacleQuestion),
    // Photo(PhotoQuestion),
}

pub trait Question {
    fn as_any(&self) -> AnyQuestion;
}

// the questions are:
// 1. Is your nearest <FIELD: CATEGORY> the same as my nearest <FIELD: CATEGORY>?
// 2. Compared to me are you closer or further from <FIELD: CATEGORY>?
// 3. I've just traveled <FIELD: DISTANCE>. Am I hotter or colder?
// 4. Are you within <FIELD: DISTANCE> of me?
// 5. Of all the <FIELD: CATEGORY> within <FIELD: DISTANCE> of me, which are you closest to?
// 6. Send a photo of <FIELD: SUBJECT>?
