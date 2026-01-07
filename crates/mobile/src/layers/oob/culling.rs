use glam::DMat4;

/// Represents a plane in 3D space as ax + by + cz + d = 0
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

impl Plane {
    pub fn new(a: f64, b: f64, c: f64, d: f64) -> Self {
        // Normalize the plane equation
        let length = (a * a + b * b + c * c).sqrt();
        Self {
            a: a / length,
            b: b / length,
            c: c / length,
            d: d / length,
        }
    }

    /// Returns the signed distance from a point to the plane
    /// Positive = front side, Negative = back side
    pub fn distance_to_point(&self, x: f64, y: f64, z: f64) -> f64 {
        self.a * x + self.b * y + self.c * z + self.d
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone, Copy)]
pub struct AABB {
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
}

impl AABB {
    pub fn new(min_x: f64, min_y: f64, min_z: f64, max_x: f64, max_y: f64, max_z: f64) -> Self {
        Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }

    /// Get the "positive vertex" - the corner most in the direction of the plane normal
    pub fn get_positive_vertex(&self, plane: &Plane) -> (f64, f64, f64) {
        (
            if plane.a >= 0.0 {
                self.max_x
            } else {
                self.min_x
            },
            if plane.b >= 0.0 {
                self.max_y
            } else {
                self.min_y
            },
            if plane.c >= 0.0 {
                self.max_z
            } else {
                self.min_z
            },
        )
    }
}

/// Frustum defined by 6 planes (left, right, bottom, top, near, far)
#[derive(Debug)]
pub struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    /// Extract frustum planes from a combined projection-view matrix
    /// Using the Gribb-Hartmann method
    pub fn from_matrix(mat: &DMat4) -> Self {
        // Matrix is column-major, so we access it as mat.col(n).row
        // For plane extraction, we need rows of the matrix
        let m = mat.to_cols_array_2d();

        // Extract rows (transposed because glam is column-major)
        let row1 = [m[0][0], m[1][0], m[2][0], m[3][0]];
        let row2 = [m[0][1], m[1][1], m[2][1], m[3][1]];
        let row3 = [m[0][2], m[1][2], m[2][2], m[3][2]];
        let row4 = [m[0][3], m[1][3], m[2][3], m[3][3]];

        Self {
            planes: [
                // Left plane = row4 + row1
                Plane::new(
                    row4[0] + row1[0],
                    row4[1] + row1[1],
                    row4[2] + row1[2],
                    row4[3] + row1[3],
                ),
                // Right plane = row4 - row1
                Plane::new(
                    row4[0] - row1[0],
                    row4[1] - row1[1],
                    row4[2] - row1[2],
                    row4[3] - row1[3],
                ),
                // Bottom plane = row4 + row2
                Plane::new(
                    row4[0] + row2[0],
                    row4[1] + row2[1],
                    row4[2] + row2[2],
                    row4[3] + row2[3],
                ),
                // Top plane = row4 - row2
                Plane::new(
                    row4[0] - row2[0],
                    row4[1] - row2[1],
                    row4[2] - row2[2],
                    row4[3] - row2[3],
                ),
                // Near plane = row4 + row3
                Plane::new(
                    row4[0] + row3[0],
                    row4[1] + row3[1],
                    row4[2] + row3[2],
                    row4[3] + row3[3],
                ),
                // Far plane = row4 - row3
                Plane::new(
                    row4[0] - row3[0],
                    row4[1] - row3[1],
                    row4[2] - row3[2],
                    row4[3] - row3[3],
                ),
            ],
        }
    }

    /// Test if an AABB intersects the frustum using the p-vertex test
    /// Returns true if the AABB is at least partially inside the frustum
    pub fn intersects_aabb(&self, aabb: &AABB) -> bool {
        for plane in &self.planes {
            let p_vertex = aabb.get_positive_vertex(plane);

            // If the p-vertex is outside this plane, the entire AABB is outside
            if plane.distance_to_point(p_vertex.0, p_vertex.1, p_vertex.2) < 0.0 {
                return false;
            }
        }

        // All p-vertices are inside or on the planes, so AABB intersects frustum
        true
    }
}
