use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::{FunctionId, Operation, Span};
fn compile(source: &str) -> nil_compiler::CompiledProgram {
    compile_with_profile(source, SourceProfile::ExprV5).unwrap()
}
fn run(source: &str) -> Value {
    let p = compile(source);
    execute_values(&p.hir, FunctionId(0), &[], Limits::default()).unwrap()
}
#[test]
fn static_arguments_specialize_to_first_order_direct_calls() {
    let source = "=b(&c,20)+b(&d,20)\n([i:i],i)=^a(b)\n1=a+1\n1=a*2";
    assert_eq!(run(source), Value::I64(61));
    let p = compile(source);
    assert!(
        p.function(1).is_none(),
        "template cannot be a runtime entry"
    );
    assert_eq!(p.hir.program().functions.len(), 5);
    for f in &p.hir.program().functions {
        for i in &f.instructions {
            if let Operation::Call { function, .. } = i.operation {
                assert!(function.0 < p.hir.program().functions.len());
            }
        }
    }
}
#[test]
fn static_parameters_forward_and_remain_lexical_inside_loop_and_each() {
    assert_eq!(
        run("=b(&d,8)\n([i:i],i)=c(^a,b)\n([i:i],i)=@(0,0;a<8;a+1,b+^a(a);b)\n1=a*2"),
        Value::I64(56)
    );
    assert_eq!(
        run("=b(&c)\n([i:i])=!each([1,2,3],0;c+^a(b);a)\n1=a*a"),
        Value::I64(14)
    );
}
#[test]
fn callback_signature_and_value_mismatches_are_structured() {
    for (source, code) in [
        ("=b(&c,2)\n([i:i],i)=^a(b)\n(b)=1", "E007"),
        ("=b(&c,2)\n([i:i],i)=^a(b)\n1:b=true", "E007"),
        ("=b(&c)\n([i:i],i)=^a(b)\n1=a", "E006"),
        ("=b(1,2)\n([i:i],i)=^a(b)", "E007"),
        ("=b(&c)\n1=a\n1=a", "E007"),
        ("=b(&c)\n([i:i])=^a(1,2)\n1=a", "E006"),
        ("=&a", "E007"),
        ("=b(&d,1)\n([i:i],i)=c(a,b)\n([i:i],i)=^a(b)\n1=a", "E007"),
        ("=b(&c)\n([i:i])=a\n1=a", "E007"),
        ("=b(&c)\n([i:i])=@(^a;true;a;a)\n1=a", "E007"),
    ] {
        let d = compile_with_profile(source, SourceProfile::ExprV5).unwrap_err();
        assert_eq!(d.code, code, "{source}: {d:?}");
        assert!(d.span.is_some());
    }
}
#[test]
fn unknown_function_and_callback_references_preserve_byte_spans() {
    let d = compile_with_profile("=b(&z,1)\n([i:i],i)=^a(b)", SourceProfile::ExprV5).unwrap_err();
    assert_eq!(d.code, "E004");
    assert_eq!(d.span, Some(Span { start: 3, end: 5 }));
    assert_eq!(
        compile_with_profile("=^a(1)", SourceProfile::ExprV5)
            .unwrap_err()
            .code,
        "E005"
    );
    // Unknown references cannot hide in an uninstantiated template.
    assert_eq!(
        compile_with_profile("=1\n([i:i])=c(&z)", SourceProfile::ExprV5)
            .unwrap_err()
            .code,
        "E004"
    );
}
#[test]
fn malformed_callback_syntax_and_earlier_profiles_are_rejected() {
    for source in [
        "([i])=1",
        "([i:])=1",
        "([:i)=1",
        "=&",
        "=^",
        "([[:i]:i])=1",
        "=&!plugin(1)",
    ] {
        let d = compile_with_profile(source, SourceProfile::ExprV5).unwrap_err();
        assert!(matches!(d.code, "E001" | "E002"));
        assert!(d.span.is_some());
    }
    for profile in [
        SourceProfile::ExprV0,
        SourceProfile::ExprV1,
        SourceProfile::ExprV2,
        SourceProfile::ExprV3,
        SourceProfile::ExprV4,
    ] {
        assert!(compile_with_profile("=&a", profile).is_err());
        assert!(compile_with_profile("([i:i])=^a(1)", profile).is_err());
    }
}
#[test]
fn recursive_hof_reuses_a_finite_specialization() {
    let source = "=b(&c,10)\n([i:i],i)=b==0?0:^a(b)+b(^a,b-1)\n1=a";
    assert_eq!(run(source), Value::I64(55));
    assert_eq!(compile(source).hir.program().functions.len(), 3);
}
#[test]
fn specialization_growth_hits_a_hard_bound_without_recursing_the_compiler() {
    fn label(mut n: u32) -> String {
        n += 1;
        let mut s = vec![];
        while n != 0 {
            n -= 1;
            s.push((b'a' + (n % 26) as u8) as char);
            n /= 26;
        }
        s.iter().rev().collect()
    }
    let calls = (2..259)
        .map(|n| format!("b(&{},1)", label(n)))
        .collect::<Vec<_>>()
        .join("+");
    let source = format!("={calls}\n([i:i],i)=^a(b)\n{}", "1=a\n".repeat(257));
    let d = compile_with_profile(&source, SourceProfile::ExprV5).unwrap_err();
    assert_eq!(d.code, "E008");
    assert!(d.message.contains("specialization"));
    assert!(d.span.is_some());
}
#[test]
fn allocating_callback_and_zero_slot_records_use_existing_value_semantics() {
    assert_eq!(
        run("=b(&c,0)\n([i:s],i)=#^a(b)\n1:s=!bytes(a,0)"),
        Value::I64(0)
    );
    assert_eq!(
        run("record Z(x:0)\n=b(&c,Z([]))\n([Z:Z],Z)=#!buffer[Z](2,^a(b))\n(Z):Z=a"),
        Value::I64(2)
    );
}
#[test]
fn nested_callback_calls_report_depth_limit_before_parser_stack_exhaustion() {
    let source = format!("([i:i])={}1{}", "^a(".repeat(512), ")".repeat(512));
    let d = compile_with_profile(&source, SourceProfile::ExprV5).unwrap_err();
    assert_eq!(d.code, "E001");
    assert!(d.message.contains("nesting"));
}
#[test]
fn specialized_borrow_and_effect_summaries_derive_from_each_callback_body() {
    let p = compile("=b(&c,1)+b(&d,1)\n([i:i],i)=^a(b)\n1=a+1\n1=!out(\"x\")+a");
    let summaries = nil_hir::borrowing::Summaries::analyze(&p.hir);
    assert!(summaries.function(3));
    assert!(!summaries.function(4));
}
