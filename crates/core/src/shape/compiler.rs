use crate::shape::types::Centimeters;

#[derive(Clone, Copy)]
pub struct Register(u32);

pub struct SdfCompiler {
    instructions: Vec<super::instruction::SdfInstruction>,
    next_register: u32,
}

impl SdfCompiler {
    pub fn new() -> Self {
        SdfCompiler {
            instructions: Vec::new(),
            next_register: 0,
        }
    }

    fn allocate_register(&mut self) -> Register {
        let reg = Register(self.next_register);
        self.next_register += 1;
        reg
    }

    pub fn point(&mut self, position: geo::Point) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::Point {
                position,
                output: output,
            });

        output
    }

    pub fn point_cloud(&mut self, points: Vec<geo::Point>) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::PointCloud { points, output });

        output
    }

    pub fn great_circle(
        &mut self,
        point: geo::Point,
        bearing: f64,
        interior_point: geo::Point,
    ) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::GreatCircle {
                point,
                bearing,
                interior_point,
                output,
            });

        output
    }

    pub fn geodesic(&mut self, start: geo::Point, end: geo::Point) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::Geodesic { start, end, output });

        output
    }

    pub fn geodesic_string(&mut self, points: Vec<geo::Point>) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::GeodesicString { points, output });

        output
    }

    pub fn union(&mut self, shapes: Vec<Register>) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::Union {
                shapes,
                output: output,
            });

        output
    }

    pub fn intersection(&mut self, left: Register, right: Register) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::Intersection {
                left,
                right,
                output: output,
            });

        output
    }

    pub fn subtract(&mut self, left: Register, right: Register) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::Subtract {
                left,
                right,
                output: output,
            });

        output
    }

    pub fn invert(&mut self, input: Register) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::Invert { input, output });

        output
    }

    pub fn dilate(&mut self, input: Register, amount: super::types::Centimeters) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::Dilate {
                input,
                amount,
                output,
            });

        output
    }

    pub fn edge(&mut self, input: Register) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::Edge { input, output });

        output
    }

    pub fn boundary(
        &mut self,
        inside: Register,
        outside: Register,
        overlap_resolution: super::instruction::BoundaryOverlapResolution,
    ) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::Boundary {
                inside,
                outside,
                overlap_resolution,
                output,
            });

        output
    }

    pub fn with_vdg(
        &mut self,
        diagram: std::sync::Arc<boostvoronoi::prelude::Diagram>,
    ) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::LoadVdg { diagram, output });

        output
    }

    pub fn with_contour_texture(
        &mut self,
        texture: std::sync::Arc<super::contour_texture::ContourTexture>,
        zero_value: Centimeters,
    ) -> Register {
        let output = self.allocate_register();

        self.instructions
            .push(super::instruction::SdfInstruction::Contour {
                texture,
                zero_value,
                output,
            });

        output
    }

    pub fn with<S: super::Shape>(&mut self, shape: &S) -> Register {
        shape.build_into(self)
    }
}
