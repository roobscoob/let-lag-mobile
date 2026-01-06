use crate::shape::{
    Shape,
    compiler::{Register, SdfCompiler},
    types::Centimeters,
};

pub struct Circle {
    pub center: geo::Point,
    pub radius: Centimeters,
}

impl Circle {
    pub fn new(center: geo::Point, radius: Centimeters) -> Self {
        Circle { center, radius }
    }
}

impl Shape for Circle {
    fn build_into(&self, compiler: &mut SdfCompiler) -> Register {
        let point = compiler.point(self.center);
        let dilated = compiler.dilate(point, self.radius);

        dilated
    }
}
