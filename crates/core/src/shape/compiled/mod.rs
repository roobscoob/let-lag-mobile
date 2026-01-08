pub mod shader;

use std::collections::HashMap;

use crate::{
    map::tile::Tile,
    shape::{
        Shape,
        compiled::shader::{
            ShaderArgument, ShaderSlot, ShapeShader, argument::IntoShaderArgument,
            cache::ShaderCache,
        },
        compiler::SdfCompiler,
        instruction::SdfInstruction,
    },
};

pub struct CompiledShape {
    compilation_id: u64,
    shader: ShapeShader,
    arguments: HashMap<ShaderSlot, Box<dyn IntoShaderArgument>>,
}

impl CompiledShape {
    pub fn compile(device: &wgpu::Device, cache: &mut ShaderCache, shape: &dyn Shape) -> Self {
        let compilation_id = rand::random();

        let mut compiler = SdfCompiler::new();
        let target = shape.build_into(&mut compiler);
        let instructions = compiler.finish();
        let shader = ShapeShader::compile(device, cache, instructions.iter(), target).unwrap();
        let mut arguments = HashMap::<ShaderSlot, Box<dyn IntoShaderArgument>>::new();

        for (i, instruction) in instructions.into_iter().enumerate() {
            match instruction {
                SdfInstruction::Point { position, .. } => {
                    let slot = ShaderSlot {
                        instruction_index: i as u8,
                        instruction_key: 0,
                    };

                    arguments.insert(slot, Box::new(position));
                }

                _ => todo!(),
            }
        }

        for argument in shader.required_slots().iter() {
            if !arguments.contains_key(argument) {
                panic!("Missing argument for slot: {:?}", argument);
            }
        }

        CompiledShape {
            compilation_id,
            shader,
            arguments,
        }
    }

    pub fn shader(&self) -> &wgpu::ShaderModule {
        &self.shader.module()
    }

    pub fn id(&self) -> u64 {
        self.compilation_id
    }

    pub fn fill_arguments(&self, buffer: &mut Vec<u8>, tile: &Tile) -> Vec<ShaderArgument> {
        let mut shader_arguments = Vec::new();

        for slot in self.shader.required_slots().iter() {
            let argument = self
                .arguments
                .get(slot)
                .expect("Missing argument for shader slot")
                .into_shader_argument(buffer, tile);

            shader_arguments.extend(argument);
        }

        shader_arguments
    }
}
