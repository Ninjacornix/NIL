use nil_hir::*;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub steps: u64,
    pub call_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            steps: 100_000,
            call_depth: 256,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Record(Type, std::sync::Arc<[Value]>),
    RecordBuffer(Type, crate::sequence::Sequence<Value>),
    Map(crate::keyed::Map),
    I64(i64),
    U64(u64),
    U128(u128),
    F64(u64),
    Bool(bool),
    Array(std::sync::Arc<[i64]>),
    Buffer(crate::sequence::Sequence<i64>),
    Bytes(crate::sequence::Sequence<u8>),
}
impl Value {
    pub fn array(values: Vec<i64>) -> Self {
        Self::Array(values.into())
    }
    fn ty(&self) -> Type {
        match self {
            Self::Record(ty, _) | Self::RecordBuffer(ty, _) => *ty,
            Self::Map(v) => v.ty(),
            Self::I64(_) => Type::I64,
            Self::U64(_) => Type::U64,
            Self::U128(_) => Type::U128,
            Self::F64(_) => Type::F64,
            Self::Bool(_) => Type::Bool,
            Self::Array(values) => Type::Array(values.len()),
            Self::Buffer(_) => Type::Buffer,
            Self::Bytes(_) => Type::Bytes,
        }
    }
    pub(crate) fn integer(&self) -> i64 {
        match self {
            Self::I64(v) => *v,
            _ => unreachable!("validated integer operand"),
        }
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        match self {
            Self::Bytes(v) => v,
            _ => unreachable!("validated byte operand"),
        }
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Bytes(v) => v.len(),
            Self::RecordBuffer(_, v) => v.len(),
            _ => self.elements().len(),
        }
    }
    pub(crate) fn capacity(&self) -> usize {
        match self {
            Self::Bytes(v) => v.capacity(),
            Self::Buffer(v) => v.capacity(),
            Self::RecordBuffer(_, v) => v.capacity(),
            _ => unreachable!("dynamic sequence"),
        }
    }
    fn identity(&self) -> Option<usize> {
        match self {
            Self::Map(v) => Some(v.identity()),
            Self::RecordBuffer(_, v) => Some(v.identity()),
            Self::Bytes(v) => Some(v.identity()),
            Self::Buffer(v) => Some(v.identity()),
            _ => None,
        }
    }
    fn boolean(&self) -> bool {
        match self {
            Self::Bool(v) => *v,
            _ => unreachable!("validated bool operand"),
        }
    }
    pub(crate) fn elements(&self) -> &[i64] {
        match self {
            Self::Array(values) => values,
            Self::Buffer(values) => values,
            _ => unreachable!("validated array operand"),
        }
    }
}
#[derive(Clone, Copy)]
struct LoopRegions<'a> {
    condition: &'a Region,
    body: &'a Region,
    finish: &'a Region,
    span: Option<Span>,
}
enum Resume<'a> {
    Return {
        function: bool,
    },
    Condition {
        regions: LoopRegions<'a>,
        state: Vec<Value>,
    },
    Body(LoopRegions<'a>),
}
struct Frame<'a> {
    instructions: &'a [Instruction],
    results: &'a [ValueId],
    span: Option<Span>,
    next: usize,
    values: Vec<Value>,
    resume: Resume<'a>,
    last_uses: Vec<Option<usize>>,
}
impl<'a> Frame<'a> {
    fn region(
        region: &'a Region,
        values: Vec<Value>,
        span: Option<Span>,
        resume: Resume<'a>,
    ) -> Self {
        let values_len = values.len();
        Self {
            instructions: &region.instructions,
            results: &region.results,
            span,
            next: 0,
            values,
            resume,
            last_uses: nil_hir::liveness::last_uses(
                &region.instructions,
                &region.results,
                values_len,
            ),
        }
    }
    fn function(function: &'a Function, values: Vec<Value>) -> Self {
        Self {
            instructions: &function.instructions,
            results: std::slice::from_ref(&function.result),
            span: function.return_span,
            next: 0,
            values,
            resume: Resume::Return { function: true },
            last_uses: nil_hir::liveness::last_uses(
                &function.instructions,
                std::slice::from_ref(&function.result),
                function.parameters.len(),
            ),
        }
    }
}
// Values outside interpreter frames (the caller's Rust arguments) are not semantic
// execution roots. Count each shared dynamic allocation once across all live frames.
fn live_bytes(frames: &mut [Frame<'_>]) -> (usize, std::collections::BTreeMap<usize, usize>) {
    let mut seen = std::collections::BTreeSet::new();
    let mut bytes = 0;
    let mut counts = std::collections::BTreeMap::new();
    fn account(
        value: &Value,
        seen: &mut std::collections::BTreeSet<usize>,
        bytes: &mut usize,
        counts: &mut std::collections::BTreeMap<usize, usize>,
    ) {
        if let Value::Record(_, fields) = value {
            for value in fields.iter() {
                account(value, seen, bytes, counts);
            }
            return;
        }
        let (pointer, size) = match value {
            Value::Map(v) => (v.identity(), v.capacity()),
            Value::RecordBuffer(Type::RecordBuffer(_, slots), v) => {
                (v.identity(), (v.capacity() + 1) * slots * 8)
            }
            Value::Buffer(v) => (v.identity(), v.capacity() * 8),
            Value::Bytes(v) => (v.identity(), v.capacity()),
            _ => return,
        };
        *counts.entry(pointer).or_default() += 1;
        if seen.insert(pointer) {
            *bytes += size + crate::application::SEQUENCE_OVERHEAD;
            if let Value::RecordBuffer(_, values) = value {
                for child in values.iter() {
                    account(child, seen, bytes, counts);
                }
            }
        }
    }
    for frame in frames {
        for (id, value) in frame.values.iter_mut().enumerate() {
            if frame.last_uses[id].is_none_or(|last| last < frame.next) {
                // Keep stable SSA positions while dropping dead Arc handles.
                *value = Value::I64(0);
            } else {
                account(value, &mut seen, &mut bytes, &mut counts);
            }
        }
        if let Resume::Condition { state, .. } = &frame.resume {
            for value in state {
                account(value, &mut seen, &mut bytes, &mut counts);
            }
        }
    }
    (bytes, counts)
}

fn error(code: &'static str, span: Option<Span>, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, Phase::Execute, span, message)
}

/// Compatibility entry for i64 function signatures. Typed values remain distinct
/// internally; booleans are never represented by arbitrary integer truthiness.
pub fn execute(
    program: &ValidatedProgram,
    entry: FunctionId,
    args: &[i64],
    limits: Limits,
) -> Result<i64, Diagnostic> {
    match execute_values(
        program,
        entry,
        &args.iter().copied().map(Value::I64).collect::<Vec<_>>(),
        limits,
    )? {
        Value::I64(value) => Ok(value),
        _ => Err(error("E007", None, "i64 entry result required")),
    }
}

/// Execute without host recursion. Each executed instruction and region/function
/// yield costs one step. Inactive regions cost nothing and cannot trap. Call depth
/// counts function frames only. Loop-state update expressions execute simultaneously.
pub fn execute_values(
    program: &ValidatedProgram,
    entry: FunctionId,
    args: &[Value],
    limits: Limits,
) -> Result<Value, Diagnostic> {
    execute_values_with_host(
        program,
        entry,
        args,
        limits,
        &mut crate::application::DeniedHost,
    )
}

pub fn execute_values_with_host(
    program: &ValidatedProgram,
    entry: FunctionId,
    args: &[Value],
    limits: Limits,
    host: &mut dyn crate::application::Host,
) -> Result<Value, Diagnostic> {
    fn validate_record_argument(
        value: &Value,
        records: &[nil_hir::RecordDefinition],
    ) -> Result<(), Diagnostic> {
        if let Value::RecordBuffer(ty, items) = value {
            let Type::RecordBuffer(id, slots) = ty else {
                return Err(error("E007", None, "invalid entry buffer type"));
            };
            nil_hir::records::validate_type(records, *ty, None)
                .map_err(|_| error("E007", None, "invalid entry buffer type"))?;
            for item in items.iter() {
                if item.ty() != Type::Record(*id, *slots) {
                    return Err(error("E007", None, "entry buffer element mismatch"));
                }
                validate_record_argument(item, records)?;
            }
        }
        if let Value::Record(ty, fields) = value {
            let definition = nil_hir::records::definition(records, *ty, None)
                .map_err(|_| error("E007", None, "invalid entry record type"))?;
            if fields.len() != definition.fields.len()
                || fields
                    .iter()
                    .zip(&definition.fields)
                    .any(|(value, field)| value.ty() != field.ty)
            {
                return Err(error("E007", None, "entry record field mismatch"));
            }
            for value in fields.iter() {
                validate_record_argument(value, records)?;
            }
        }
        Ok(())
    }
    for value in args {
        validate_record_argument(value, &program.program().records)?;
    }
    let mut allocated = 0;
    let mut admitted = std::collections::BTreeSet::new();
    fn admit(
        value: &Value,
        admitted: &mut std::collections::BTreeSet<usize>,
        allocated: &mut usize,
    ) -> Result<(), Diagnostic> {
        if let Value::Record(_, fields) = value {
            for value in fields.iter() {
                admit(value, admitted, allocated)?;
            }
            return Ok(());
        }
        let (pointer, size) = match value {
            Value::Map(v) => (v.identity(), v.capacity()),
            Value::RecordBuffer(Type::RecordBuffer(_, slots), v) => {
                (v.identity(), (v.capacity() + 1) * slots * 8)
            }
            Value::Buffer(v) => (v.identity(), v.capacity() * 8),
            Value::Bytes(v) => (v.identity(), v.capacity()),
            _ => return Ok(()),
        };
        if admitted.insert(pointer) {
            crate::application::charge(allocated, size, None)?;
            if let Value::RecordBuffer(_, items) = value {
                for child in items.iter() {
                    admit(child, admitted, allocated)?;
                }
            }
        }
        Ok(())
    }
    for arg in args {
        admit(arg, &mut admitted, &mut allocated)?;
    }
    let functions = &program.program().functions;
    let function = functions
        .get(entry.0)
        .ok_or_else(|| error("E004", None, "unknown entry function"))?;
    if args.len() != function.parameters.len() {
        return Err(error("E006", None, "entry arity mismatch")
            .mismatch(function.parameters.len(), args.len()));
    }
    for (arg, ty) in args.iter().zip(&function.parameters) {
        if arg.ty() != *ty {
            return Err(error("E007", None, "entry type mismatch"));
        }
    }
    if limits.call_depth == 0 {
        return Err(error("E008", None, "call depth limit exceeded"));
    }
    let mut frames = vec![Frame::function(
        function,
        args.iter()
            .map(|v| match v {
                Value::F64(bits) => {
                    Value::F64(crate::numeric::canonical_bits(f64::from_bits(*bits)))
                }
                _ => v.clone(),
            })
            .collect(),
    )];
    let mut calls = 1;
    let mut fuel = limits.steps;
    loop {
        let (live, root_counts) = live_bytes(&mut frames);
        allocated = live;
        let frame = frames.last_mut().expect("entry remains until final return");
        let instruction = frame.instructions.get(frame.next);
        let span = instruction.map_or(frame.span, |i| i.span);
        if fuel == 0 {
            return Err(error("E008", span, "instruction budget exhausted"));
        }
        fuel -= 1;
        if let Some(instruction) = instruction {
            frame.next += 1;
            let value = match &instruction.operation {
                Operation::PluginCall {
                    provider,
                    arguments,
                } => {
                    let values = arguments
                        .iter()
                        .map(|id| frame.values[id.0].clone())
                        .collect::<Vec<_>>();
                    execute_values_with_host(
                        provider.program(),
                        provider.entry(),
                        &values,
                        Limits {
                            steps: u64::MAX,
                            call_depth: 256,
                        },
                        host,
                    )?
                }
                Operation::Record { ty, fields } => Value::Record(
                    *ty,
                    fields
                        .iter()
                        .map(|id| frame.values[id.0].clone())
                        .collect::<Vec<_>>()
                        .into(),
                ),
                Operation::Field { record, field } => {
                    let Value::Record(_, fields) = &frame.values[record.0] else {
                        unreachable!()
                    };
                    fields[*field].clone()
                }
                Operation::UpdateField {
                    record,
                    field,
                    value,
                } => {
                    let Value::Record(ty, fields) = &frame.values[record.0] else {
                        unreachable!()
                    };
                    let mut updated = fields.to_vec();
                    updated[*field] = frame.values[value.0].clone();
                    Value::Record(*ty, updated.into())
                }
                Operation::RecordBuffer { ty, length, fill } => {
                    let n = usize::try_from(frame.values[length.0].integer())
                        .map_err(|_| error("E013", span, "invalid record buffer length"))?;
                    let Type::RecordBuffer(_, slots) = ty else {
                        unreachable!()
                    };
                    let bytes = n
                        .checked_add(1)
                        .and_then(|n| n.checked_mul(slots * 8))
                        .ok_or_else(|| error("E013", span, "dynamic allocation limit exceeded"))?;
                    crate::application::charge(&mut allocated, bytes, span)?;
                    Value::RecordBuffer(*ty, vec![frame.values[fill.0].clone(); n].into())
                }
                Operation::Intrinsic {
                    op: Intrinsic::Slice,
                    arguments,
                } if matches!(instruction.ty, Type::RecordBuffer(..)) => {
                    let Value::RecordBuffer(ty, v) = &frame.values[arguments[0].0] else {
                        unreachable!()
                    };
                    let a = usize::try_from(frame.values[arguments[1].0].integer())
                        .map_err(|_| error("E012", span, "sequence slice out of bounds"))?;
                    let n = usize::try_from(frame.values[arguments[2].0].integer())
                        .map_err(|_| error("E012", span, "sequence slice out of bounds"))?;
                    let end = a
                        .checked_add(n)
                        .filter(|end| *end <= v.len())
                        .ok_or_else(|| error("E012", span, "sequence slice out of bounds"))?;
                    let Type::RecordBuffer(_, slots) = ty else {
                        unreachable!()
                    };
                    crate::application::charge(&mut allocated, (n + 1) * slots * 8, span)?;
                    Value::RecordBuffer(*ty, v[a..end].to_vec().into())
                }
                Operation::Intrinsic {
                    op: Intrinsic::Sort,
                    arguments,
                } if matches!(instruction.ty, Type::RecordBuffer(..)) => {
                    let mode = frame.values[arguments[1].0].integer();
                    return Err(error(
                        if !(0..=1).contains(&mode) {
                            "E012"
                        } else {
                            "E018"
                        },
                        span,
                        "record buffer sort requires an ordering contract",
                    ));
                }
                Operation::Intrinsic {
                    op: Intrinsic::Concat,
                    arguments,
                } if matches!(instruction.ty, Type::RecordBuffer(..)) => {
                    let left = arguments[0];
                    let a = &frame.values[left.0];
                    let Value::RecordBuffer(ty, first) = a else {
                        unreachable!()
                    };
                    let Type::RecordBuffer(_, slots) = ty else {
                        unreachable!()
                    };
                    let Value::RecordBuffer(_, other) = &frame.values[arguments[1].0] else {
                        unreachable!()
                    };
                    let capacity = crate::collections::capacity(
                        first.capacity(),
                        first.len(),
                        other.len(),
                        other.capacity(),
                        slots * 8,
                        span,
                    )?;
                    crate::application::charge(&mut allocated, (capacity + 1) * slots * 8, span)?;
                    let ty = *ty;
                    let other = other.clone();
                    let unique = frame.last_uses[left.0] == Some(frame.next - 1)
                        && root_counts[&a.identity().unwrap()] == 1
                        && a.identity() != frame.values[arguments[1].0].identity();
                    let original = if unique {
                        std::mem::replace(&mut frame.values[left.0], Value::I64(0))
                    } else {
                        a.clone()
                    };
                    let Value::RecordBuffer(_, mut first) = original else {
                        unreachable!()
                    };
                    if unique {
                        first.append(&other, capacity);
                        Value::RecordBuffer(ty, first)
                    } else {
                        Value::RecordBuffer(ty, first.concat(&other, capacity))
                    }
                }
                Operation::RecordMap(ty) => {
                    let map = crate::keyed::Map::empty_record(*ty);
                    crate::application::charge(&mut allocated, map.capacity(), span)?;
                    Value::Map(map)
                }
                Operation::Intrinsic {
                    op: Intrinsic::Get,
                    arguments,
                } if matches!(instruction.ty, Type::Record(..)) => {
                    let Value::Map(map) = &frame.values[arguments[0].0] else {
                        unreachable!()
                    };
                    let entry = map
                        .get(frame.values[arguments[1].0].bytes())
                        .ok_or_else(|| {
                            crate::application::fault("E019", span, "missing map key")
                        })?;
                    let crate::keyed::EntryValue::Bytes(bytes) = entry else {
                        unreachable!()
                    };
                    crate::records::unpack(
                        instruction.ty,
                        &program.program().records,
                        &mut bytes.as_slice(),
                    )
                }
                Operation::Bytes(v) => {
                    crate::application::charge(&mut allocated, v.len(), span)?;
                    Value::Bytes(v.clone().into())
                }
                Operation::Intrinsic {
                    op: Intrinsic::Sort,
                    arguments,
                } => {
                    let left = arguments[0];
                    let order = frame.values[arguments[1].0].integer();
                    if !(0..=1).contains(&order) {
                        return Err(crate::application::fault(
                            "E012",
                            span,
                            "invalid sort order",
                        ));
                    }
                    let a = &frame.values[left.0];
                    if matches!(a.ty(), Type::MapRecord(..)) && order != 0 {
                        return Err(crate::application::fault(
                            "E018",
                            span,
                            "record maps sort only by key",
                        ));
                    }
                    let bytes = match a {
                        Value::Map(m) => m.capacity(),
                        Value::Buffer(v) => v.capacity() * 8,
                        Value::Bytes(v) => v.capacity(),
                        _ => unreachable!("validated sort"),
                    };
                    crate::application::charge(&mut allocated, bytes, span)?;
                    let unique = frame.last_uses[left.0] == Some(frame.next - 1)
                        && root_counts[&a.identity().unwrap()] == 1;
                    let mut value = if unique {
                        std::mem::replace(&mut frame.values[left.0], Value::I64(0))
                    } else {
                        a.clone()
                    };
                    match &mut value {
                        Value::Map(m) => m.sort(order == 1),
                        Value::Buffer(v) => v
                            .mutable()
                            .sort_unstable_by(|a, b| if order == 0 { a.cmp(b) } else { b.cmp(a) }),
                        Value::Bytes(v) => v
                            .mutable()
                            .sort_unstable_by(|a, b| if order == 0 { a.cmp(b) } else { b.cmp(a) }),
                        _ => unreachable!(),
                    }
                    value
                }
                Operation::Intrinsic {
                    op: Intrinsic::Concat,
                    arguments,
                } => {
                    let left = arguments[0];
                    let right = arguments[1];
                    let a = &frame.values[left.0];
                    let b = &frame.values[right.0];
                    let width = if matches!(a, Value::Bytes(_)) { 1 } else { 8 };
                    let capacity = crate::application::concat_capacity(
                        a.capacity(),
                        a.len(),
                        b.len(),
                        b.capacity(),
                        width,
                        span,
                    )?;
                    crate::application::charge(&mut allocated, capacity * width, span)?;
                    let unique = frame.last_uses[left.0] == Some(frame.next - 1)
                        && root_counts[&a.identity().unwrap()] == 1
                        && a.identity() != b.identity();
                    let other = b.clone();
                    let original = if unique {
                        std::mem::replace(&mut frame.values[left.0], Value::I64(0))
                    } else {
                        a.clone()
                    };
                    match (original, other) {
                        (Value::Bytes(mut a), Value::Bytes(b)) => {
                            if unique {
                                a.append(&b, capacity);
                                Value::Bytes(a)
                            } else {
                                Value::Bytes(a.concat(&b, capacity))
                            }
                        }
                        (Value::Buffer(mut a), Value::Buffer(b)) => {
                            if unique {
                                a.append(&b, capacity);
                                Value::Buffer(a)
                            } else {
                                Value::Buffer(a.concat(&b, capacity))
                            }
                        }
                        _ => unreachable!("validated concat"),
                    }
                }
                Operation::Intrinsic {
                    op: op @ (Intrinsic::Insert | Intrinsic::Put),
                    arguments,
                } => {
                    let left = arguments[0];
                    let a = &frame.values[left.0];
                    let key = frame.values[arguments[1].0].bytes().to_vec();
                    let value = match &frame.values[arguments[2].0] {
                        Value::I64(v) => crate::keyed::EntryValue::Integer(*v),
                        Value::Bytes(v) => crate::keyed::EntryValue::Bytes(v.to_vec()),
                        value @ Value::Record(..) => {
                            let mut bytes = Vec::new();
                            crate::records::pack(value, &mut bytes);
                            crate::keyed::EntryValue::Bytes(bytes)
                        }
                        _ => unreachable!("validated map value"),
                    };
                    let unique = frame.last_uses[left.0] == Some(frame.next - 1)
                        && root_counts[&a.identity().unwrap()] == 1;
                    let original = if unique {
                        std::mem::replace(&mut frame.values[left.0], Value::I64(0))
                    } else {
                        a.clone()
                    };
                    let Value::Map(mut map) = original else {
                        unreachable!()
                    };
                    map.update(&key, value, *op == Intrinsic::Insert, &mut allocated, span)?;
                    Value::Map(map)
                }
                Operation::Intrinsic { op, arguments } if op.is_numeric() => {
                    crate::numeric::intrinsic(*op, &frame.values[arguments[0].0], span)?
                }
                Operation::Intrinsic { op, arguments } => {
                    let args = arguments
                        .iter()
                        .map(|id| &frame.values[id.0])
                        .collect::<Vec<_>>();
                    crate::application::intrinsic(*op, &args, &mut allocated, host, span)?
                }
                Operation::Unsigned { value, ty } => {
                    if *ty == Type::U64 {
                        Value::U64(*value as u64)
                    } else {
                        Value::U128(*value)
                    }
                }
                Operation::Float(v) => Value::F64(*v),
                Operation::Constant(v) => Value::I64(*v),
                Operation::Boolean(v) => Value::Bool(*v),
                Operation::Array(elements) => Value::array(
                    elements
                        .iter()
                        .map(|id| frame.values[id.0].integer())
                        .collect(),
                ),
                Operation::Repeat { value, len } => {
                    Value::array(vec![frame.values[value.0].integer(); *len])
                }
                Operation::Length(array) => Value::I64(frame.values[array.0].len() as i64),
                Operation::Index { array, index } => {
                    let array = &frame.values[array.0];
                    let index = checked_index(frame.values[index.0].integer(), array.len(), span)?;
                    if let Value::RecordBuffer(_, v) = array {
                        v[index].clone()
                    } else {
                        Value::I64(match array {
                            Value::Bytes(v) => v[index] as i64,
                            _ => array.elements()[index],
                        })
                    }
                }
                Operation::Replace {
                    array,
                    index,
                    value,
                } => {
                    let index = checked_index(
                        frame.values[index.0].integer(),
                        frame.values[array.0].len(),
                        span,
                    )?;
                    if let Value::RecordBuffer(ty, v) = &frame.values[array.0] {
                        let ty = *ty;
                        let Type::RecordBuffer(_, slots) = ty else {
                            unreachable!()
                        };
                        crate::application::charge(
                            &mut allocated,
                            (v.capacity() + 1) * slots * 8,
                            span,
                        )?;
                        let replacement = frame.values[value.0].clone();
                        let original = if frame.last_uses[array.0] == Some(frame.next - 1) {
                            std::mem::replace(&mut frame.values[array.0], Value::I64(0))
                        } else {
                            frame.values[array.0].clone()
                        };
                        let Value::RecordBuffer(_, mut v) = original else {
                            unreachable!()
                        };
                        v.mutable()[index] = replacement;
                        Value::RecordBuffer(ty, v)
                    } else {
                        let value = frame.values[value.0].integer();
                        let original =
                            if matches!(frame.values[array.0], Value::Buffer(_) | Value::Bytes(_))
                                && frame.last_uses[array.0] == Some(frame.next - 1)
                            {
                                std::mem::replace(&mut frame.values[array.0], Value::I64(0))
                            } else {
                                frame.values[array.0].clone()
                            };
                        match original {
                            Value::Bytes(mut v) => {
                                if !(0..=255).contains(&value) {
                                    return Err(crate::application::fault(
                                        "E014",
                                        span,
                                        "byte value must be 0..255",
                                    ));
                                }
                                crate::application::charge(&mut allocated, v.capacity(), span)?;
                                v.mutable()[index] = value as u8;
                                Value::Bytes(v)
                            }
                            Value::Buffer(mut v) => {
                                crate::application::charge(&mut allocated, v.capacity() * 8, span)?;
                                v.mutable()[index] = value;
                                Value::Buffer(v)
                            }
                            _ => {
                                let mut next = original.elements().to_vec();
                                next[index] = value;
                                Value::array(next)
                            }
                        }
                    }
                }
                Operation::Binary { op, lhs, rhs } if instruction.ty != Type::I64 => {
                    crate::numeric::binary(*op, &frame.values[lhs.0], &frame.values[rhs.0], span)?
                }
                Operation::Compare { op, lhs, rhs }
                    if !matches!(frame.values[lhs.0], Value::I64(_)) =>
                {
                    Value::Bool(crate::numeric::compare(
                        *op,
                        &frame.values[lhs.0],
                        &frame.values[rhs.0],
                    ))
                }
                Operation::Binary { op, lhs, rhs } => {
                    let a = frame.values[lhs.0].integer();
                    let b = frame.values[rhs.0].integer();
                    let result = if program.program().arithmetic == Arithmetic::Wrapping {
                        match op {
                            BinaryOp::Add => Some(a.wrapping_add(b)),
                            BinaryOp::Sub => Some(a.wrapping_sub(b)),
                            BinaryOp::Mul => Some(a.wrapping_mul(b)),
                            BinaryOp::Div if b == 0 => None,
                            BinaryOp::Div => Some(a.wrapping_div(b)),
                        }
                    } else {
                        match op {
                            BinaryOp::Add => a.checked_add(b),
                            BinaryOp::Sub => a.checked_sub(b),
                            BinaryOp::Mul => a.checked_mul(b),
                            BinaryOp::Div => a.checked_div(b),
                        }
                    };
                    Value::I64(result.ok_or_else(|| {
                        error(
                            "E009",
                            span,
                            if *op == BinaryOp::Div && b == 0 {
                                "division by zero"
                            } else {
                                "signed integer overflow"
                            },
                        )
                    })?)
                }
                Operation::Compare { op, lhs, rhs } => {
                    let a = frame.values[lhs.0].integer();
                    let b = frame.values[rhs.0].integer();
                    Value::Bool(match op {
                        CompareOp::Eq => a == b,
                        CompareOp::Ne => a != b,
                        CompareOp::Lt => a < b,
                        CompareOp::Le => a <= b,
                        CompareOp::Gt => a > b,
                        CompareOp::Ge => a >= b,
                    })
                }
                Operation::Call {
                    function,
                    arguments,
                } => {
                    if calls >= limits.call_depth {
                        return Err(error("E008", span, "call depth limit exceeded"));
                    }
                    let values = arguments
                        .iter()
                        .map(|id| frame.values[id.0].clone())
                        .collect();
                    frames.push(Frame::function(&functions[function.0], values));
                    calls += 1;
                    continue;
                }
                Operation::If {
                    condition,
                    then_region,
                    else_region,
                } => {
                    let region = if frame.values[condition.0].boolean() {
                        then_region
                    } else {
                        else_region
                    };
                    let values = frame.values.clone();
                    frames.push(Frame::region(
                        region,
                        values,
                        span,
                        Resume::Return { function: false },
                    ));
                    continue;
                }
                Operation::Loop {
                    initial,
                    condition,
                    body,
                    finish,
                } => {
                    let state: Vec<Value> = initial
                        .iter()
                        .map(|id| frame.values[id.0].clone())
                        .collect();
                    let regions = LoopRegions {
                        condition,
                        body,
                        finish,
                        span,
                    };
                    frames.push(Frame::region(
                        condition,
                        state.clone(),
                        span,
                        Resume::Condition { regions, state },
                    ));
                    continue;
                }
            };
            frame.values.push(value);
        } else {
            let frame = frames.pop().unwrap();
            let results: Vec<Value> = frame
                .results
                .iter()
                .map(|id| frame.values[id.0].clone())
                .collect();
            match frame.resume {
                Resume::Return { function } => {
                    if function {
                        calls -= 1;
                    }
                    if let Some(parent) = frames.last_mut() {
                        parent.values.push(results[0].clone());
                    } else {
                        return Ok(results[0].clone());
                    }
                }
                Resume::Condition { regions, state } => {
                    if results[0].boolean() {
                        frames.push(Frame::region(
                            regions.body,
                            state,
                            regions.span,
                            Resume::Body(regions),
                        ));
                    } else {
                        frames.push(Frame::region(
                            regions.finish,
                            state,
                            regions.span,
                            Resume::Return { function: false },
                        ));
                    }
                }
                Resume::Body(regions) => {
                    frames.push(Frame::region(
                        regions.condition,
                        results.clone(),
                        regions.span,
                        Resume::Condition {
                            regions,
                            state: results,
                        },
                    ));
                }
            }
        }
    }
}

fn checked_index(index: i64, len: usize, span: Option<Span>) -> Result<usize, Diagnostic> {
    usize::try_from(index)
        .ok()
        .filter(|i| *i < len)
        .ok_or_else(|| error("E012", span, "array index out of bounds"))
}
