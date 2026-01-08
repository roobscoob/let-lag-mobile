pub mod shader;

use std::collections::HashMap;

use crate::shape::{
    Shape,
    compiled::shader::{ShaderSlot, ShapeShader, argument::IntoShaderArgument},
    compiler::SdfCompiler,
    instruction::SdfInstruction,
};

pub struct CompiledShape {
    shader: ShapeShader,
    arguments: HashMap<ShaderSlot, Box<dyn IntoShaderArgument>>,
}

impl CompiledShape {
    pub fn compile(shape: &dyn Shape) -> Self {
        let mut compiler = SdfCompiler::new();
        let target = shape.build_into(&mut compiler);
        let instructions = compiler.finish();
        let shader = ShapeShader::compile(instructions.iter(), target).unwrap();

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

        CompiledShape { shader, arguments }
    }
}
