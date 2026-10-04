mod numeric;
use nil_hir::*;
use std::fmt::Write;

fn ty(ty: Type) -> String {
    match ty {
        Type::I64 | Type::U64 => "i64".into(),
        Type::U128 => "i128".into(),
        Type::F64 => "double".into(),
        Type::Bool => "i1".into(),
        Type::Buffer | Type::Bytes | Type::MapI64 | Type::MapBytes => "ptr".into(),
        Type::Array(len) => format!("[{len} x i64]"),
        Type::Record(id, _) => format!("%nil.record{id}"),
        Type::MapRecord(..) => "ptr".into(),
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
struct Builder<'a> {
    records: &'a [RecordDefinition],
    plugins: Vec<&'a plugin::Provider>,
    root_members: std::collections::BTreeMap<String, Vec<String>>,
    summaries: Option<&'a nil_hir::borrowing::Summaries>,
    blocks: Vec<Block>,
    globals: Vec<String>,
    function_index: usize,
    arithmetic: Arithmetic,
    bounded: bool,
    current: usize,
    register: usize,
    root_count: usize,
    proven_reads: std::collections::BTreeMap<(String, String), (String, Type)>,
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
impl<'a> Builder<'a> {
    fn new(arithmetic: Arithmetic, bounded: bool) -> Self {
        Self {
            records: &[],
            plugins: vec![],
            root_members: std::collections::BTreeMap::new(),
            summaries: None,
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
            proven_reads: std::collections::BTreeMap::new(),
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
        if value == "null" {
            if let Some(members) = self.root_members.get(slot).cloned() {
                for member in members {
                    self.line(format!("call void @nil_root_store(ptr {member}, ptr null)"));
                }
                return;
            }
        }
        self.line(format!(
            "call void @nil_root_store(ptr {slot}, ptr {value})"
        ));
    }
    fn dynamic_leaves(&mut self, value: &Operand, leaves: &mut Vec<Operand>) {
        if value.ty.is_dynamic() {
            leaves.push(value.clone());
        } else if let Type::Record(id, _) = value.ty {
            let fields = self.records[id]
                .fields
                .iter()
                .map(|f| f.ty)
                .collect::<Vec<_>>();
            for (index, field_ty) in fields.into_iter().enumerate() {
                if !nil_hir::records::has_dynamic(self.records, field_ty) {
                    continue;
                }
                let field = self.value(
                    field_ty,
                    format!("extractvalue {} {}, {index}", ty(value.ty), value.text),
                );
                self.dynamic_leaves(&field, leaves);
            }
        }
    }
    fn root_for(&mut self, value: &Operand) -> Option<String> {
        let mut leaves = Vec::new();
        self.dynamic_leaves(value, &mut leaves);
        if leaves.is_empty() {
            return None;
        }
        let slots = leaves.iter().map(|_| self.root_slot()).collect::<Vec<_>>();
        let slot = slots[0].clone();
        for (slot, leaf) in slots.iter().zip(leaves) {
            self.line(format!(
                "call void @nil_root_store(ptr {slot}, ptr {})",
                leaf.text
            ));
        }
        self.root_members.insert(slot.clone(), slots);
        Some(slot)
    }
    fn store_value_root(&mut self, slot: &str, value: &Operand) {
        let mut leaves = Vec::new();
        self.dynamic_leaves(value, &mut leaves);
        let slots = self
            .root_members
            .get(slot)
            .cloned()
            .unwrap_or_else(|| vec![slot.into()]);
        assert_eq!(slots.len(), leaves.len(), "retained root layout");
        for (slot, leaf) in slots.iter().zip(leaves) {
            self.line(format!(
                "call void @nil_root_store(ptr {slot}, ptr {})",
                leaf.text
            ));
        }
    }
    fn record(&mut self, record_ty: Type, fields: &[Operand]) -> Operand {
        let mut value = Operand {
            ty: record_ty,
            text: "zeroinitializer".into(),
        };
        for (index, field) in fields.iter().enumerate() {
            value = self.value(
                record_ty,
                format!(
                    "insertvalue {} {}, {} {}, {index}",
                    ty(record_ty),
                    value.text,
                    ty(field.ty),
                    field.text
                ),
            );
        }
        value
    }
    fn words(&mut self, value: &Operand, words: &mut Vec<Operand>) {
        match value.ty {
            Type::Record(id, _) => {
                let fields = self.records[id]
                    .fields
                    .iter()
                    .map(|f| f.ty)
                    .collect::<Vec<_>>();
                for (index, field_ty) in fields.into_iter().enumerate() {
                    let field = self.value(
                        field_ty,
                        format!("extractvalue {} {}, {index}", ty(value.ty), value.text),
                    );
                    self.words(&field, words);
                }
            }
            Type::Array(n) => {
                for index in 0..n {
                    words.push(self.value(
                        Type::I64,
                        format!("extractvalue {} {}, {index}", ty(value.ty), value.text),
                    ));
                }
            }
            Type::I64 | Type::U64 => words.push(Operand {
                ty: Type::I64,
                text: value.text.clone(),
            }),
            Type::F64 => {
                words.push(self.value(Type::I64, format!("bitcast double {} to i64", value.text)))
            }
            Type::Bool => {
                words.push(self.value(Type::I64, format!("zext i1 {} to i64", value.text)))
            }
            Type::U128 => {
                words.push(self.value(Type::I64, format!("trunc i128 {} to i64", value.text)));
                let high = self.value(Type::U128, format!("lshr i128 {}, 64", value.text));
                words.push(self.value(Type::I64, format!("trunc i128 {} to i64", high.text)));
            }
            _ => unreachable!("validated scalar record map"),
        }
    }
    fn decode_words(
        &mut self,
        value_ty: Type,
        words: &mut std::collections::VecDeque<Operand>,
    ) -> Operand {
        match value_ty {
            Type::Record(id, _) => {
                let types = self.records[id]
                    .fields
                    .iter()
                    .map(|f| f.ty)
                    .collect::<Vec<_>>();
                let fields = types
                    .into_iter()
                    .map(|ty| self.decode_words(ty, words))
                    .collect::<Vec<_>>();
                self.record(value_ty, &fields)
            }
            Type::Array(n) => {
                let fields = (0..n)
                    .map(|_| words.pop_front().unwrap())
                    .collect::<Vec<_>>();
                self.array(&fields)
            }
            Type::U128 => {
                let low = words.pop_front().unwrap();
                let high = words.pop_front().unwrap();
                self.wide_join(&low, &high)
            }
            Type::F64 => {
                let v = words.pop_front().unwrap();
                self.value(value_ty, format!("bitcast i64 {} to double", v.text))
            }
            Type::Bool => {
                let v = words.pop_front().unwrap();
                self.value(value_ty, format!("trunc i64 {} to i1", v.text))
            }
            Type::I64 | Type::U64 => {
                let mut v = words.pop_front().unwrap();
                v.ty = value_ty;
                v
            }
            _ => unreachable!("validated scalar record map"),
        }
    }
    fn record_map(
        &mut self,
        op: Intrinsic,
        arguments: &[ValueId],
        values: &[Operand],
        result_ty: Type,
        unique: bool,
        span: Option<Span>,
    ) -> Operand {
        let (start, end) = Self::span(span);
        let record_ty = if op == Intrinsic::Get {
            result_ty
        } else {
            values[arguments[2].0].ty
        };
        let slots = record_ty.slots();
        let storage = self.register();
        self.allocations
            .push(format!("{storage} = alloca [{slots} x i64], align 8"));
        let map = &values[arguments[0].0].text;
        let key = &values[arguments[1].0].text;
        if op == Intrinsic::Get {
            self.line(format!("call void @nil_map_get_record(ptr {map}, ptr {key}, ptr {storage}, i64 {slots}, i64 {start}, i64 {end})"));
            let mut words = std::collections::VecDeque::new();
            for index in 0..slots {
                let ptr = self.register();
                self.line(format!(
                    "{ptr} = getelementptr i64, ptr {storage}, i64 {index}"
                ));
                words.push_back(self.value(Type::I64, format!("load i64, ptr {ptr}, align 8")));
            }
            self.decode_words(record_ty, &mut words)
        } else {
            let mut words = Vec::new();
            self.words(&values[arguments[2].0], &mut words);
            for (index, word) in words.iter().enumerate() {
                let ptr = self.register();
                self.line(format!(
                    "{ptr} = getelementptr i64, ptr {storage}, i64 {index}"
                ));
                self.line(format!("store i64 {}, ptr {ptr}, align 8", word.text));
            }
            self.value(result_ty,format!("call ptr @nil_map_update_record(ptr {map}, ptr {key}, ptr {storage}, i64 {slots}, i1 {}, i1 {unique}, i64 {start}, i64 {end})",op == Intrinsic::Insert))
        }
    }
    fn region(&mut self, region: &Region, inputs: &[Operand], span: Option<Span>) -> Vec<Operand> {
        self.region_deferred(region, inputs, span, None, false)
    }
    fn region_deferred(
        &mut self,
        region: &Region,
        inputs: &[Operand],
        span: Option<Span>,
        deferred: Option<&mut Deferred>,
        borrow_scalar: bool,
    ) -> Vec<Operand> {
        // Lazy arms may borrow independently. Other regions use the parent loop's
        // retention proof; unproved allocating loops keep their transfer protocol.
        let input_types = inputs.iter().map(|value| value.ty).collect::<Vec<_>>();
        let no_roots = vec![None; inputs.len()];
        let rootless = borrow_scalar
            && self.summaries.map_or_else(
                || nil_hir::liveness::rootless_scalar_region(region, &input_types),
                |proof| proof.region(region, &input_types),
            );
        let values = self.instructions_with_roots(
            &region.instructions,
            inputs,
            &region.results,
            deferred,
            rootless.then_some(no_roots.as_slice()),
            rootless,
        );
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
    fn instructions_deferred(
        &mut self,
        instructions: &[Instruction],
        inputs: &[Operand],
        results: &[ValueId],
        deferred: Option<&mut Deferred>,
    ) -> Vec<Operand> {
        self.instructions_with_roots(instructions, inputs, results, deferred, None, false)
    }
    fn instructions_with_roots(
        &mut self,
        instructions: &[Instruction],
        inputs: &[Operand],
        results: &[ValueId],
        mut deferred: Option<&mut Deferred>,
        retained_roots: Option<&[Option<String>]>,
        borrowing: bool,
    ) -> Vec<Operand> {
        let mut values = inputs.to_vec();
        let last_uses = nil_hir::liveness::last_uses(instructions, results, inputs.len());
        let mut roots = Vec::new();
        for input in inputs {
            let slot = if borrowing {
                None
            } else if let Some(retained) = retained_roots {
                retained[roots.len()].clone()
            } else if last_uses[roots.len()].is_some() {
                self.root_for(input)
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
                Operation::PluginCall {
                    provider,
                    arguments,
                } => {
                    let index = self
                        .plugins
                        .iter()
                        .position(|p| *p == provider.as_ref())
                        .expect("linked plugin");
                    let (start, end) = Self::span(span);
                    let mut args = format!("ptr %ctx, i64 {start}, i64 {end}");
                    for id in arguments {
                        let arg = &values[id.0];
                        write!(args, ", {} {}", ty(arg.ty), arg.text).unwrap();
                    }
                    self.value(
                        instruction.ty,
                        format!(
                            "call {} @nil_plugin{index}_fn{}({args})",
                            ty(instruction.ty),
                            provider.entry().0
                        ),
                    )
                }
                Operation::Record { ty, fields } => self.record(
                    *ty,
                    &fields
                        .iter()
                        .map(|id| values[id.0].clone())
                        .collect::<Vec<_>>(),
                ),
                Operation::Field { record, field } => self.value(
                    instruction.ty,
                    format!(
                        "extractvalue {} {}, {field}",
                        ty(values[record.0].ty),
                        values[record.0].text
                    ),
                ),
                Operation::UpdateField {
                    record,
                    field,
                    value,
                } => self.value(
                    instruction.ty,
                    format!(
                        "insertvalue {} {}, {} {}, {field}",
                        ty(values[record.0].ty),
                        values[record.0].text,
                        ty(values[value.0].ty),
                        values[value.0].text
                    ),
                ),
                Operation::RecordMap(map_ty) => {
                    let (start, end) = Self::span(span);
                    self.value(
                        *map_ty,
                        format!("call ptr @nil_map(i64 {start}, i64 {end})"),
                    )
                }
                Operation::Intrinsic { op, arguments }
                    if (*op == Intrinsic::Get && matches!(instruction.ty, Type::Record(..)))
                        || (matches!(op, Intrinsic::Insert | Intrinsic::Put)
                            && matches!(values[arguments[0].0].ty, Type::MapRecord(..))) =>
                {
                    self.record_map(
                        *op,
                        arguments,
                        &values,
                        instruction.ty,
                        last_uses[arguments[0].0] == Some(position),
                        span,
                    )
                }

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
                Operation::Unsigned { value, ty } => Operand {
                    ty: *ty,
                    text: value.to_string(),
                },
                Operation::Float(bits) => Operand {
                    ty: Type::F64,
                    text: format!("0x{bits:016X}"),
                },
                Operation::Intrinsic { op, arguments }
                    if op.is_numeric()
                        || (*op == Intrinsic::Format && values[arguments[0].0].ty != Type::I64) =>
                {
                    self.numeric(*op, &values[arguments[0].0], instruction.ty, span)
                }
                Operation::Intrinsic { op, arguments } => {
                    let (start, end) = Self::span(span);
                    if *op == Intrinsic::Sort && matches!(instruction.ty, Type::MapRecord(..)) {
                        let outside = self.value(
                            Type::Bool,
                            format!("icmp ugt i64 {}, 1", values[arguments[1].0].text),
                        );
                        self.guard(&outside.text, 4, span);
                        let invalid = self.value(
                            Type::Bool,
                            format!("icmp ne i64 {}, 0", values[arguments[1].0].text),
                        );
                        self.guard(&invalid.text, 11, span);
                    }
                    let args = arguments
                        .iter()
                        .map(|id| format!("{} {}", ty(values[id.0].ty), values[id.0].text))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let name = match op {
                        Intrinsic::Sort if last_uses[arguments[0].0] == Some(position) => {
                            "sort_unique"
                        }
                        Intrinsic::Sort => "sort",
                        Intrinsic::Map | Intrinsic::ByteMap => "map",
                        Intrinsic::Has => "map_has",
                        Intrinsic::Size => "map_size",
                        Intrinsic::Key => "map_key",
                        Intrinsic::Get if instruction.ty == Type::I64 => "map_get_int",
                        Intrinsic::Get => "map_get_bytes",
                        Intrinsic::Insert | Intrinsic::Put => {
                            let unique = last_uses[arguments[0].0] == Some(position);
                            match (*op, unique, values[arguments[0].0].ty) {
                                (Intrinsic::Insert, true, Type::MapI64) => "map_insert_unique_int",
                                (Intrinsic::Insert, false, Type::MapI64) => "map_insert_int",
                                (Intrinsic::Insert, true, _) => "map_insert_unique_bytes",
                                (Intrinsic::Insert, false, _) => "map_insert_bytes",
                                (Intrinsic::Put, true, Type::MapI64) => "map_put_unique_int",
                                (Intrinsic::Put, false, Type::MapI64) => "map_put_int",
                                (Intrinsic::Put, true, _) => "map_put_unique_bytes",
                                (Intrinsic::Put, false, _) => "map_put_bytes",
                                _ => unreachable!(),
                            }
                        }
                        Intrinsic::Buffer | Intrinsic::Bytes => "make",
                        Intrinsic::Concat if last_uses[arguments[0].0] == Some(position) => {
                            "concat_unique"
                        }
                        Intrinsic::Concat => "concat",
                        Intrinsic::Slice => "slice",
                        Intrinsic::Format => "format",
                        Intrinsic::Parse => "parse",
                        Intrinsic::ParseBuffer => "parsebuf",
                        Intrinsic::Equal => unreachable!("equality normalized to plugin call"),
                        Intrinsic::Find => "find",
                        Intrinsic::Read => "read",
                        Intrinsic::Write => "write",
                        Intrinsic::Out => "out",
                        _ => unreachable!("numeric emission handled separately"),
                    };
                    let width = if matches!(op, Intrinsic::Buffer | Intrinsic::Bytes) {
                        format!(", i64 {}", if *op == Intrinsic::Buffer { 8 } else { 1 })
                    } else if *op == Intrinsic::Sort {
                        format!(
                            ", i64 {}",
                            match instruction.ty {
                                Type::Buffer => 0,
                                Type::Bytes => 1,
                                Type::MapI64 => 2,
                                Type::MapBytes | Type::MapRecord(..) => 3,
                                _ => unreachable!(),
                            }
                        )
                    } else {
                        String::new()
                    };
                    let separator = if args.is_empty() && width.is_empty() {
                        ""
                    } else {
                        ", "
                    };
                    self.value(
                        instruction.ty,
                        format!(
                            "call {} @nil_{name}({args}{width}{separator}i64 {start}, i64 {end})",
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
                    if let Some((data, kind)) = self
                        .proven_reads
                        .get(&(values[array.0].text.clone(), values[index.0].text.clone()))
                        .cloned()
                    {
                        let element = if kind == Type::Bytes { "i8" } else { "i64" };
                        let pointer = self.register();
                        self.line(format!(
                            "{pointer} = getelementptr {element}, ptr {data}, i64 {}",
                            values[index.0].text
                        ));
                        if kind == Type::Bytes {
                            let byte = self.register();
                            self.line(format!("{byte} = load i8, ptr {pointer}, align 1"));
                            self.value(Type::I64, format!("zext i8 {byte} to i64"))
                        } else {
                            self.value(Type::I64, format!("load i64, ptr {pointer}, align 8"))
                        }
                    } else {
                        let (start, end) = Self::span(span);
                        self.value(
                            Type::I64,
                            format!(
                                "call i64 @nil_get(ptr {}, i64 {}, i64 {start}, i64 {end})",
                                values[array.0].text, values[index.0].text
                            ),
                        )
                    }
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
                Operation::Binary { op, lhs, rhs } if instruction.ty != Type::I64 => {
                    let (a, b) = (&values[lhs.0], &values[rhs.0]);
                    if instruction.ty == Type::F64 {
                        let name = match op {
                            BinaryOp::Add => "fadd",
                            BinaryOp::Sub => "fsub",
                            BinaryOp::Mul => "fmul",
                            BinaryOp::Div => "fdiv",
                        };
                        let result =
                            self.value(Type::F64, format!("{name} double {}, {}", a.text, b.text));
                        self.canonical_float(result)
                    } else {
                        if *op == BinaryOp::Div {
                            let bad = self
                                .value(Type::Bool, format!("icmp eq {} {}, 0", ty(b.ty), b.text));
                            self.guard(&bad.text, 3, span);
                        }
                        let name = match op {
                            BinaryOp::Add => "add",
                            BinaryOp::Sub => "sub",
                            BinaryOp::Mul => "mul",
                            BinaryOp::Div => "udiv",
                        };
                        self.value(
                            instruction.ty,
                            format!("{name} {} {}, {}", ty(a.ty), a.text, b.text),
                        )
                    }
                }
                Operation::Compare { op, lhs, rhs } if values[lhs.0].ty != Type::I64 => {
                    let (a, b) = (&values[lhs.0], &values[rhs.0]);
                    let float = a.ty == Type::F64;
                    let pred = match (op, float) {
                        (CompareOp::Eq, true) => "oeq",
                        (CompareOp::Ne, true) => "une",
                        (CompareOp::Lt, true) => "olt",
                        (CompareOp::Le, true) => "ole",
                        (CompareOp::Gt, true) => "ogt",
                        (CompareOp::Ge, true) => "oge",
                        (CompareOp::Eq, false) => "eq",
                        (CompareOp::Ne, false) => "ne",
                        (CompareOp::Lt, false) => "ult",
                        (CompareOp::Le, false) => "ule",
                        (CompareOp::Gt, false) => "ugt",
                        (CompareOp::Ge, false) => "uge",
                    };
                    self.value(
                        Type::Bool,
                        format!(
                            "{} {pred} {} {}, {}",
                            if float { "fcmp" } else { "icmp" },
                            ty(a.ty),
                            a.text,
                            b.text
                        ),
                    )
                }
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
                        .region_deferred(then_region, &values, span, then_deferred.as_mut(), true)
                        .remove(0);
                    let yes_end = self.current;
                    self.branch(join);
                    self.current = no;
                    let no_value = self
                        .region_deferred(else_region, &values, span, else_deferred.as_mut(), true)
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
                    let range_plans = crate::read_range::plans(
                        initial,
                        condition,
                        body,
                        &instructions[..position],
                        inputs.len(),
                    );
                    let saved_reads = self.proven_reads.clone();
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
                        if value.ty.is_dynamic() {
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
                    // replacement chain, including nonallocating scalar lazy regions.
                    // Only calls covered by the HIR summary may borrow; allocating
                    // calls and escaping sequences keep the shadow-stack protocol.
                    let retain_roots = !initial.iter().any(|v| matches!(v.ty, Type::Record(..)))
                        && crate::loop_storage::retain_roots(
                            &initial.iter().map(|v| v.ty).collect::<Vec<_>>(),
                            condition,
                            body,
                            &plans,
                            self.summaries,
                        );
                    let borrow_loop = self.summaries.is_some_and(|proof| {
                        proof.instruction(
                            instruction,
                            &values.iter().map(|v| v.ty).collect::<Vec<_>>(),
                        )
                    });
                    let data_pointers = if borrow_loop {
                        range_plans
                            .into_iter()
                            .filter_map(|(sequence, index)| {
                                let kind = initial[sequence].ty;
                                if !kind.is_dynamic() {
                                    return None;
                                }
                                let pointer = self.register();
                                self.line(format!(
                                    "{pointer} = getelementptr i8, ptr {}, i64 40",
                                    initial[sequence].text
                                ));
                                Some((sequence, index, pointer, kind))
                            })
                            .collect::<Vec<_>>()
                    } else {
                        vec![]
                    };
                    let retained_slots = if borrow_loop {
                        vec![None; initial.len()]
                    } else if retain_roots {
                        initial
                            .iter()
                            .map(|value| {
                                if nil_hir::records::has_dynamic(self.records, value.ty) {
                                    self.root_for(value)
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
                            (value.ty.is_dynamic()
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
                    let state_roots = if retain_roots || borrow_loop {
                        retained_slots
                    } else {
                        state
                            .iter()
                            .map(|value| {
                                if nil_hir::records::has_dynamic(self.records, value.ty) {
                                    self.root_for(value)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    };
                    let condition_value = if retain_roots || borrow_loop {
                        let no_roots = vec![None; state.len()];
                        let values = self.instructions_with_roots(
                            &condition.instructions,
                            &state,
                            &condition.results,
                            None,
                            Some(&no_roots),
                            true,
                        );
                        self.tick(span);
                        values[condition.results[0].0].clone()
                    } else {
                        self.region(condition, &state, span).remove(0)
                    };
                    self.conditional(&condition_value.text, body_block, exit);
                    self.current = body_block;
                    for (sequence, index, data, kind) in data_pointers {
                        self.proven_reads.insert(
                            (state[sequence].text.clone(), state[index].text.clone()),
                            (data, kind),
                        );
                    }
                    if !retain_roots && !borrow_loop {
                        for slot in state_roots.iter().flatten() {
                            self.store_root(slot, "null");
                        }
                    }
                    let body_values = self.instructions_with_roots(
                        &body.instructions,
                        &state,
                        &body.results,
                        Some(&mut deferred),
                        (retain_roots || borrow_loop).then_some(state_roots.as_slice()),
                        borrow_loop,
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
                    // Body range facts do not hold on the exit edge (index may
                    // equal length). Restore only enclosing-loop facts before finish.
                    self.proven_reads = saved_reads.clone();
                    self.current = exit;
                    for slot in state_roots.iter().flatten() {
                        self.store_root(slot, "null");
                    }
                    let result = self
                        .region_deferred(finish, &state, span, None, borrow_loop)
                        .remove(0);
                    self.invariant_lengths = saved_lengths;
                    self.proven_reads = saved_reads;
                    result
                }
            };
            let result_slot = if !borrowing
                && nil_hir::records::has_dynamic(self.records, result.ty)
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
                if let Some(slot) = recycled {
                    self.store_value_root(&slot, &result);
                    Some(slot)
                } else {
                    self.root_for(&result)
                }
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
        let no_roots = vec![None; inputs.len()];
        let borrow_function = self.summaries.is_some_and(|proof| proof.function(index));
        let values = self.instructions_with_roots(
            &function.instructions,
            &inputs,
            std::slice::from_ref(&function.result),
            None,
            borrow_function.then_some(no_roots.as_slice()),
            borrow_function,
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
    builder.records = &program.program().records;
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
            Type::Record(..) | Type::MapRecord(..) => {
                unreachable!("record entries require wrapper")
            }
            Type::I64 => elements.remove(0),
            Type::U64 => {
                let mut v = elements.remove(0);
                v.ty = Type::U64;
                v
            }
            Type::U128 => builder.wide_join(&elements[0], &elements[1]),
            Type::F64 => {
                let v = builder.value(
                    Type::F64,
                    format!("bitcast i64 {} to double", elements[0].text),
                );
                builder.canonical_float(v)
            }
            Type::Bool => {
                let raw = elements.remove(0);
                let invalid = builder.value(Type::Bool, format!("icmp ugt i64 {}, 1", raw.text));
                builder.guard(&invalid.text, 5, None);
                builder.value(Type::Bool, format!("trunc i64 {} to i1", raw.text))
            }
            Type::Array(_) => builder.array(&elements),
            Type::Buffer | Type::Bytes | Type::MapI64 | Type::MapBytes => builder.value(
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
            Type::Record(..) | Type::MapRecord(..) => {
                unreachable!("record entries require wrapper")
            }
            Type::I64 | Type::U64 => result.clone(),
            Type::F64 => builder.value(Type::I64, format!("bitcast double {} to i64", result.text)),
            Type::U128 => {
                let v = if index == 0 {
                    result.clone()
                } else {
                    builder.value(Type::U128, format!("lshr i128 {}, 64", result.text))
                };
                builder.value(Type::I64, format!("trunc i128 {} to i64", v.text))
            }
            Type::Buffer | Type::Bytes | Type::MapI64 | Type::MapBytes => {
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
    for (id, record) in program.program().records.iter().enumerate() {
        writeln!(
            out,
            "%nil.record{id} = type {{ {} }}",
            record
                .fields
                .iter()
                .map(|f| ty(f.ty))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
    }
    let summaries = nil_hir::borrowing::Summaries::analyze(program);
    let plugins = plugin::providers(program);
    for (index, function) in program.program().functions.iter().enumerate() {
        let mut builder = Builder::new(program.program().arithmetic, bounded);
        builder.records = &program.program().records;
        builder.summaries = Some(&summaries);
        builder.plugins = plugins.clone();
        out.push_str(&builder.function(index, function));
    }
    for (plugin_index, provider) in plugins.iter().enumerate() {
        let p = provider.program();
        let proof = borrowing::Summaries::analyze(p);
        for (index, function) in p.program().functions.iter().enumerate() {
            let mut builder = Builder::new(p.program().arithmetic, false);
            builder.records = &p.program().records;
            builder.summaries = Some(&proof);
            out.push_str(
                &builder
                    .function(index, function)
                    .replace("@nil_fn", &format!("@nil_plugin{plugin_index}_fn"))
                    .replace(") alwaysinline {", ") {"),
            );
        }
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
            (item.ty.is_dynamic()
                || matches!(
                    item.ty,
                    Type::U64 | Type::U128 | Type::F64 | Type::Record(..)
                ))
                || match &item.operation {
                    Operation::Intrinsic { .. }
                    | Operation::Bytes(_)
                    | Operation::PluginCall { .. } => true,
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
        (f.result_type.is_dynamic()
            || matches!(
                f.result_type,
                Type::U64 | Type::U128 | Type::F64 | Type::Record(..)
            ))
            || f.parameters.iter().any(|t| {
                t.is_dynamic() || matches!(t, Type::U64 | Type::U128 | Type::F64 | Type::Record(..))
            })
            || instructions(&f.instructions)
    })
}
const APPLICATION_HELPERS: &str = r#"
declare void @nil_root_store(ptr, ptr)
declare ptr @nil_roots_enter(ptr, i64)
declare void @nil_roots_leave(ptr)
declare ptr @nil_format_f64(double, i64, i64)
declare ptr @nil_format_u64(i64, i64, i64)
declare ptr @nil_format_u128(ptr, i64, i64)
declare double @nil_parse_f64(ptr, i64, i64)
declare i64 @nil_parse_u64(ptr, i64, i64)
declare void @nil_parse_u128(ptr, ptr, i64, i64)
declare ptr @nil_sort(ptr, i64, i64, i64, i64)
declare ptr @nil_sort_unique(ptr, i64, i64, i64, i64)
declare ptr @nil_map(i64, i64)
declare void @nil_map_get_record(ptr, ptr, ptr, i64, i64, i64)
declare ptr @nil_map_update_record(ptr, ptr, ptr, i64, i1, i1, i64, i64)
declare i64 @nil_map_size(ptr, i64, i64)
declare i1 @nil_map_has(ptr, ptr, i64, i64)
declare ptr @nil_map_key(ptr, i64, i64, i64)
declare i64 @nil_map_get_int(ptr, ptr, i64, i64)
declare ptr @nil_map_get_bytes(ptr, ptr, i64, i64)
declare ptr @nil_map_insert_int(ptr, ptr, i64, i64, i64)
declare ptr @nil_map_insert_bytes(ptr, ptr, ptr, i64, i64)
declare ptr @nil_map_insert_unique_int(ptr, ptr, i64, i64, i64)
declare ptr @nil_map_insert_unique_bytes(ptr, ptr, ptr, i64, i64)
declare ptr @nil_map_put_int(ptr, ptr, i64, i64, i64)
declare ptr @nil_map_put_bytes(ptr, ptr, ptr, i64, i64)
declare ptr @nil_map_put_unique_int(ptr, ptr, i64, i64, i64)
declare ptr @nil_map_put_unique_bytes(ptr, ptr, ptr, i64, i64)
declare ptr @nil_literal(ptr, i64, i64, i64)
declare ptr @nil_make(i64, i64, i64, i64, i64)
declare i64 @nil_length(ptr)
declare i64 @nil_get(ptr, i64, i64, i64)
declare ptr @nil_set(ptr, i64, i64, i64, i64)
declare ptr @nil_set_unique(ptr, i64, i64, i64, i64)
declare ptr @nil_concat(ptr, ptr, i64, i64)
declare ptr @nil_concat_unique(ptr, ptr, i64, i64)
declare ptr @nil_slice(ptr, i64, i64, i64, i64)
declare ptr @nil_format(i64, i64, i64)
declare i64 @nil_parse(ptr, i64, i64)
declare ptr @nil_parsebuf(ptr, ptr, i64, i64)
declare i64 @nil_find(ptr, i64, i64, i64, i64)
declare ptr @nil_read(ptr, i64, i64)
declare i64 @nil_write(ptr, ptr, i64, i64)
declare i64 @nil_out(ptr, i64, i64)
"#;
