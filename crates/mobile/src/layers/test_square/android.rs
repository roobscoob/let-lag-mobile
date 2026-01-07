use std::{
    cell::Cell,
    f64::consts::{FRAC_PI_2, PI},
    sync::LazyLock,
};

use eyre::{ContextCompat, bail};
use glam::{DMat4, DQuat, DVec3, FloatExt, Vec2, dvec3, dvec4, vec3, vec4};
use glow::{HasContext, NativeBuffer, NativeProgram, NativeUniformLocation};
use khronos_egl::{DynamicInstance, EGL1_0};
use mercantile::{LngLat, XY, convert_xy};
use tracing::{debug, error, info};
use zerocopy::IntoBytes;

use crate::{
    android::gl::{GlResult, get_egl_instance},
    layers::{
        android::{CustomLayer, Parameters},
        test_square::traverse_quadtree::{Tile, TileAction, traverse_quadtree},
    },
};

/// Represents a plane in 3D space as ax + by + cz + d = 0
#[derive(Debug, Clone, Copy)]
struct Plane {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

impl Plane {
    fn new(a: f64, b: f64, c: f64, d: f64) -> Self {
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
    fn distance_to_point(&self, x: f64, y: f64, z: f64) -> f64 {
        self.a * x + self.b * y + self.c * z + self.d
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone, Copy)]
struct AABB {
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
}

impl AABB {
    fn new(min_x: f64, min_y: f64, min_z: f64, max_x: f64, max_y: f64, max_z: f64) -> Self {
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
    fn get_positive_vertex(&self, plane: &Plane) -> (f64, f64, f64) {
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
struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    /// Extract frustum planes from a combined projection-view matrix
    /// Using the Gribb-Hartmann method
    fn from_matrix(mat: &DMat4) -> Self {
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
    fn intersects_aabb(&self, aabb: &AABB) -> bool {
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

struct SimpleGraphics {
    pos_attrib: u32,
    proj_uniform: NativeUniformLocation,
    border_color_uniform: NativeUniformLocation,
    buffer: NativeBuffer,
    program: NativeProgram,
    debug_counter: Cell<u16>,
}

const TILE_SIZE: f64 = 512.0;
const SQUARE_SIZE: f32 = 1.0;
const MAX_ZOOM: u8 = 20;

impl SimpleGraphics {
    fn new(gl: &glow::Context, program: NativeProgram) -> eyre::Result<Self> {
        use glow::*;
        unsafe {
            let pos_attrib = gl
                .get_attrib_location(program, "a_pos")
                .context("no a_pos attribute")?;
            let border_color_uniform = gl
                .get_uniform_location(program, "fill_color")
                .context("no fill_color uniform")?;
            let proj_uniform = gl
                .get_uniform_location(program, "proj")
                .context("no proj uniform")?;

            // Vertices ordered for LINE_LOOP: counter-clockwise around perimeter
            static BACKGROUND: [f32; 8] = [
                0.0,
                0.0, // bottom-left
                SQUARE_SIZE,
                0.0, // bottom-right
                SQUARE_SIZE,
                SQUARE_SIZE, // top-right
                0.0,
                SQUARE_SIZE, // top-left
            ];

            let buffer = gl.create_buffer().wrap_gl()?;
            gl.bind_buffer(ARRAY_BUFFER, Some(buffer));
            gl.buffer_data_u8_slice(ARRAY_BUFFER, BACKGROUND.as_bytes(), STATIC_DRAW);

            Ok(Self {
                pos_attrib,
                proj_uniform,
                border_color_uniform,
                buffer,
                program,
                debug_counter: Cell::new(0),
            })
        }
    }

    fn render(&self, gl: &glow::Context, parameters: &Parameters) -> eyre::Result<()> {
        use glow::*;
        unsafe {
            gl.use_program(Some(self.program));
            gl.bind_buffer(ARRAY_BUFFER, Some(self.buffer));
            gl.enable_vertex_attrib_array(self.pos_attrib);
            gl.vertex_attrib_pointer_f32(self.pos_attrib, 2, FLOAT, false, 0, 0);
            gl.disable(STENCIL_TEST);
            gl.disable(DEPTH_TEST);

            // Set border color (cornflower blue at full opacity)
            gl.uniform_4_f32(
                Some(&self.border_color_uniform),
                100.0 / 255.0,
                149.0 / 255.0,
                237.0 / 255.0,
                1.0,
            );

            // Set line width for the border (adjust thickness as needed)
            gl.line_width(5.0);

            let tile_count = 2u32.pow(parameters.zoom as u32);
            let tile_scale = (parameters.zoom * -1.0).exp2().recip();

            let viewport_width = parameters.width;
            let viewport_height = parameters.height;

            let world_matrix = parameters.projection_matrix.mul_mat4(
                &glam::DMat4::from_scale_rotation_translation(
                    dvec3(tile_scale * TILE_SIZE, tile_scale * TILE_SIZE, 1.0),
                    DQuat::IDENTITY,
                    DVec3::new(0.0, 0.0, 0.0),
                ),
            );

            let draw_tile_at = |zoom: u8, x: f64, y: f64| {
                let pos = (2u32.pow(zoom as u32) as f64).recip();

                let mat = world_matrix.mul_mat4(&DMat4::from_scale_rotation_translation(
                    DVec3::new(pos, pos, 1.0),
                    DQuat::IDENTITY,
                    DVec3::new(x as f64, y as f64, 0.0),
                ));

                let mat = mat.to_cols_array().map(|v| v as f32);

                gl.uniform_matrix_4_f32_slice(Some(&self.proj_uniform), false, &mat);
                gl.draw_arrays(LINE_LOOP, 0, 4);
            };

            let zoom_level = parameters.zoom.floor() as u8;

            // Extract frustum planes from the world matrix for culling in world space
            let frustum = Frustum::from_matrix(&world_matrix);

            let tiles = traverse_quadtree(Tile::WORLD, |tile| {
                // Create AABB for this tile in world space (z=0 plane)
                let tile_aabb = AABB::new(tile.x0, tile.y0, 0.0, tile.x1, tile.y1, 0.0);

                // Frustum culling in world space - drop tiles completely outside frustum
                if !frustum.intersects_aabb(&tile_aabb) {
                    return TileAction::Drop;
                }

                // Tile passed frustum culling - now check if we should subdivide based on screen area
                // Project tile corners to screen space for area calculation
                let projected_tile_corners = [
                    world_matrix.mul_vec4(dvec4(tile.x0, tile.y1, 0.0, 1.0)), // bottom-left
                    world_matrix.mul_vec4(dvec4(tile.x1, tile.y1, 0.0, 1.0)), // bottom-right
                    world_matrix.mul_vec4(dvec4(tile.x1, tile.y0, 0.0, 1.0)), // top-right
                    world_matrix.mul_vec4(dvec4(tile.x0, tile.y0, 0.0, 1.0)), // top-left
                ];

                // Check if all corners are in front of camera (w > 0)
                let all_in_front = projected_tile_corners.iter().all(|v| v.w > 0.0);

                if !all_in_front {
                    // Some corners behind camera - can't reliably compute area, so subdivide
                    return TileAction::Enter;
                }

                // All corners in front - compute normalized screen space coordinates (0..1)
                let tile_corners: [(f64, f64); 4] = projected_tile_corners
                    .map(|v| (((v.x / v.w) / 2.0) + 0.5, ((v.y / v.w) / 2.0) + 0.5));

                // Measure edge lengths in screen pixel space
                // Corners are ordered: bottom-left, bottom-right, top-right, top-left
                let edge_lengths = [
                    Self::screen_distance(
                        tile_corners[0],
                        tile_corners[1],
                        viewport_width,
                        viewport_height,
                    ), // bottom edge
                    Self::screen_distance(
                        tile_corners[1],
                        tile_corners[2],
                        viewport_width,
                        viewport_height,
                    ), // right edge
                    Self::screen_distance(
                        tile_corners[2],
                        tile_corners[3],
                        viewport_width,
                        viewport_height,
                    ), // top edge
                    Self::screen_distance(
                        tile_corners[3],
                        tile_corners[0],
                        viewport_width,
                        viewport_height,
                    ), // left edge
                ];

                // Get maximum horizontal span (bottom and top edges)
                let max_horizontal_pixels = edge_lengths[0].max(edge_lengths[2]);
                // Get maximum vertical span (left and right edges)
                let max_vertical_pixels = edge_lengths[1].max(edge_lengths[3]);

                // Tile texture resolution (adjust if your tiles aren't 256x256)
                const TILE_TEXTURE_SIZE: f64 = 256.0;
                // Optional: add tolerance to prevent thrashing at boundaries
                const UPSCALE_THRESHOLD: f64 = TILE_TEXTURE_SIZE * 1.0; // 1.0 = no tolerance, 1.2 = 20% tolerance

                // Subdivide if either dimension would require texture upscaling
                if (max_horizontal_pixels > UPSCALE_THRESHOLD
                    || max_vertical_pixels > UPSCALE_THRESHOLD)
                    && (tile.zoom < MAX_ZOOM)
                {
                    TileAction::Enter
                } else {
                    TileAction::Return
                }
            });

            for tile in tiles {
                draw_tile_at(tile.zoom, tile.x0, tile.y0);
            }
        }

        Ok(())
    }

    fn get_visible_tile_bounds(parameters: &Parameters, zoom_level: u8) -> [(f64, f64); 4] {
        let tile_scale = (parameters.zoom * -1.0).exp2().recip();
        let inv_proj = parameters
            .projection_matrix
            .mul_mat4(&glam::DMat4::from_scale_rotation_translation(
                dvec3(tile_scale * TILE_SIZE, tile_scale * TILE_SIZE, 1.0),
                DQuat::IDENTITY,
                DVec3::ZERO,
            ))
            .inverse();

        let ndc_corners = [
            (-1.0, -1.0), // bottom-left
            (1.0, -1.0),  // bottom-right
            (-1.0, 1.0),  // top-left
            (1.0, 1.0),   // top-right
        ];

        ndc_corners.map(|(x, y)| {
            // Unproject near and far points
            let near = inv_proj.mul_vec4(dvec4(x, y, -1.0, 1.0));
            let far = inv_proj.mul_vec4(dvec4(x, y, 1.0, 1.0));

            let near_world = (near / near.w).truncate();
            let far_world = (far / far.w).truncate();

            // Ray direction
            let ray_dir = (far_world - near_world).normalize();

            // Intersect with ground plane (z = 0)
            let t = -near_world.z / ray_dir.z;
            let intersection = near_world + ray_dir * t;

            (intersection.x, intersection.y)
        })
    }

    /// Calculate Euclidean distance between two points in screen pixel space
    fn screen_distance(
        p1: (f64, f64),
        p2: (f64, f64),
        viewport_width: f64,
        viewport_height: f64,
    ) -> f64 {
        let dx = (p2.0 - p1.0) * viewport_width;
        let dy = (p2.1 - p1.1) * viewport_height;
        (dx * dx + dy * dy).sqrt()
    }

    fn cleanup(self, gl: &glow::Context) {}
}

pub struct TestSquare {
    gl: glow::Context,
    program: Option<NativeProgram>,
    graphics: Option<SimpleGraphics>,
}

impl TestSquare {
    fn create_program(gl: &glow::Context) -> eyre::Result<NativeProgram> {
        use glow::*;
        unsafe {
            let check_compile_status = |shader: NativeShader, kind: &str| -> eyre::Result<()> {
                if !gl.get_shader_compile_status(shader) {
                    bail!("[{kind}]: {}", gl.get_shader_info_log(shader));
                }
                Ok(())
            };
            let program = gl.create_program().wrap_gl()?;
            let vertex_shader = gl.create_shader(VERTEX_SHADER).wrap_gl()?;
            let fragment_shader = gl.create_shader(FRAGMENT_SHADER).wrap_gl()?;

            gl.shader_source(
                vertex_shader,
                &format!(
                    r"#version 300 es

                    uniform highp mat4 proj;
                    uniform float zoom_level;
            
                    layout (location = 0) in vec2 a_pos;
                    void main() {{
                        gl_Position = proj * vec4(a_pos, 0.0, 1.0);
                    }}"
                ),
            );
            gl.compile_shader(vertex_shader);
            check_compile_status(vertex_shader, "vertex shader")?;
            gl.attach_shader(program, vertex_shader);

            gl.shader_source(
                fragment_shader,
                r"#version 300 es

                uniform highp vec4 fill_color;
                out highp vec4 fragColor;
                void main() {
                    fragColor = fill_color;
                }",
            );
            gl.compile_shader(fragment_shader);
            check_compile_status(fragment_shader, "fragment shader")?;
            gl.attach_shader(program, fragment_shader);

            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                bail!("[program] {}", gl.get_program_info_log(program))
            }

            Ok(program)
        }
    }
}

impl CustomLayer for TestSquare {
    fn new() -> eyre::Result<Self> {
        tracing::info!("setting up context");
        static DYNAMIC: LazyLock<DynamicInstance<EGL1_0>> = LazyLock::new(|| unsafe {
            DynamicInstance::load().expect("failed to obtain egl instance")
        });

        let gl = unsafe {
            glow::Context::from_loader_function(move |str| {
                DYNAMIC
                    .get_proc_address(str)
                    .map(|x| x as *const _)
                    .unwrap_or_default()
            })
        };

        info!("got gl context!");
        let program = Self::create_program(&gl).expect("failed to setup shader program");

        info!("prepared shader program");

        let graphics = SimpleGraphics::new(&gl, program).expect("failed to setup graphics");

        info!("graphics are up!");

        Ok(Self {
            gl,
            program: Some(program),
            graphics: Some(graphics),
        })
    }

    fn render(&mut self, parameters: &Parameters) -> eyre::Result<()> {
        let gl = &self.gl;
        let graphics = self
            .graphics
            .as_ref()
            .expect("graphics was removed prematurely");

        graphics.render(gl, parameters).expect("failed to");

        Ok(())
    }

    fn context_lost(&mut self) {
        self.program = None;
        error!("context lost...");
    }

    fn cleanup(self) {
        if let Some(graphics) = self.graphics {
            graphics.cleanup(&self.gl);
        }
    }
}
