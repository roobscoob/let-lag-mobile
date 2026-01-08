use core::{future::Future, pin::Pin, task::Waker};
use std::{collections::HashMap, sync::OnceLock};

use crate::{
    android::gl::{GlResult, get_gl_context},
    layers::{
        android::CustomLayer,
        oob::{
            RENDER_SESSION,
            culling::{AABB, Frustum},
            traverse_quadtree::{TileAction, traverse_quadtree},
        },
    },
    render::thread::{RenderThread, RequestTile, StartShapeCompilation},
};
use actix::{Addr, dev::Request};
use eyre::{ContextCompat, OptionExt, WrapErr, bail};
use glam::{DMat4, DQuat, DVec3, dvec3, dvec4};
use glow::{HasContext, NativeBuffer, NativeProgram, NativeUniformLocation};
use jet_lag_core::{map::tile::Tile, shape::compiler::Register};
use pollster::FutureExt;
use wgpu_hal::gles::TextureInner;
use zerocopy::IntoBytes;

const TILE_SIZE: f64 = 512.0;
const MAX_ZOOM: u8 = 20;

enum TileEntry {
    Loaded {
        texture: wgpu::Texture,
    },
    InProgress {
        request: Pin<Box<Request<RenderThread, RequestTile>>>,
    },
}

pub struct OutOfBoundsLayer {
    pos_attrib: u32,
    proj_uniform: NativeUniformLocation,
    border_color_uniform: NativeUniformLocation,
    buffer: NativeBuffer,
    program: NativeProgram,

    active_tile_requests: HashMap<(u8, u32, u32), TileEntry>,
}

impl OutOfBoundsLayer {
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

            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);

            Ok(program)
        }
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
}

impl CustomLayer for OutOfBoundsLayer {
    fn new() -> eyre::Result<Self> {
        let gl = get_gl_context();
        let program = Self::create_program(gl).context("failed to create shader program")?;
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
                0.0, 0.0, // bottom-left
                1.0, 0.0, // bottom-right
                1.0, 1.0, // top-right
                0.0, 1.0, // top-left
            ];

            let buffer = gl.create_buffer().wrap_gl()?;
            gl.bind_buffer(ARRAY_BUFFER, Some(buffer));
            gl.buffer_data_u8_slice(ARRAY_BUFFER, BACKGROUND.as_bytes(), STATIC_DRAW);

            Ok(Self {
                program,
                pos_attrib,
                proj_uniform,
                border_color_uniform,
                buffer,

                active_tile_requests: HashMap::with_capacity(60),
            })
        }
    }

    fn render(&mut self, parameters: &crate::layers::android::Parameters) -> eyre::Result<()> {
        use glow::*;
        let gl = get_gl_context();
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

            let draw_tile_at = |zoom: u8, x: f64, y: f64, texture: glow::Texture| {
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

            static SHAPE: OnceLock<Register> = OnceLock::new();

            for tile in tiles {
                let tile: Tile = tile;
                let request_params = (tile.zoom, tile.tile_x, tile.tile_y);
                match self.active_tile_requests.get_mut(&request_params) {
                    Some(TileEntry::InProgress { request }) => {
                        match request
                            .as_mut()
                            .poll(&mut core::task::Context::from_waker(Waker::noop()))
                        {
                            core::task::Poll::Ready(texture) => {
                                let texture = texture.unwrap();
                                let _ = self
                                    .active_tile_requests
                                    .insert(request_params, TileEntry::Loaded { texture });
                            }
                            core::task::Poll::Pending => {}
                        }
                    }
                    Some(TileEntry::Loaded { texture }) => {
                        let hal_texture = texture.as_hal::<wgpu_hal::api::Gles>().unwrap();
                        let TextureInner::Texture { raw, target } = &hal_texture.inner else {
                            unreachable!(
                                "render thread created incorrect type of texture {:#?}",
                                hal_texture.inner
                            )
                        };

                        draw_tile_at(tile.zoom, tile.x0, tile.y0, *raw);
                    }
                    None => {
                        let mut guard = RENDER_SESSION.lock().unwrap();

                        let register = SHAPE.get_or_init(|| {
                            let register: Register = guard.test();
                            let thread: &Addr<RenderThread> = &guard.render_thread;
                            thread
                                .send(StartShapeCompilation { register })
                                .block_on()
                                .unwrap();
                            register
                        });

                        let thread: &Addr<RenderThread> = &guard.render_thread;

                        let request = thread.send(RequestTile {
                            x: tile.tile_x,
                            y: tile.tile_y,
                            z: tile.zoom,
                            shape: *register,
                        });

                        let sent_request = request;
                        self.active_tile_requests.insert(
                            request_params,
                            TileEntry::InProgress {
                                request: Box::pin(sent_request),
                            },
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn context_lost(&mut self) {
        todo!("handle context loss properly")
    }

    fn cleanup(self) {
        let gl = get_gl_context();
        unsafe {
            gl.delete_buffer(self.buffer);
            gl.delete_program(self.program);
        }
    }
}
