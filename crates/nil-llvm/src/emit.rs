use nil_hir::*;
use std::fmt::Write;

fn ty(ty: Type) -> String {
    match ty {
        Type::I64 => "i64".into(),
        Type::Bool => "i1".into(),
        Type::Buffer | Type::Bytes => "ptr".into(),
        Type::Array(len) => format!("[{len} x i64]"),
    }
}
#[derive(Clone)]
struct Operand {
    ty: Type,
    text: String,
}
struct Block {
    lines: Vec<String>,
    terminator: Option<String>,
}
struct Builder {
    blocks: Vec<Block>,
    globals: Vec<String>,
    function_index: usize,
    arithmetic: Arithmetic,
    bounded: bool,
    current: usize,
    register: usize,
    root_count: usize,
    allocations: Vec<String>,
    invariant_lengths: std::collections::BTreeMap<String, Operand>,
    // Only immutable function parameters and identity-carried loop state enter
    // this map. Writable replacement storage is always distinct.
    readonly_arrays: std::collections::BTreeMap<String, String>,
}
struct Deferred {
    /// Body instruction -> private loop-state storage.
    nodes: std::collections::BTreeMap<usize, (String, crate::loop_storage::Node)>,
    writes: Vec<PendingWrite>,
}
struct PendingWrite {
    storage: String,
    index: Operand,
    value: Operand,
    enabled: Option<Operand>,
}
impl Builder {
    fn new(arithmetic: Arithmetic, bounded: bool) -> Self {
        Self {
            globals: vec![],
            function_index: 0,
            arithmetic,
            bounded,
            blocks: vec![Block {
                lines: vec![],
                terminator: None,
            }],
            current: 0,
            register: 0,
            root_count: 0,
            allocations: vec![],
            invariant_lengths: std::collections::BTreeMap::new(),
            readonly_arrays: std::collections::BTreeMap::new(),
        }
    }
    fn block(&mut self) -> usize {
        let id = self.blocks.len();
        self.blocks.push(Block {
            lines: vec![],
            terminator: None,
        });
        id
    }
    fn line(&mut self, line: impl Into<String>) {
        self.blocks[self.current].lines.push(line.into());
    }
    fn register(&mut self) -> String {
        let n = self.register;
        self.register += 1;
        format!("%v{n}")
    }
    fn value(&mut self, ty: Type, expression: impl AsRef<str>) -> Operand {
        let text = self.register();
        self.line(format!("{text} = {}", expression.as_ref()));
        Operand { ty, text }
    }
    fn branch(&mut self, target: usize) {
        self.blocks[self.current].terminator = Some(format!("br label %b{target}"));
    }
    fn conditional(&mut self, condition: &str, yes: usize, no: usize) {
        self.blocks[self.current].terminator =
            Some(format!("br i1 {condition}, label %b{yes}, label %b{no}"));
    }
    fn span(span: Option<Span>) -> (u64, u64) {
        span.map_or((u64::MAX, u64::MAX), |s| (s.start as u64, s.end as u64))
    }
    fn tick(&mut self, span: Option<Span>) {
        if !self.bounded {
            return;
        }
        let (start, end) = Self::span(span);
        self.line(format!(
            "call void @nil_tick(ptr %ctx, i64 {start}, i64 {end})"
        ));
    }
    fn guard(&mut self, bad: &str, reason: u32, span: Option<Span>) {
        let fail = self.block();
        let next = self.block();
        self.conditional(bad, fail, next);
        self.current = fail;
        let (start, end) = Self::span(span);
        self.line(format!(
            "call void @nil_fail(i32 {reason}, i64 {start}, i64 {end})"
        ));
        self.blocks[fail].terminator = Some("unreachable".into());
        self.current = next;
    }
    fn dynamic_length(&mut self, value: &Operand) -> Operand {
        if let Some(length) = self.invariant_lengths.get(&value.text) {
            return length.clone();
        }
        self.value(
            Type::I64,
            format!("call i64 @nil_length(ptr {})", value.text),
        )
    }
    fn root_slot(&mut self) -> String {
        let slot = self.register();
        let i = self.root_count;
        self.root_count += 1;
        self.allocations.push(format!(
            "{slot} = getelementptr [$ROOT_COUNT x ptr], ptr %nil_root_slots, i64 0, i64 {i}"
        ));
        slot
    }
    fn store_root(&mut self, slot: &str, value: &str) {
        self.line(format!(
            "call void @nil_root_store(ptr {slot}, ptr {value})"
        ));
    }
    fn region(&mut self, region: &Region, inputs: &[Operand], span: Option<Span>) -> Vec<Operand> {
        self.region_deferred(region, inputs, span, None)
    }
    fn region_deferred(
        &mut self,
        region: &Region,
        inputs: &[Operand],
        span: Option<Span>,
        deferred: Option<&mut Deferred>,
    ) -> Vec<Operand> {
        let values =
            self.instructions_deferred(&region.instructions, inputs, &region.results, deferred);
        self.tick(span);
        region
            .results
            .iter()
            .map(|id| values[id.0].clone())
            .collect()
    }
    fn array(&mut self, elements: &[Operand]) -> Operand {
        let array_type = Type::Array(elements.len());
        let mut array = Operand {
            ty: array_type,
            text: "zeroinitializer".into(),
        };
        for (index, value) in elements.iter().enumerate() {
            array = self.value(
                array_type,
                format!(
                    "insertvalue {} {}, i64 {}, {index}",
                    ty(array_type),
                    array.text,
                    value.text
                ),
            );
        }
        array
    }
    fn array_storage(
        &mut self,
        array: &Operand,
        index: &Operand,
        span: Option<Span>,
        writable: bool,
    ) -> (String, String) {
        let Type::Array(len) = array.ty else {
            unreachable!("validated array")
        };
        let bad = self.value(Type::Bool, format!("icmp uge i64 {}, {len}", index.text));
        self.guard(&bad.text, 4, span);
        let cached = (!writable)
            .then(|| self.readonly_arrays.get(&array.text).cloned())
            .flatten();
        let storage = cached.unwrap_or_else(|| {
            let storage = self.register();
            self.allocations
                .push(format!("{storage} = alloca {}, align 8", ty(array.ty)));
            self.line(format!(
                "store {} {}, ptr {storage}, align 8",
                ty(array.ty),
                array.text
            ));
            storage
        });
        let pointer = self.register();
        self.line(format!(
            "{pointer} = getelementptr {}, ptr {storage}, i64 0, i64 {}",
            ty(array.ty),
            index.text
        ));
        (storage, pointer)
    }
    fn instructions(
        &mut self,
        instructions: &[Instruction],
        inputs: &[Operand],
        results: &[ValueId],
    ) -> Vec<Operand> {
        self.instructions_deferred(instructions, inputs, results, None)
    }
    fn instructions_deferred(
        &mut self,
        instructions: &[Instruction],
        inputs: &[Operand],
        results: &[ValueId],
        deferred: Option<&mut Deferred>,
    ) -> Vec<Operand> {
        self.instructions_with_roots(instructions, inputs, results, deferred, None)
    }
    fn instructions_with_roots(
        &mut self,
        instructions: &[Instruction],
        inputs: &[Operand],
        results: &[ValueId],
        mut deferred: Option<&mut Deferred>,
        retained_roots: Option<&[Option<String>]>,
    ) -> Vec<Operand> {
        let mut values = inputs.to_vec();
        let last_uses = nil_hir::liveness::last_uses(instructions, results, inputs.len());
        let mut roots = Vec::new();
        for input in inputs {
            let slot = if let Some(retained) = retained_roots {
                retained[roots.len()].clone()
            } else if matches!(input.ty, Type::Buffer | Type::Bytes)
                && last_uses[roots.len()].is_some()
            {
                let slot = self.root_slot();
                self.store_root(&slot, &input.text);
                Some(slot)
            } else {
                None
            };
            roots.push(slot);
        }
        for (position, instruction) in instructions.iter().enumerate() {
            let span = instruction.span;
            self.tick(span);
            if matches!(
                instruction.operation,
                Operation::Call { .. } | Operation::If { .. } | Operation::Loop { .. }
            ) {
                for (id, slot) in roots.iter_mut().enumerate() {
                    if last_uses[id] == Some(position) {
                        if let Some(slot) = slot.take() {
                            self.store_root(&slot, "null");
                        }
                    }
                }
            }
            if let Some((
                storage,
                crate::loop_storage::Node::Branch {
                    then_nodes,
                    else_nodes,
                },
            )) = deferred
                .as_ref()
                .and_then(|d| d.nodes.get(&position))
                .cloned()
                .filter(|_| matches!(instruction.ty, Type::Array(_)))
            {
                let Operation::If {
                    condition,
                    then_region,
                    else_region,
                } = &instruction.operation
                else {
                    unreachable!("proved branch")
                };
                let yes = self.block();
                let no = self.block();
                let join = self.block();
                self.conditional(&values[condition.0].text, yes, no);
                let make = |nodes: std::collections::BTreeMap<usize, crate::loop_storage::Node>| {
                    Deferred {
                        nodes: nodes
                            .into_iter()
                            .map(|(i, node)| (i, (storage.clone(), node)))
                            .collect(),
                        writes: vec![],
                    }
                };
                let mut then_deferred = make(then_nodes);
                let mut else_deferred = make(else_nodes);
                self.current = yes;
                self.instructions_deferred(
                    &then_region.instructions,
                    &values,
                    &then_region.results,
                    Some(&mut then_deferred),
                );
                self.tick(span);
                let yes_end = self.current;
                self.branch(join);
                self.current = no;
                self.instructions_deferred(
                    &else_region.instructions,
                    &values,
                    &else_region.results,
                    Some(&mut else_deferred),
                );
                self.tick(span);
                let no_end = self.current;
                self.branch(join);
                self.current = join;
                for (writes, selected, other) in [
                    (then_deferred.writes, yes_end, no_end),
                    (else_deferred.writes, no_end, yes_end),
                ] {
                    for write in writes {
                        // Dominating indices can be reused directly. Branch-local
                        // values need phis; false-path values are never accessed.
                        let merge = |builder: &mut Self, operand: Operand, fallback: &str| {
                            if values.iter().any(|v| v.text == operand.text)
                                || !operand.text.starts_with('%')
                            {
                                operand
                            } else {
                                builder.value(
                                    operand.ty,
                                    format!(
                                        "phi {} [ {}, %b{selected} ], [ {fallback}, %b{other} ]",
                                        ty(operand.ty),
                                        operand.text
                                    ),
                                )
                            }
                        };
                        let index = merge(self, write.index, "0");
                        let value = merge(self, write.value, "0");
                        let enabled = write.enabled.map_or_else(|| "true".to_string(), |v| v.text);
                        let enabled = self.value(
                            Type::Bool,
                            format!("phi i1 [ {enabled}, %b{selected} ], [ false, %b{other} ]"),
                        );
                        deferred.as_mut().unwrap().writes.push(PendingWrite {
                            storage: write.storage,
                            index,
                            value,
                            enabled: Some(enabled),
                        });
                    }
                }
                values.push(Operand {
                    ty: instruction.ty,
                    text: "; deferred array".into(),
                });
                roots.push(None);
                continue;
            }
            let result = match &instruction.operation {
                Operation::Bytes(bytes) => {
                    let name = format!("@nil_bytes_{}_{}", self.function_index, self.register);
                    let content = bytes
                        .iter()
                        .map(|b| format!("i8 {b}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.globals.push(format!(
                        "{name} = private constant [{} x i8] [{content}]",
                        bytes.len()
                    ));
                    let (start, end) = Self::span(span);
                    self.value(
                        Type::Bytes,
                        format!(
                            "call ptr @nil_literal(ptr {name}, i64 {}, i64 {start}, i64 {end})",
                            bytes.len()
                        ),
                    )
                }
                Operation::Intrinsic { op, arguments } => {
                    let (start, end) = Self::span(span);
                    let args = arguments
                        .iter()
                        .map(|id| format!("{} {}", ty(values[id.0].ty), values[id.0].text))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let name = match op {
                        Intrinsic::Buffer | Intrinsic::Bytes => "make",
                        Intrinsic::Concat => "concat",
                        Intrinsic::Slice => "slice",
                        Intrinsic::Format => "format",
                        Intrinsic::Parse => "parse",
                        Intrinsic::Read => "read",
                        Intrinsic::Write => "write",
                        Intrinsic::Out => "out",
                    };
                    let width = if matches!(op, Intrinsic::Buffer | Intrinsic::Bytes) {
                        format!(", i64 {}", if *op == Intrinsic::Buffer { 8 } else { 1 })
                    } else {
                        String::new()
                    };
                    self.value(
                        instruction.ty,
                        format!(
                            "call {} @nil_{name}({args}{width}, i64 {start}, i64 {end})",
                            ty(instruction.ty)
                        ),
                    )
                }
                Operation::Length(array)
                    if matches!(values[array.0].ty, Type::Buffer | Type::Bytes) =>
                {
                    self.dynamic_length(&values[array.0])
                }
                Operation::Index { array, index }
                    if matches!(values[array.0].ty, Type::Buffer | Type::Bytes) =>
                {
                    let (start, end) = Self::span(span);
                    self.value(
                        Type::I64,
                        format!(
                            "call i64 @nil_get(ptr {}, i64 {}, i64 {start}, i64 {end})",
                            values[array.0].text, values[index.0].text
                        ),
                    )
                }
                Operation::Replace {
                    array,
                    index,
                    value,
                } if matches!(values[array.0].ty, Type::Buffer | Type::Bytes) => {
                    let (start, end) = Self::span(span);
                    let reusable = last_uses[array.0] == Some(position)
                        && deferred.as_ref().is_some_and(|d| {
                            matches!(
                                d.nodes.get(&position),
                                Some((_, crate::loop_storage::Node::Replace))
                            )
                        });
                    let name = if reusable { "set_unique" } else { "set" };
                    let result = self.value(
                        instruction.ty,
                        format!(
                            "call ptr @nil_{name}(ptr {}, i64 {}, i64 {}, i64 {start}, i64 {end})",
                            values[array.0].text, values[index.0].text, values[value.0].text
                        ),
                    );
                    if let Some(length) = self.invariant_lengths.get(&values[array.0].text).cloned()
                    {
                        self.invariant_lengths.insert(result.text.clone(), length);
                    }
                    result
                }
                Operation::Array(elements) => self.array(
                    &elements
                        .iter()
                        .map(|id| values[id.0].clone())
                        .collect::<Vec<_>>(),
                ),
                Operation::Repeat { value, len } => {
                    self.array(&vec![values[value.0].clone(); *len])
                }
                Operation::Length(array) => {
                    let Type::Array(len) = values[array.0].ty else {
                        unreachable!("validated array")
                    };
                    Operand {
                        ty: Type::I64,
                        text: len.to_string(),
                    }
                }
                Operation::Index { array, index } => {
                    let (_, pointer) =
                        self.array_storage(&values[array.0], &values[index.0], span, false);
                    self.value(Type::I64, format!("load i64, ptr {pointer}, align 8"))
                }
                Operation::Replace {
                    array,
                    index,
                    value,
                } => {
                    if let Some(storage) = deferred
                        .as_ref()
                        .and_then(|d| d.nodes.get(&position))
                        .map(|(storage, _)| storage)
                        .cloned()
                    {
                        let Type::Array(len) = instruction.ty else {
                            unreachable!("validated replacement")
                        };
                        let bad = self.value(
                            Type::Bool,
                            format!("icmp uge i64 {}, {len}", values[index.0].text),
                        );
                        // Check at the original instruction, commit only after the
                        // complete body/yield. Later expressions still see old state.
                        self.guard(&bad.text, 4, span);
                        deferred.as_mut().unwrap().writes.push(PendingWrite {
                            storage,
                            index: values[index.0].clone(),
                            value: values[value.0].clone(),
                            enabled: None,
                        });
                        // The proof guarantees this value has no consumers except
                        // the planned chain/body result; it never escapes as an operand.
                        values.push(Operand {
                            ty: instruction.ty,
                            text: "; deferred array".into(),
                        });
                        roots.push(None);
                        continue;
                    }
                    let (storage, pointer) =
                        self.array_storage(&values[array.0], &values[index.0], span, true);
                    self.line(format!(
                        "store i64 {}, ptr {pointer}, align 8",
                        values[value.0].text
                    ));
                    self.value(
                        values[array.0].ty,
                        format!("load {}, ptr {storage}, align 8", ty(values[array.0].ty)),
                    )
                }
                Operation::Constant(v) => Operand {
                    ty: Type::I64,
                    text: v.to_string(),
                },
                Operation::Boolean(v) => Operand {
                    ty: Type::Bool,
                    text: v.to_string(),
                },
                Operation::Binary { op, lhs, rhs } => {
                    let a = &values[lhs.0].text;
                    let b = &values[rhs.0].text;
                    if *op == BinaryOp::Div {
                        let zero = self.value(Type::Bool, format!("icmp eq i64 {b}, 0"));
                        self.guard(&zero.text, 3, span);
                        if self.arithmetic == Arithmetic::Wrapping {
                            // MIN/-1 must never reach sdiv (LLVM poison). Negation wraps.
                            let negative_one =
                                self.value(Type::Bool, format!("icmp eq i64 {b}, -1"));
                            let neg_block = self.block();
                            let div_block = self.block();
                            let join = self.block();
                            self.conditional(&negative_one.text, neg_block, div_block);
                            // This edge handles the exceptional divisor. A likelihood hint
                            // affects layout only, never the arithmetic contract.
                            self.blocks[self.current]
                                .terminator
                                .as_mut()
                                .unwrap()
                                .push_str(", !prof !0");
                            self.current = neg_block;
                            let neg = self.value(Type::I64, format!("sub i64 0, {a}"));
                            self.branch(join);
                            self.current = div_block;
                            let div = self.value(Type::I64, format!("sdiv i64 {a}, {b}"));
                            self.branch(join);
                            self.current = join;
                            self.value(
                                Type::I64,
                                format!(
                                    "phi i64 [ {}, %b{neg_block} ], [ {}, %b{div_block} ]",
                                    neg.text, div.text
                                ),
                            )
                        } else {
                            let min = self.value(
                                Type::Bool,
                                format!("icmp eq i64 {a}, -9223372036854775808"),
                            );
                            let neg = self.value(Type::Bool, format!("icmp eq i64 {b}, -1"));
                            let overflow = self
                                .value(Type::Bool, format!("and i1 {}, {}", min.text, neg.text));
                            self.guard(&overflow.text, 2, span);
                            self.value(Type::I64, format!("sdiv i64 {a}, {b}"))
                        }
                    } else if self.arithmetic == Arithmetic::Wrapping {
                        let name = match op {
                            BinaryOp::Add => "add",
                            BinaryOp::Sub => "sub",
                            BinaryOp::Mul => "mul",
                            BinaryOp::Div => unreachable!(),
                        };
                        self.value(Type::I64, format!("{name} i64 {a}, {b}"))
                    } else {
                        let name = match op {
                            BinaryOp::Add => "sadd",
                            BinaryOp::Sub => "ssub",
                            BinaryOp::Mul => "smul",
                            BinaryOp::Div => unreachable!(),
                        };
                        let pair = self.register();
                        self.line(format!("{pair} = call {{ i64, i1 }} @llvm.{name}.with.overflow.i64(i64 {a}, i64 {b})"));
                        let result =
                            self.value(Type::I64, format!("extractvalue {{ i64, i1 }} {pair}, 0"));
                        let overflow =
                            self.value(Type::Bool, format!("extractvalue {{ i64, i1 }} {pair}, 1"));
                        self.guard(&overflow.text, 2, span);
                        result
                    }
                }
                Operation::Compare { op, lhs, rhs } => {
                    let predicate = match op {
                        CompareOp::Eq => "eq",
                        CompareOp::Ne => "ne",
                        CompareOp::Lt => "slt",
                        CompareOp::Le => "sle",
                        CompareOp::Gt => "sgt",
                        CompareOp::Ge => "sge",
                    };
                    self.value(
                        Type::Bool,
                        format!(
                            "icmp {predicate} i64 {}, {}",
                            values[lhs.0].text, values[rhs.0].text
                        ),
                    )
                }
                Operation::Call {
                    function,
                    arguments,
                } => {
                    let (start, end) = Self::span(span);
                    let mut args = format!("ptr %ctx, i64 {start}, i64 {end}");
                    for id in arguments {
                        let arg = &values[id.0];
                        write!(args, ", {} {}", ty(arg.ty), arg.text).unwrap();
                    }
                    self.value(
                        instruction.ty,
                        format!("call {} @nil_fn{}({args})", ty(instruction.ty), function.0),
                    )
                }
                Operation::If {
                    condition,
                    then_region,
                    else_region,
                } => {
                    let yes = self.block();
                    let no = self.block();
                    let join = self.block();
                    self.conditional(&values[condition.0].text, yes, no);
                    let branch_nodes = deferred
                        .as_ref()
                        .and_then(|d| d.nodes.get(&position))
                        .and_then(|(_, node)| {
                            if let crate::loop_storage::Node::Branch {
                                then_nodes,
                                else_nodes,
                            } = node
                            {
                                Some((then_nodes.clone(), else_nodes.clone()))
                            } else {
                                None
                            }
                        });
                    let make = |nodes: std::collections::BTreeMap<
                        usize,
                        crate::loop_storage::Node,
                    >| Deferred {
                        nodes: nodes
                            .into_iter()
                            .map(|(id, node)| (id, (String::new(), node)))
                            .collect(),
                        writes: vec![],
                    };
                    let mut then_deferred =
                        branch_nodes.as_ref().map(|(nodes, _)| make(nodes.clone()));
                    let mut else_deferred =
                        branch_nodes.as_ref().map(|(_, nodes)| make(nodes.clone()));
                    self.current = yes;
                    let yes_value = self
                        .region_deferred(then_region, &values, span, then_deferred.as_mut())
                        .remove(0);
                    let yes_end = self.current;
                    self.branch(join);
                    self.current = no;
                    let no_value = self
                        .region_deferred(else_region, &values, span, else_deferred.as_mut())
                        .remove(0);
                    let no_end = self.current;
                    self.branch(join);
                    self.current = join;
                    self.value(
                        instruction.ty,
                        format!(
                            "phi {} [ {}, %b{yes_end} ], [ {}, %b{no_end} ]",
                            ty(instruction.ty),
                            yes_value.text,
                            no_value.text
                        ),
                    )
                }
                Operation::Loop {
                    initial,
                    condition,
                    body,
                    finish,
                } => {
                    let initial: Vec<Operand> =
                        initial.iter().map(|id| values[id.0].clone()).collect();
                    let plans = crate::loop_storage::plans(
                        &initial.iter().map(|v| v.ty).collect::<Vec<_>>(),
                        body,
                    );
                    let mut storage_by_state = std::collections::BTreeMap::new();
                    let mut deferred = Deferred {
                        nodes: std::collections::BTreeMap::new(),
                        writes: vec![],
                    };
                    for plan in &plans {
                        let value = &initial[plan.state];
                        if matches!(value.ty, Type::Buffer | Type::Bytes) {
                            for (&position, node) in &plan.nodes {
                                deferred
                                    .nodes
                                    .insert(position, (String::new(), node.clone()));
                            }
                            continue;
                        }
                        let storage = self.register();
                        self.allocations
                            .push(format!("{storage} = alloca {}, align 8", ty(value.ty)));
                        self.line(format!(
                            "store {} {}, ptr {storage}, align 8",
                            ty(value.ty),
                            value.text
                        ));
                        for (&position, node) in &plan.nodes {
                            deferred
                                .nodes
                                .insert(position, (storage.clone(), node.clone()));
                        }
                        storage_by_state.insert(plan.state, storage);
                    }
                    // Identity state and proved replacement chains preserve length,
                    // even when alias checks select copying. Load in the preheader.
                    // Keep a root in one slot across a straight, last-use
                    // replacement chain. No call, nested region or other allocation
                    // can observe a skipped root transfer. Unproved loops retain
                    // the full shadow-stack protocol.
                    let retain_roots = crate::loop_storage::retain_roots(
                        &initial.iter().map(|v| v.ty).collect::<Vec<_>>(),
                        condition,
                        body,
                        &plans,
                    );
                    let retained_slots = if retain_roots {
                        initial
                            .iter()
                            .map(|value| {
                                if matches!(value.ty, Type::Buffer | Type::Bytes) {
                                    let slot = self.root_slot();
                                    self.store_root(&slot, &value.text);
                                    Some(slot)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    } else {
                        vec![]
                    };
                    let lengths = initial
                        .iter()
                        .enumerate()
                        .map(|(i, value)| {
                            (matches!(value.ty, Type::Buffer | Type::Bytes)
                                && (body.results[i] == ValueId(i)
                                    || plans.iter().any(|p| p.state == i)))
                            .then(|| self.dynamic_length(value))
                        })
                        .collect::<Vec<_>>();
                    let saved_lengths = self.invariant_lengths.clone();
                    let predecessor = self.current;
                    let header = self.block();
                    let body_block = self.block();
                    let exit = self.block();
                    self.branch(header);
                    self.current = header;
                    let mut state = Vec::new();
                    let mut slots = Vec::new();
                    let mut header_loads = Vec::new();
                    for (i, value) in initial.iter().enumerate() {
                        if let Some(storage) = storage_by_state.get(&i) {
                            let text = self.register();
                            header_loads.push(format!(
                                "{text} = load {}, ptr {storage}, align 8",
                                ty(value.ty)
                            ));
                            let operand = Operand { ty: value.ty, text };
                            self.readonly_arrays
                                .insert(operand.text.clone(), storage.clone());
                            slots.push(None);
                            state.push(operand);
                            continue;
                        }
                        let text = self.register();
                        if body.results[i] == ValueId(i) {
                            if let Some(storage) = self.readonly_arrays.get(&value.text).cloned() {
                                self.readonly_arrays.insert(text.clone(), storage);
                            }
                        }
                        slots.push(Some(self.blocks[header].lines.len()));
                        self.line("; phi placeholder");
                        state.push(Operand { ty: value.ty, text });
                    }
                    for load in header_loads {
                        self.line(load);
                    }
                    for (value, length) in state.iter().zip(lengths) {
                        if let Some(length) = length {
                            self.invariant_lengths.insert(value.text.clone(), length);
                        }
                    }
                    let state_roots = if retain_roots {
                        retained_slots
                    } else {
                        state
                            .iter()
                            .map(|value| {
                                if matches!(value.ty, Type::Buffer | Type::Bytes) {
                                    let slot = self.root_slot();
                                    self.store_root(&slot, &value.text);
                                    Some(slot)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    };
                    let condition_value = if retain_roots {
                        let no_roots = vec![None; state.len()];
                        let values = self.instructions_with_roots(
                            &condition.instructions,
                            &state,
                            &condition.results,
                            None,
                            Some(&no_roots),
                        );
                        self.tick(span);
                        values[condition.results[0].0].clone()
                    } else {
                        self.region(condition, &state, span).remove(0)
                    };
                    self.conditional(&condition_value.text, body_block, exit);
                    self.current = body_block;
                    if !retain_roots {
                        for slot in state_roots.iter().flatten() {
                            self.store_root(slot, "null");
                        }
                    }
                    let body_values = self.instructions_with_roots(
                        &body.instructions,
                        &state,
                        &body.results,
                        Some(&mut deferred),
                        retain_roots.then_some(state_roots.as_slice()),
                    );
                    self.tick(span);
                    let next = body
                        .results
                        .iter()
                        .map(|id| body_values[id.0].clone())
                        .collect::<Vec<_>>();
                    for write in deferred.writes {
                        let join = write.enabled.as_ref().map(|enabled| {
                            let store = self.block();
                            let join = self.block();
                            self.conditional(&enabled.text, store, join);
                            self.current = store;
                            join
                        });
                        let pointer = self.register();
                        self.line(format!(
                            "{pointer} = getelementptr i64, ptr {}, i64 {}",
                            write.storage, write.index.text
                        ));
                        self.line(format!(
                            "store i64 {}, ptr {pointer}, align 8",
                            write.value.text
                        ));
                        if let Some(join) = join {
                            self.branch(join);
                            self.current = join;
                        }
                    }
                    let backedge = self.current;
                    self.branch(header);
                    for ((slot, current), (first, next)) in
                        slots.iter().zip(&state).zip(initial.iter().zip(next))
                    {
                        if let Some(slot) = slot {
                            self.blocks[header].lines[*slot] = format!(
                                "{} = phi {} [ {}, %b{predecessor} ], [ {}, %b{backedge} ]",
                                current.text,
                                ty(current.ty),
                                first.text,
                                next.text
                            );
                        }
                    }
                    self.current = exit;
                    for slot in state_roots.iter().flatten() {
                        self.store_root(slot, "null");
                    }
                    let result = self.region(finish, &state, span).remove(0);
                    self.invariant_lengths = saved_lengths;
                    result
                }
            };
            let result_slot = if matches!(result.ty, Type::Buffer | Type::Bytes)
                && last_uses[values.len()].is_some()
            {
                let recycled = if retained_roots.is_some() {
                    if let Operation::Replace { array, .. } = instruction.operation {
                        if last_uses[array.0] == Some(position) {
                            roots[array.0].take()
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                let slot = recycled.unwrap_or_else(|| self.root_slot());
                self.store_root(&slot, &result.text);
                Some(slot)
            } else {
                None
            };
            values.push(result);
            roots.push(result_slot);
            for (id, slot) in roots.iter_mut().enumerate() {
                if last_uses[id] == Some(position) {
                    if let Some(slot) = slot.take() {
                        self.store_root(&slot, "null");
                    }
                }
            }
        }
        for (id, slot) in roots.iter().enumerate() {
            if retained_roots.is_some() && results.contains(&ValueId(id)) {
                continue;
            }
            if let Some(slot) = slot {
                self.store_root(slot, "null");
            }
        }
        values
    }
    fn function(mut self, index: usize, function: &Function) -> String {
        self.function_index = index;
        // Typed functions are private implementation details of nil_entry. Avoid
        // forcing a large aggregate C ABI across its flat tooling bridge.
        let typed = function.result_type != Type::I64
            || function.parameters.iter().any(|ty| *ty != Type::I64);
        let mut out = format!(
            "define {}{} @nil_fn{index}(ptr %ctx, i64 %call_start, i64 %call_end",
            if typed { "internal " } else { "" },
            ty(function.result_type)
        );
        let mut inputs = Vec::new();
        for (n, parameter) in function.parameters.iter().enumerate() {
            let text = format!("%p{n}");
            write!(out, ", {} {text}", ty(*parameter)).unwrap();
            inputs.push(Operand {
                ty: *parameter,
                text,
            });
        }
        out.push_str(if typed { ") alwaysinline {\n" } else { ") {\n" });
        if self.bounded {
            self.line("call void @nil_enter(ptr %ctx, i64 %call_start, i64 %call_end)");
        }
        for input in &inputs {
            if let Type::Array(_) = input.ty {
                let storage = self.register();
                self.allocations
                    .push(format!("{storage} = alloca {}, align 8", ty(input.ty)));
                self.line(format!(
                    "store {} {}, ptr {storage}, align 8",
                    ty(input.ty),
                    input.text
                ));
                self.readonly_arrays.insert(input.text.clone(), storage);
            }
        }
        let values = self.instructions(
            &function.instructions,
            &inputs,
            std::slice::from_ref(&function.result),
        );
        self.tick(function.return_span);
        if self.bounded {
            self.line("call void @nil_leave(ptr %ctx)");
        }
        if self.root_count > 0 {
            self.line("call void @nil_roots_leave(ptr %nil_root_frame)");
        }
        self.blocks[self.current].terminator = Some(format!(
            "ret {} {}",
            ty(function.result_type),
            values[function.result.0].text
        ));
        for (n, block) in self.blocks.iter().enumerate() {
            writeln!(out, "b{n}:").unwrap();
            if n == 0 {
                if self.root_count > 0 {
                    writeln!(
                        out,
                        "  %nil_root_slots = alloca [{} x ptr], align 8",
                        self.root_count
                    )
                    .unwrap();
                    writeln!(
                        out,
                        "  store [{} x ptr] zeroinitializer, ptr %nil_root_slots, align 8",
                        self.root_count
                    )
                    .unwrap();
                    writeln!(out,"  %nil_root_frame = call ptr @nil_roots_enter(ptr %nil_root_slots, i64 {})",self.root_count).unwrap();
                }
                for allocation in &self.allocations {
                    writeln!(
                        out,
                        "  {}",
                        allocation.replace("$ROOT_COUNT", &self.root_count.to_string())
                    )
                    .unwrap();
                }
            }
            for line in &block.lines {
                writeln!(out, "  {line}").unwrap();
            }
            writeln!(
                out,
                "  {}",
                block
                    .terminator
                    .as_ref()
                    .expect("all CFG blocks terminated")
            )
            .unwrap();
        }
        out.push_str("}\n\n");
        if self.globals.is_empty() {
            out
        } else {
            format!("{}\n{out}", self.globals.join("\n"))
        }
    }
}

/// Flat tooling slots isolate C entry code from LLVM aggregate calling conventions.
pub(crate) fn entry_bridge(program: &ValidatedProgram, entry: FunctionId) -> String {
    let function = &program.program().functions[entry.0];
    let mut builder = Builder::new(program.program().arithmetic, false);
    let mut slot = 0;
    let mut arguments = "ptr %ctx, i64 18446744073709551615, i64 18446744073709551615".to_string();
    for parameter in &function.parameters {
        let mut elements = vec![];
        for _ in 0..parameter.slots() {
            let pointer = builder.register();
            builder.line(format!(
                "{pointer} = getelementptr i64, ptr %args, i64 {slot}"
            ));
            elements.push(builder.value(Type::I64, format!("load i64, ptr {pointer}, align 8")));
            slot += 1;
        }
        let value = match parameter {
            Type::I64 => elements.remove(0),
            Type::Bool => {
                let raw = elements.remove(0);
                let invalid = builder.value(Type::Bool, format!("icmp ugt i64 {}, 1", raw.text));
                builder.guard(&invalid.text, 5, None);
                builder.value(Type::Bool, format!("trunc i64 {} to i1", raw.text))
            }
            Type::Array(_) => builder.array(&elements),
            Type::Buffer | Type::Bytes => builder.value(
                *parameter,
                format!("inttoptr i64 {} to ptr", elements.remove(0).text),
            ),
        };
        write!(arguments, ", {} {}", ty(*parameter), value.text).unwrap();
    }
    let result = builder.value(
        function.result_type,
        format!(
            "call {} @nil_fn{}({arguments})",
            ty(function.result_type),
            entry.0
        ),
    );
    for index in 0..function.result_type.slots() {
        let value = match function.result_type {
            Type::I64 => result.clone(),
            Type::Buffer | Type::Bytes => {
                builder.value(Type::I64, format!("ptrtoint ptr {} to i64", result.text))
            }
            Type::Bool => builder.value(Type::I64, format!("zext i1 {} to i64", result.text)),
            Type::Array(_) => builder.value(
                Type::I64,
                format!(
                    "extractvalue {} {}, {index}",
                    ty(function.result_type),
                    result.text
                ),
            ),
        };
        let pointer = builder.register();
        builder.line(format!(
            "{pointer} = getelementptr i64, ptr %out, i64 {index}"
        ));
        builder.line(format!("store i64 {}, ptr {pointer}, align 8", value.text));
    }
    builder.blocks[builder.current].terminator = Some("ret void".into());
    let mut out = "define void @nil_entry(ptr %ctx, ptr %args, ptr %out) {\n".to_string();
    for (n, block) in builder.blocks.iter().enumerate() {
        writeln!(out, "b{n}:").unwrap();
        for line in &block.lines {
            writeln!(out, "  {line}").unwrap();
        }
        writeln!(out, "  {}", block.terminator.as_ref().unwrap()).unwrap();
    }
    out.push_str("}\n");
    out
}

/// Deterministic LLVM SSA lowering; source spelling has no role in this backend.
pub fn emit_llvm(program: &ValidatedProgram) -> String {
    emit_llvm_with_instrumentation(program, crate::Instrumentation::ProfileDefault)
}

pub fn emit_llvm_with_instrumentation(
    program: &ValidatedProgram,
    instrumentation: crate::Instrumentation,
) -> String {
    let bounded = instrumentation.bounded(program);
    let mut out = if bounded {
        HELPERS.to_string()
    } else {
        HELPERS.split("define internal").next().unwrap().to_string()
    };
    if uses_application(program) {
        out.push_str(APPLICATION_HELPERS);
    }
    out.push_str("!0 = !{!\"branch_weights\", i32 1, i32 1024}\n");
    writeln!(
        out,
        "; arithmetic: {:?}; resource instrumentation: {}",
        program.program().arithmetic,
        bounded
    )
    .unwrap();
    for (index, function) in program.program().functions.iter().enumerate() {
        out.push_str(
            &Builder::new(program.program().arithmetic, bounded).function(index, function),
        );
    }
    out
}
const HELPERS: &str = r#"; NIL LLVM AOT prototype: runtime context = fuel, depth, depth limit.
%nil.ctx = type { i64, i64, i64 }
declare void @nil_fail(i32, i64, i64) noreturn
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)

define internal void @nil_tick(ptr %ctx, i64 %start, i64 %end) {
entry:
  %fuel_ptr = getelementptr %nil.ctx, ptr %ctx, i32 0, i32 0
  %fuel = load i64, ptr %fuel_ptr, align 8
  %empty = icmp eq i64 %fuel, 0
  br i1 %empty, label %fail, label %ok
fail:
  call void @nil_fail(i32 0, i64 %start, i64 %end)
  unreachable
ok:
  %next = sub i64 %fuel, 1
  store i64 %next, ptr %fuel_ptr, align 8
  ret void
}
define internal void @nil_enter(ptr %ctx, i64 %start, i64 %end) {
entry:
  %depth_ptr = getelementptr %nil.ctx, ptr %ctx, i32 0, i32 1
  %limit_ptr = getelementptr %nil.ctx, ptr %ctx, i32 0, i32 2
  %depth = load i64, ptr %depth_ptr, align 8
  %limit = load i64, ptr %limit_ptr, align 8
  %full = icmp uge i64 %depth, %limit
  br i1 %full, label %fail, label %ok
fail:
  call void @nil_fail(i32 1, i64 %start, i64 %end)
  unreachable
ok:
  %next = add i64 %depth, 1
  store i64 %next, ptr %depth_ptr, align 8
  ret void
}
define internal void @nil_leave(ptr %ctx) {
entry:
  %depth_ptr = getelementptr %nil.ctx, ptr %ctx, i32 0, i32 1
  %depth = load i64, ptr %depth_ptr, align 8
  %next = sub i64 %depth, 1
  store i64 %next, ptr %depth_ptr, align 8
  ret void
}

"#;

pub(crate) fn uses_application(program: &ValidatedProgram) -> bool {
    fn instructions(items: &[Instruction]) -> bool {
        items.iter().any(|item| {
            matches!(item.ty, Type::Bytes | Type::Buffer)
                || match &item.operation {
                    Operation::Intrinsic { .. } | Operation::Bytes(_) => true,
                    Operation::If {
                        then_region,
                        else_region,
                        ..
                    } => {
                        instructions(&then_region.instructions)
                            || instructions(&else_region.instructions)
                    }
                    Operation::Loop {
                        condition,
                        body,
                        finish,
                        ..
                    } => {
                        instructions(&condition.instructions)
                            || instructions(&body.instructions)
                            || instructions(&finish.instructions)
                    }
                    _ => false,
                }
        })
    }
    program.program().functions.iter().any(|f| {
        matches!(f.result_type, Type::Buffer | Type::Bytes)
            || f.parameters
                .iter()
                .any(|t| matches!(t, Type::Buffer | Type::Bytes))
            || instructions(&f.instructions)
    })
}
const APPLICATION_HELPERS: &str = r#"
declare void @nil_root_store(ptr, ptr)
declare ptr @nil_roots_enter(ptr, i64)
declare void @nil_roots_leave(ptr)
declare ptr @nil_literal(ptr, i64, i64, i64)
declare ptr @nil_make(i64, i64, i64, i64, i64)
declare i64 @nil_length(ptr)
declare i64 @nil_get(ptr, i64, i64, i64)
declare ptr @nil_set(ptr, i64, i64, i64, i64)
declare ptr @nil_set_unique(ptr, i64, i64, i64, i64)
declare ptr @nil_concat(ptr, ptr, i64, i64)
declare ptr @nil_slice(ptr, i64, i64, i64, i64)
declare ptr @nil_format(i64, i64, i64)
declare i64 @nil_parse(ptr, i64, i64)
declare ptr @nil_read(ptr, i64, i64)
declare i64 @nil_write(ptr, ptr, i64, i64)
declare i64 @nil_out(ptr, i64, i64)
"#;
