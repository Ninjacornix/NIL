//! Native unbounded HIR expansion of linear, flat sequence leaf expressions.
//! Arguments are already SSA values. No evaluation, trap or source span moves.
use nil_hir::{
    Function, Instruction, Intrinsic, Operation, Region, Type, ValidatedProgram, ValueId,
};

fn operands(op: &Operation) -> Option<Vec<ValueId>> {
    match op {
        Operation::Constant(_) => Some(vec![]),
        Operation::Intrinsic {
            op: Intrinsic::Buffer | Intrinsic::Bytes | Intrinsic::Concat,
            arguments,
        } => Some(arguments.clone()),
        _ => None,
    }
}
fn eligible(f: &Function) -> bool {
    if !matches!(f.result_type, Type::Buffer | Type::Bytes)
        || f.parameters.iter().filter(|t| t.is_dynamic()).count() != 1
        || f.parameters
            .iter()
            .any(|t| !matches!(t, Type::Buffer | Type::Bytes | Type::I64))
        || f.instructions.len() > 8
    {
        return false;
    }
    let mut uses = vec![0usize; f.parameters.len() + f.instructions.len()];
    for i in &f.instructions {
        let Some(ids) = operands(&i.operation) else {
            return false;
        };
        for id in ids {
            uses[id.0] += 1;
        }
    }
    uses[f.result.0] += 1;
    uses.iter().all(|n| *n == 1)
}
fn region(
    r: &Region,
    bindings: Vec<ValueId>,
    first: usize,
    candidates: &[Option<Function>],
) -> Region {
    let (instructions, map) = items(&r.instructions, bindings, first, candidates);
    Region {
        instructions,
        results: r.results.iter().map(|id| map[id.0]).collect(),
    }
}
fn items(
    input: &[Instruction],
    mut map: Vec<ValueId>,
    first: usize,
    candidates: &[Option<Function>],
) -> (Vec<Instruction>, Vec<ValueId>) {
    let mut output = vec![];
    for i in input {
        if let Operation::Call {
            function,
            arguments,
        } = &i.operation
        {
            if let Some(f) = &candidates[function.0] {
                let mut ids = arguments.iter().map(|id| map[id.0]).collect::<Vec<_>>();
                for instruction in &f.instructions {
                    let mut cloned = instruction.clone();
                    if let Operation::Intrinsic { arguments, .. } = &mut cloned.operation {
                        for id in arguments {
                            *id = ids[id.0];
                        }
                    }
                    ids.push(ValueId(first + output.len()));
                    output.push(cloned);
                }
                map.push(ids[f.result.0]);
                continue;
            }
        }
        let mut cloned = i.clone();
        let id = |v: &mut ValueId| *v = map[v.0];
        let ids = |vs: &mut Vec<ValueId>| {
            for v in vs {
                id(v);
            }
        };
        match &mut cloned.operation {
            Operation::Constant(_)
            | Operation::Unsigned { .. }
            | Operation::Float(_)
            | Operation::Boolean(_)
            | Operation::Bytes(_)
            | Operation::RecordMap(_) => {}
            Operation::RecordBuffer { length, fill, .. } => {
                id(length);
                id(fill);
            }
            Operation::Record { fields, .. } => ids(fields),
            Operation::Field { record, .. } => id(record),
            Operation::UpdateField { record, value, .. } => {
                id(record);
                id(value);
            }
            Operation::Array(v)
            | Operation::Intrinsic { arguments: v, .. }
            | Operation::Call { arguments: v, .. }
            | Operation::PluginCall { arguments: v, .. } => ids(v),
            Operation::Repeat { value, .. } | Operation::Length(value) => id(value),
            Operation::Index { array, index } => {
                id(array);
                id(index);
            }
            Operation::Replace {
                array,
                index,
                value,
            } => {
                id(array);
                id(index);
                id(value);
            }
            Operation::Binary { lhs, rhs, .. } | Operation::Compare { lhs, rhs, .. } => {
                id(lhs);
                id(rhs);
            }
            Operation::If {
                condition,
                then_region,
                else_region,
            } => {
                id(condition);
                *then_region = region(then_region, map.clone(), first + output.len(), candidates);
                *else_region = region(else_region, map.clone(), first + output.len(), candidates);
            }
            Operation::Loop {
                initial,
                condition,
                body,
                finish,
            } => {
                ids(initial);
                let bindings = (0..initial.len()).map(ValueId).collect::<Vec<_>>();
                *condition = region(condition, bindings.clone(), initial.len(), candidates);
                *body = region(body, bindings.clone(), initial.len(), candidates);
                *finish = region(finish, bindings, initial.len(), candidates);
            }
        }
        map.push(ValueId(first + output.len()));
        output.push(cloned);
    }
    (output, map)
}
pub(crate) fn expand(program: &ValidatedProgram) -> ValidatedProgram {
    let mut p = program.program().clone();
    let candidates = p
        .functions
        .iter()
        .map(|f| eligible(f).then(|| f.clone()))
        .collect::<Vec<_>>();
    for f in &mut p.functions {
        let bindings = (0..f.parameters.len()).map(ValueId).collect();
        let (instructions, ids) = items(&f.instructions, bindings, f.parameters.len(), &candidates);
        f.result = ids[f.result.0];
        f.instructions = instructions;
    }
    nil_hir::validate(p).expect("linear HIR expansion preserves validated types and scopes")
}
