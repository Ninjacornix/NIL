//! Seeded mutation/property fuzzing for expr-v3; no third-party dependencies.
pub mod application;
pub mod generate;
use generate::{Case, Random};
use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, execute},
};
use nil_hir::{Diagnostic, FunctionId, Operation, Type, ValueId};
pub const LIMITS: Limits = Limits {
    steps: 100000,
    call_depth: 16,
};
fn diagnostic(error: &Diagnostic, source: &str) {
    if let Some(span) = error.span {
        assert!(
            span.start <= span.end && span.end <= source.len(),
            "invalid diagnostic span: {error:?}"
        );
        assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
    }
    assert!(error.code.starts_with('E') && !error.message.is_empty());
    assert!(!error.to_string().is_empty());
}
pub fn frontend(source: &str) {
    frontend_with_profile(source, SourceProfile::ExprV3);
}
pub fn frontend_with_profile(source: &str, profile: SourceProfile) {
    let a = compile_with_profile(source, profile);
    let b = compile_with_profile(source, profile);
    match (a, b) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a.hir, b.hir, "nondeterministic HIR");
            assert_eq!(nil_compiler::dump(&a.hir), nil_compiler::dump(&b.hir));
            assert_eq!(nil_hir::validate(a.hir.program().clone()).unwrap(), a.hir);
            // Mutation-accepted programs can loop/recurse; always use bounded execution.
            let args = a.hir.program().functions[0]
                .parameters
                .iter()
                .map(|ty| {
                    use nil_compiler::evaluator::Value;
                    match ty {
                        Type::I64 => Value::I64(0),
                        Type::Bool => Value::Bool(false),
                        Type::Array(n) => Value::array(vec![0; *n]),
                        Type::Buffer => Value::Buffer(vec![].into()),
                        Type::Bytes => Value::Bytes(vec![].into()),
                        Type::MapI64 => Value::Map(nil_compiler::keyed::Map::empty(false)),
                        Type::MapBytes => Value::Map(nil_compiler::keyed::Map::empty(true)),
                    }
                })
                .collect::<Vec<_>>();
            if let Err(e) = nil_compiler::evaluator::execute_values(
                &a.hir,
                FunctionId(0),
                &args,
                Limits {
                    steps: 256,
                    call_depth: 8,
                },
            ) {
                diagnostic(&e, source);
            }
            assert_eq!(nil_llvm::emit_llvm(&a.hir), nil_llvm::emit_llvm(&b.hir));
        }
        (Err(a), Err(b)) => {
            assert_eq!(a, b, "nondeterministic diagnostics");
            diagnostic(&a, source);
        }
        _ => panic!("nondeterministic compile acceptance"),
    }
}
pub fn valid(case: &Case) {
    let p = compile_with_profile(&case.source, SourceProfile::ExprV3)
        .unwrap_or_else(|e| panic!("generator produced invalid source: {e}"));
    for args in &case.arguments {
        let expected = case.expected(args);
        let actual = execute(&p.hir, FunctionId(0), args, LIMITS);
        match (expected, actual) {
            (Ok(a), Ok(b)) => assert_eq!(a, b, "oracle mismatch for {args:?}"),
            (Err("division by zero"), Err(e)) => {
                assert_eq!(e.code, "E009");
                assert_eq!(e.message, "division by zero");
                diagnostic(&e, &case.source);
            }
            (a, b) => panic!("oracle/reference mismatch for {args:?}: {a:?} / {b:?}"),
        }
    }
    // Metamorphic formatting: whole-line whitespace and LF/CRLF preserve semantics.
    let spaced = case
        .source
        .lines()
        .map(|line| format!(" \t{line} \r\n\n"))
        .collect::<String>();
    let p2 = compile_with_profile(&spaced, SourceProfile::ExprV3).unwrap();
    assert_eq!(nil_compiler::dump(&p.hir), nil_compiler::dump(&p2.hir));
    for args in &case.arguments {
        match (
            execute(&p.hir, FunctionId(0), args, LIMITS),
            execute(&p2.hir, FunctionId(0), args, LIMITS),
        ) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(a), Err(b)) => {
                assert_eq!(a.code, b.code);
                assert_eq!(a.message, b.message);
                diagnostic(&b, &spaced);
            }
            _ => panic!("formatting changed execution"),
        }
    }
}
pub fn mutate(bytes: &[u8], r: &mut Random) -> Vec<u8> {
    const ALPHABET: &[u8] = b"abcxyz01239+-*/<>!=?:;@(),\r\n \t\0\xff";
    mutate_alphabet(bytes, r, ALPHABET)
}
pub fn mutate_typed(bytes: &[u8], r: &mut Random) -> Vec<u8> {
    mutate_alphabet(bytes, r, b"abcxyz01239+-*/<>!=?:;@(),[]#\r\n \t\0\xff")
}
fn mutate_alphabet(bytes: &[u8], r: &mut Random, alphabet: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    for _ in 0..1 + r.pick(6) {
        let pos = r.pick(out.len() + 1);
        match r.pick(5) {
            0 if pos < out.len() => {
                out.remove(pos);
            }
            1 if pos < out.len() => out[pos] = alphabet[r.pick(alphabet.len())],
            2 => out.insert(pos, alphabet[r.pick(alphabet.len())]),
            3 => out.truncate(pos),
            _ => {
                out.splice(pos..pos, "λ💥".as_bytes().iter().copied());
            }
        }
    }
    out
}
fn corrupt_instruction(instruction: &mut nil_hir::Instruction, r: &mut Random) {
    match &mut instruction.operation {
        Operation::If {
            condition,
            then_region,
            else_region,
        } => match r.pick(3) {
            0 => *condition = ValueId(usize::MAX),
            1 => then_region.results = vec![ValueId(usize::MAX)],
            _ => {
                if let Some(child) = else_region.instructions.first_mut() {
                    corrupt_instruction(child, r);
                } else {
                    else_region.results.clear();
                }
            }
        },
        Operation::Loop {
            initial,
            condition,
            body,
            finish,
        } => match r.pick(4) {
            0 => {
                initial.pop();
            }
            1 => condition.results = vec![ValueId(usize::MAX)],
            2 => body.results.clear(),
            _ => {
                if let Some(child) = finish.instructions.first_mut() {
                    corrupt_instruction(child, r);
                } else {
                    finish.results = vec![ValueId(usize::MAX)];
                }
            }
        },
        Operation::Binary { lhs, .. } | Operation::Compare { lhs, .. } => {
            *lhs = ValueId(usize::MAX)
        }
        Operation::Call { arguments, .. } => arguments.push(ValueId(usize::MAX)),
        _ => {
            instruction.ty = if instruction.ty == Type::Bool {
                Type::I64
            } else {
                Type::Bool
            }
        }
    }
}
pub fn hir(case: &Case, r: &mut Random) {
    let p = compile_with_profile(&case.source, SourceProfile::ExprV3).unwrap();
    let mut mutant = p.hir.program().clone();
    let f = &mut mutant.functions[r.pick(3)];
    match r.pick(8) {
        0 => f.result = ValueId(usize::MAX),
        1 => f.result_type = Type::Bool,
        2 => {
            if !f.parameters.is_empty() {
                f.parameters[0] = Type::Bool;
            }
        }
        3 => {
            if let Some(i) = f.instructions.first_mut() {
                i.ty = Type::Bool;
            }
        }
        4 => {
            if let Some(i) = f.instructions.first_mut() {
                i.operation = Operation::Call {
                    function: FunctionId(usize::MAX),
                    arguments: vec![],
                };
            }
        }
        5 => {
            if let Some(i) = f.instructions.first_mut() {
                i.operation = Operation::Binary {
                    op: nil_hir::BinaryOp::Add,
                    lhs: ValueId(usize::MAX),
                    rhs: ValueId(0),
                };
            }
        }
        6 => {
            if !f.instructions.is_empty() {
                let index = r.pick(f.instructions.len());
                corrupt_instruction(&mut f.instructions[index], r);
            }
        }
        _ => mutant.functions.pop().map(|_| ()).unwrap(),
    }
    let a = nil_hir::validate(mutant.clone());
    let b = nil_hir::validate(mutant);
    assert_eq!(a, b, "nondeterministic HIR validation");
    match a {
        Ok(p) => {
            assert_eq!(nil_llvm::emit_llvm(&p), nil_llvm::emit_llvm(&p));
            let args = vec![0; p.program().functions[0].parameters.len()];
            let _ = execute(
                &p,
                FunctionId(0),
                &args,
                Limits {
                    steps: 256,
                    call_depth: 8,
                },
            );
        }
        Err(e) => diagnostic(&e, &case.source),
    }
}
pub const CORPUS: &[&str] = &[
    include_str!("../../../fuzz/corpus/expr-v3/arithmetic.nil"),
    include_str!("../../../fuzz/corpus/expr-v3/control.nil"),
    include_str!("../../../fuzz/corpus/expr-v3/recursion.nil"),
    include_str!("../../../fuzz/corpus/expr-v3/invalid.nil"),
];
