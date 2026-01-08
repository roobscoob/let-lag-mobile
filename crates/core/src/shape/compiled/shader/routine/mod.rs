pub mod point;

pub struct RoutineResult {
    pub argument_len: u8,
    pub variable: naga::Handle<naga::LocalVariable>,
}
