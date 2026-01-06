use std::sync::Arc;

use crate::shape::{compiler::Register, contour_texture::ContourTexture, types::Centimeters};

#[repr(u8)]
pub enum BoundaryOverlapResolution {
    Inside = 1,
    Outside = 2,
    Midpoint = 3,
}

pub enum SdfInstruction {
    Point {
        // distance from center point
        position: geo::Point,
        output: Register,
    },
    PointCloud {
        points: Vec<geo::Point>,
        output: Register,
    },
    GreatCircle {
        point: geo::Point,
        bearing: f64,
        interior_point: geo::Point,
        output: Register,
    },
    Geodesic {
        start: geo::Point,
        end: geo::Point,
        output: Register,
    },
    GeodesicString {
        points: Vec<geo::Point>,
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
    // abs(vdf)
    Edge {
        input: Register,
        output: Register,
    },
    Boundary {
        inside: Register,
        outside: Register,
        overlap_resolution: BoundaryOverlapResolution,
        output: Register,
    },
    // Positive (outside) values are
    // contour values greater than the zero_value.
    // Negative (inside) values are
    // contour values less than the zero_value.
    Contour {
        texture: Arc<ContourTexture>,
        zero_value: Centimeters,
        output: Register,
    },
    LoadVdg {
        diagram: Arc<boostvoronoi::prelude::Diagram>,
        output: Register,
    },
}
