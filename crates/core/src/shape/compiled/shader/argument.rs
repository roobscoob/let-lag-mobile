use crate::shape::compiled::shader::ShaderArgument;

pub trait IntoShaderArgument {
    fn into_shader_argument(self, buffer: &mut Vec<u8>, tile: u8) -> Vec<ShaderArgument>;
}

const COORD_SCALE: i32 = 10_000_000;

impl IntoShaderArgument for geo::Point {
    // TODO: tile type :)
    fn into_shader_argument(self, buffer: &mut Vec<u8>, tile: u8) -> Vec<ShaderArgument> {
        // convert into (i32, i32) where each value is the f32 * COORD_SCALE
        let x = (self.x() * COORD_SCALE as f64).round() as i32;
        let y = (self.y() * COORD_SCALE as f64).round() as i32;

        let offset = buffer.len() as u32;
        buffer.extend_from_slice(&x.to_le_bytes());
        buffer.extend_from_slice(&y.to_le_bytes());

        vec![
            ShaderArgument { offset, length: 1 },
            ShaderArgument {
                offset: offset + 8,
                length: 1,
            },
        ]
    }
}
