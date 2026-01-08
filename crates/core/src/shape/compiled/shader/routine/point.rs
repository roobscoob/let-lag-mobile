use std::collections::HashMap;

use naga::{Expression, Function, LocalVariable, Module, ScalarKind, Span, Statement, TypeInner};

use crate::shape::{compiled::shader::routine::RoutineResult, compiler::Register};

pub fn compile_point(
    module: &mut Module,
    into: naga::Handle<Function>,
    _registers: &HashMap<Register, naga::Handle<naga::LocalVariable>>,
    unique_id: &str,
) -> RoutineResult {
    // Find the point routine in the module
    let point_routine = module
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some("point"))
        .map(|(handle, _)| handle)
        .expect("point routine not found in module");

    // Get function arguments (sample, idx_ptr)
    // These are already available as FunctionArgument expressions
    let sample_expr = module.functions[into]
        .expressions
        .append(Expression::FunctionArgument(0), Span::UNDEFINED);

    let idx_ptr_expr = module.functions[into]
        .expressions
        .append(Expression::FunctionArgument(1), Span::UNDEFINED);

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

    // Call point routine
    let call_result = module.functions[into]
        .expressions
        .append(Expression::CallResult(point_routine), Span::UNDEFINED);

    module.functions[into].body.push(
        Statement::Call {
            function: point_routine,
            arguments: vec![sample_expr, idx_ptr_expr],
            result: Some(call_result),
        },
        Span::UNDEFINED,
    );

    // Create local variable for the result
    let result_var = module.functions[into].local_variables.append(
        LocalVariable {
            name: Some(format!("{}__point_distance", unique_id)),
            ty: i32_type,
            init: None,
        },
        Span::UNDEFINED,
    );

    // let emit_start = module.functions[into].expressions.len();
    let var_ptr = module.functions[into]
        .expressions
        .append(Expression::LocalVariable(result_var), Span::UNDEFINED);
    // let emit_range = module.functions[into].expressions.range_from(emit_start);

    // Store call result in variable
    module.functions[into].body.push(
        Statement::Store {
            pointer: var_ptr,
            value: call_result,
        },
        Span::UNDEFINED,
    );

    RoutineResult {
        argument_len: 1,
        variable: result_var,
    }
}
