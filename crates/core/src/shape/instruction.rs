use std::sync::Arc;

use crate::shape::{
    compiler::Register,
    types::{Centimeters, Position},
};

#[repr(u8)]
pub enum BoundaryOverlapResolution {
    Inside = 1,
    Outside = 2,
    Midpoint = 3,
}

pub enum SdfInstruction {
    Point {
        position: Position,
        output: Register,
    },
    PointCloud {
        points: Vec<Position>,
        output: Register,
    },
    Line {
        start: Position,
        end: Position,
        output: Register,
    },
    LineString {
        points: Vec<Position>,
        output: Register,
    },
    Union {
        shapes: Vec<Register>,
        output: Register,
    },
    Intersection {
        left: Register,
        right: Register,
        output: Register,
    },
    Subtract {
        left: Register,
        right: Register,
        output: Register,
    },
    Invert {
        input: Register,
        output: Register,
    },
    Dilate {
        input: Register,
        amount: Centimeters,
        output: Register,
    },
    Boundary {
        inside: Register,
        outside: Register,
        overlap_resolution: BoundaryOverlapResolution,
        output: Register,
    },
    LoadVdg {
        diagram: Arc<boostvoronoi::prelude::Diagram>,
        output: Register,
    },
}
