pub mod builtin;
pub mod compiler;
pub mod contour_texture;
pub mod instruction;
pub mod types;

pub trait Shape {
    fn build_into(&self, compiler: &mut compiler::SdfCompiler) -> compiler::Register;
}
