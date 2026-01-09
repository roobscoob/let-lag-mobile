pub mod argument;
pub mod cache;
pub mod routine;

use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash, Hasher, RandomState},
    sync::LazyLock,
};

use naga::{
    Block, ScalarKind, Span, TypeInner,
    front::wgsl,
    valid::{Capabilities, ValidationFlags},
};
use strum::IntoDiscriminant;
use tracing::debug;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::shape::{
    compiled::shader::routine::{RoutineResult, point::compile_point},
    compiler::Register,
    instruction::SdfInstruction,
};

const MODULE_TEMPLATE: LazyLock<naga::Module> =
    LazyLock::new(|| wgsl::parse_str(include_str!("template.wgsl")).unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderSlot {
    pub instruction_index: u8,
    pub instruction_key: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct ShaderArgument {
    pub offset: u32,
    pub length: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct TileBounds {
    pub min_lat_deg: i32,
    pub min_lon_deg: i32,
    pub lat_span_deg: i32,
    pub lon_span_deg: i32,
}

impl Ord for ShaderSlot {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.instruction_index
            .cmp(&other.instruction_index)
            .then(self.instruction_key.cmp(&other.instruction_key))
    }
}

impl PartialOrd for ShaderSlot {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct ShapeShader {
    hash: u64,
    module: wgpu::ShaderModule,
    required_slots: Vec<ShaderSlot>,
}

impl ShapeShader {
    pub fn compile<'a>(
        device: &wgpu::Device,
        cache: &mut crate::shape::compiled::shader::cache::ShaderCache,
        instructions: impl Iterator<Item = &'a SdfInstruction>,
        result: Register,
    ) -> Result<Self, ()> {
        let mut hasher = RandomState::new().build_hasher();
        let mut module = MODULE_TEMPLATE.clone();
        let mut required_slots = Vec::new();

        let compute_handle = module
            .functions
            .iter()
            .find(|(_, f)| f.name.as_deref() == Some("compute"))
            .map(|(h, _)| h)
            .expect("compute function not found");

        let compute_function = module.functions.get_mut(compute_handle);

        compute_function.body = Block::new();

        let mut registers = HashMap::<Register, naga::Handle<naga::LocalVariable>>::new();

        for (index, instruction) in instructions.enumerate() {
            instruction.discriminant().hash(&mut hasher);

            let (output, routine) = match instruction {
                SdfInstruction::Point { output, .. } => (*output, compile_point),

                _ => unimplemented!(),
            };

            output.hash(&mut hasher);

            let RoutineResult {
                argument_len,
                variable,
            } = routine(
                &mut module,
                compute_handle,
                &registers,
                format!("i{}", index).as_str(),
            );

            for i in 0..argument_len {
                required_slots.push(ShaderSlot {
                    instruction_index: index as u8,
                    instruction_key: i as u8,
                });
            }

            registers.insert(output, variable);
        }

        let output = registers
            .get(&result)
            .expect("InvalidArgument: 'result' register not present")
            .clone();

        let compute_function = module.functions.get_mut(compute_handle);

        // Find i32 type
        let i32_type = module
            .types
            .iter()
            .find(|(_, ty)| {
                matches!(
                    ty.inner,
                    TypeInner::Scalar(naga::Scalar {
                        kind: ScalarKind::Sint,
                        width: 4
                    })
                )
            })
            .map(|(handle, _)| handle)
            .expect("i32 type not found in module");

        compute_function.result = Some(naga::FunctionResult {
            ty: i32_type,
            binding: None,
        });

        debug!(
            "{:?} vs {i32_type:?}",
            compute_function.local_variables.get_mut(output)
        );
        let pointer = compute_function
            .expressions
            .append(naga::Expression::LocalVariable(output), Span::UNDEFINED);

        let emit_start = compute_function.expressions.len();
        let output = compute_function
            .expressions
            .append(naga::Expression::Load { pointer }, Span::UNDEFINED);
        let emit_range = compute_function.expressions.range_from(emit_start);
        compute_function
            .body
            .push(naga::Statement::Emit(emit_range), Span::UNDEFINED);

        compute_function.body.push(
            naga::Statement::Return {
                value: Some(output),
            },
            Span::UNDEFINED,
        );
        debug!("{:#?}", compute_function.body);

        let hash = hasher.finish();

        naga::compact::compact(&mut module, naga::compact::KeepUnused::No);

        Ok(ShapeShader {
            hash,
            module: cache.get_or_create(hash, module, device),
            required_slots,
        })
    }

    pub fn hash(&self) -> u64 {
        self.hash
    }

    pub fn required_slots(&self) -> &Vec<ShaderSlot> {
        &self.required_slots
    }

    pub fn module(&self) -> &wgpu::ShaderModule {
        &self.module
    }
}
