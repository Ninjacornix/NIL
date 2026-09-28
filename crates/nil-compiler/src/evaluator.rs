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
struct Frame {
    function: FunctionId,
    next: usize,
    values: Vec<i64>,
}
fn error(code: &'static str, span: Option<Span>, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, Phase::Execute, span, message)
}

/// Execute validated HIR with no host recursion. Each instruction and return costs
/// one step; call depth includes the entry frame. Errors cannot expose partial results.
pub fn execute(
    program: &ValidatedProgram,
    entry: FunctionId,
    args: &[i64],
    limits: Limits,
) -> Result<i64, Diagnostic> {
    let functions = &program.program().functions;
    let function = functions
        .get(entry.0)
        .ok_or_else(|| error("E004", None, "unknown entry function"))?;
    if args.len() != function.parameters.len() {
        return Err(error("E006", None, "entry arity mismatch")
            .mismatch(function.parameters.len(), args.len()));
    }
    if limits.call_depth == 0 {
        return Err(error("E008", None, "call depth limit exceeded"));
    }
    let mut frames = vec![Frame {
        function: entry,
        next: 0,
        values: args.to_vec(),
    }];
    let mut fuel = limits.steps;
    loop {
        let depth = frames.len();
        let frame = frames.last_mut().expect("entry frame exists until return");
        let function = &functions[frame.function.0];
        let instruction = function.instructions.get(frame.next);
        let span = instruction.map_or(function.return_span, |i| i.span);
        if fuel == 0 {
            return Err(error("E008", span, "instruction budget exhausted"));
        }
        fuel -= 1;
        if let Some(instruction) = instruction {
            frame.next += 1;
            let value = match &instruction.operation {
                Operation::Constant(value) => *value,
                Operation::Binary { op, lhs, rhs } => {
                    let a = frame.values[lhs.0];
                    let b = frame.values[rhs.0];
                    let result = match op {
                        BinaryOp::Add => a.checked_add(b),
                        BinaryOp::Sub => a.checked_sub(b),
                        BinaryOp::Mul => a.checked_mul(b),
                        BinaryOp::Div => a.checked_div(b),
                    };
                    result.ok_or_else(|| {
                        error(
                            "E009",
                            span,
                            if *op == BinaryOp::Div && b == 0 {
                                "division by zero"
                            } else {
                                "signed integer overflow"
                            },
                        )
                    })?
                }
                Operation::Call {
                    function,
                    arguments,
                } => {
                    if depth >= limits.call_depth {
                        return Err(error("E008", span, "call depth limit exceeded"));
                    }
                    let args = arguments.iter().map(|id| frame.values[id.0]).collect();
                    frames.push(Frame {
                        function: *function,
                        next: 0,
                        values: args,
                    });
                    continue;
                }
            };
            frame.values.push(value);
        } else {
            let value = frame.values[function.result.0];
            frames.pop();
            if let Some(caller) = frames.last_mut() {
                caller.values.push(value);
            } else {
                return Ok(value);
            }
        }
    }
}
