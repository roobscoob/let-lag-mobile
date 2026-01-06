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
    android::gl::GlResult,
    layers::android::{CustomLayer, Parameters},
};
struct SimpleGraphics {
    pos_attrib: u32,
    proj_uniform: NativeUniformLocation,
    fill_color_uniform: NativeUniformLocation,
    buffer: NativeBuffer,
    program: NativeProgram,
    debug_counter: Cell<u16>,
}

const TILE_SIZE: f64 = 1.0;
const SQUARE_SIZE: f32 = 1.0;

impl SimpleGraphics {
    fn new(gl: &glow::Context, program: NativeProgram) -> eyre::Result<Self> {
        use glow::*;
        unsafe {
            let pos_attrib = gl
                .get_attrib_location(program, "a_pos")
                .context("no a_pos attribute")?;
            let fill_color_uniform = gl
                .get_uniform_location(program, "fill_color")
                .context("no fill_color uniform")?;
            let proj_uniform = gl
                .get_uniform_location(program, "proj")
                .context("no proj uniform")?;

            static BACKGROUND: [f32; 8] = [
                0.0,
                0.0,
                SQUARE_SIZE,
                0.0,
                0.0,
                SQUARE_SIZE,
                SQUARE_SIZE,
                SQUARE_SIZE,
            ];

            let buffer = gl.create_buffer().wrap_gl()?;
            gl.bind_buffer(ARRAY_BUFFER, Some(buffer));
            gl.buffer_data_u8_slice(ARRAY_BUFFER, BACKGROUND.as_bytes(), STATIC_DRAW);

            Ok(Self {
                pos_attrib,
                proj_uniform,
                fill_color_uniform,
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
            gl.uniform_4_f32(
                Some(&self.fill_color_uniform),
                100.0 / 255.0,
                149.0 / 255.0,
                237.0 / 255.0,
                0.5,
            );

            let tile_count = 2u32.pow(parameters.zoom as u32);
            let tile_scale = (parameters.zoom * -1.0).exp2().recip();
            let world_size = TILE_SIZE * tile_count as f64;

            let world_matrix = parameters.projection_matrix.mul_mat4(
                &glam::DMat4::from_scale_rotation_translation(
                    dvec3(tile_scale * 512.0, tile_scale * 512.0, 1.0),
                    DQuat::IDENTITY,
                    DVec3::new(tile_scale, tile_scale, 1.0),
                ),
            );

            // translates from a wgs84 point to world coordinates.
            // fn translate_point(point: geo::Point) -> (f64, f64) {}

            debug!(
                "tile_count: {tile_count}, world_size: {world_size}, zoom: {}, tile_scale: {}",
                parameters.zoom, tile_scale
            );
            let draw_tile_at = |zoom: u8, x: u64, y: u64| {
                let pos = (2u32.pow(zoom as u32) as f64).recip();

                let mat = world_matrix.mul_mat4(&DMat4::from_scale_rotation_translation(
                    DVec3::new(pos, pos, 1.0),
                    DQuat::IDENTITY,
                    DVec3::new(pos * x as f64, pos * y as f64, 1.0),
                ));

                let mat = mat.to_cols_array().map(|v| v as f32);

                gl.uniform_matrix_4_f32_slice(Some(&self.proj_uniform), false, &mat);
                gl.draw_arrays(TRIANGLE_STRIP, 0, 4);
            };

            // draw_tile_at(0, 0, 0);
            // draw_tile_at(1, 0, 0);
            // draw_tile_at(2, 2, 1);
            // draw_tile_at(3, 6, 3);
            draw_tile_at(4, 3, 3);
        }

        Ok(())
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
            gl_Position = proj * vec4(a_pos, 1.0, 1.0);
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
