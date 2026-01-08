use std::sync::Arc;

use crate::shape::{compiler::Register, contour_texture::ContourTexture, types::Centimeters};

#[repr(u8)]
pub enum BoundaryOverlapResolution {
    /// When finding the "boundary" between overlapping 'inside' and 'outside' regions,
    /// take the 'inside' region.
    Inside = 1,

    /// When finding the "boundary" between overlapping 'inside' and 'outside' regions,
    /// take the 'outside' region.
    Outside = 2,

    /// When finding the "boundary" between overlapping 'inside' and 'outside' regions,
    /// take the midpoint between the two regions.
    Midpoint = 3,
}

#[derive(strum::EnumDiscriminants)]
#[strum_discriminants(derive(Hash))]
pub enum SdfInstruction {
    Point {
        // distance from center point
        // argument index 0
        position: geo::Point,
        output: Register,
    },
    PointCloud {
        // argument index 0
        points: Vec<geo::Point>,
        output: Register,
    },
    GreatCircle {
        // argument index 0
        point: geo::Point,
        // argument index 1
        bearing: f64,
        // argument index 2
        interior_point: geo::Point,
        output: Register,
    },
    Geodesic {
        // argument index 0
        start: geo::Point,
        // argument index 1
        end: geo::Point,
        output: Register,
    },
    GeodesicString {
        // argument index 0
        points: geo::LineString,
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
        // argument index 0
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
        // argument index 0
        overlap_resolution: BoundaryOverlapResolution,
        output: Register,
    },
    // Positive (outside) values are
    // contour values greater than the zero_value.
    // Negative (inside) values are
    // contour values less than the zero_value.
    Contour {
        texture: Arc<ContourTexture>,
        // argument index 0
        zero_value: Centimeters,
        output: Register,
    },
    LoadVdg {
        diagram: Arc<boostvoronoi::prelude::Diagram>,
        output: Register,
    },
}
