use nil_hir::*;
use std::fmt::Write;

fn ty(ty: Type) -> &'static str {
    match ty {
        Type::I64 => "i64",
        Type::Bool => "i1",
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
    arithmetic: Arithmetic,
    bounded: bool,
    current: usize,
    register: usize,
}
impl Builder {
    fn new(arithmetic: Arithmetic, bounded: bool) -> Self {
        Self {
            arithmetic,
            bounded,
            blocks: vec![Block {
                lines: vec![],
                terminator: None,
            }],
            current: 0,
            register: 0,
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
    fn region(&mut self, region: &Region, inputs: &[Operand], span: Option<Span>) -> Vec<Operand> {
        let values = self.instructions(&region.instructions, inputs);
        self.tick(span);
        region
            .results
            .iter()
            .map(|id| values[id.0].clone())
            .collect()
    }
    fn instructions(&mut self, instructions: &[Instruction], inputs: &[Operand]) -> Vec<Operand> {
        let mut values = inputs.to_vec();
        for instruction in instructions {
            let span = instruction.span;
            self.tick(span);
            let result = match &instruction.operation {
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
                    self.current = yes;
                    let yes_value = self.region(then_region, &values, span).remove(0);
                    let yes_end = self.current;
                    self.branch(join);
                    self.current = no;
                    let no_value = self.region(else_region, &values, span).remove(0);
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
                    let predecessor = self.current;
                    let header = self.block();
                    let body_block = self.block();
                    let exit = self.block();
                    self.branch(header);
                    self.current = header;
                    let mut state = Vec::new();
                    let mut slots = Vec::new();
                    for value in &initial {
                        let text = self.register();
                        slots.push(self.blocks[header].lines.len());
                        self.line("; phi placeholder");
                        state.push(Operand { ty: value.ty, text });
                    }
                    let condition_value = self.region(condition, &state, span).remove(0);
                    self.conditional(&condition_value.text, body_block, exit);
                    self.current = body_block;
                    let next = self.region(body, &state, span);
                    let backedge = self.current;
                    self.branch(header);
                    for ((slot, current), (first, next)) in
                        slots.iter().zip(&state).zip(initial.iter().zip(next))
                    {
                        self.blocks[header].lines[*slot] = format!(
                            "{} = phi {} [ {}, %b{predecessor} ], [ {}, %b{backedge} ]",
                            current.text,
                            ty(current.ty),
                            first.text,
                            next.text
                        );
                    }
                    self.current = exit;
                    self.region(finish, &state, span).remove(0)
                }
            };
            values.push(result);
        }
        values
    }
    fn function(mut self, index: usize, function: &Function) -> String {
        let mut out = format!(
            "define {} @nil_fn{index}(ptr %ctx, i64 %call_start, i64 %call_end",
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
        out.push_str(") {\n");
        if self.bounded {
            self.line("call void @nil_enter(ptr %ctx, i64 %call_start, i64 %call_end)");
        }
        let values = self.instructions(&function.instructions, &inputs);
        self.tick(function.return_span);
        if self.bounded {
            self.line("call void @nil_leave(ptr %ctx)");
        }
        self.blocks[self.current].terminator = Some(format!(
            "ret {} {}",
            ty(function.result_type),
            values[function.result.0].text
        ));
        for (n, block) in self.blocks.iter().enumerate() {
            writeln!(out, "b{n}:").unwrap();
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
        out
    }
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
