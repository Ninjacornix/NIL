use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::{FunctionId, Type};

fn eval(source: &str, args: &[Value]) -> Value {
    let p = compile_with_profile(source, SourceProfile::ExprV4).unwrap();
    execute_values(&p.hir, FunctionId(0), args, Limits::default()).unwrap()
}
fn array(values: &[i64]) -> Value {
    Value::array(values.to_vec())
}

#[test]
fn every_existing_v3_example_has_identical_v4_hir() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/expr-v3");
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "nil") {
            let text = std::fs::read_to_string(path).unwrap();
            let a = compile_with_profile(&text, SourceProfile::ExprV3).unwrap();
            let b = compile_with_profile(&text, SourceProfile::ExprV4).unwrap();
            assert_eq!(a.hir.program(), b.hir.program());
        }
    }
    assert_eq!(SourceProfile::default(), SourceProfile::ExprV0);
}
#[test]
fn typed_functions_forward_calls_and_returns() {
    assert_eq!(eval("1=b(a)?1:0\n1:b=a>0", &[Value::I64(4)]), Value::I64(1));
    assert_eq!(
        eval("(b,i)=a?b:0", &[Value::Bool(false), Value::I64(42)]),
        Value::I64(0)
    );
    assert_eq!(
        eval(
            "3=#b([a,b,c])\n(3):3=a[1:a[0]+a[2]]",
            &[Value::I64(2), Value::I64(3), Value::I64(4)]
        ),
        Value::I64(3)
    );
    assert_eq!(
        eval("(3):3=b(a)\n(3):3=a[1:a[0]+a[2]]", &[array(&[2, 3, 4])]),
        array(&[2, 6, 4])
    );
    assert_eq!(
        eval(
            "(3,i):3=b>0?a(a[0:a[0]+1],b-1):a",
            &[array(&[0, 1, 2]), Value::I64(5)]
        ),
        array(&[5, 1, 2])
    );
}
#[test]
fn construction_length_indexing_and_array_branches() {
    assert_eq!(eval(":0=[]", &[]), array(&[]));
    assert_eq!(eval("=#[]", &[]), Value::I64(0));
    assert_eq!(eval(":3=[4;3]", &[]), array(&[4, 4, 4]));
    assert_eq!(eval("=true?[1,2][1]:[][0]", &[]), Value::I64(2));
    assert_eq!(
        eval("(b):2=a?[1,2]:[3,4]", &[Value::Bool(false)]),
        array(&[3, 4])
    );
    assert_eq!(eval("=[1,2][0:7][0]", &[]), Value::I64(7));
}
#[test]
fn replacement_preserves_aliases_and_updates_are_simultaneous() {
    let source = "(2)=@(a,a,0;c<1;a[0:9],a,c+1;a[0]*10+b[0])";
    let input = array(&[1, 2]);
    assert_eq!(eval(source, std::slice::from_ref(&input)), Value::I64(91));
    assert_eq!(input, array(&[1, 2]));
    assert_eq!(
        eval("=@([1,2],true,0;b;a[0:a[0]+1],false,c+1;a[0]+c)", &[]),
        Value::I64(3)
    );
}
#[test]
fn bounds_traps_are_structured_and_lazy() {
    for source in ["=[][0]", "=[1,2][-1]", "=[1,2][2]", ":2=[1,2][2:9]"] {
        let p = compile_with_profile(source, SourceProfile::ExprV4).unwrap();
        let e = execute_values(&p.hir, FunctionId(0), &[], Limits::default()).unwrap_err();
        assert_eq!(e.code, "E012");
        assert_eq!(e.phase, nil_hir::Phase::Execute);
        assert!(e.span.is_some());
        assert_eq!(e.message, "array index out of bounds");
    }
    // Replacement's value expression executes before the bounds check.
    let p = compile_with_profile("=[1][2:1/0][0]", SourceProfile::ExprV4).unwrap();
    assert_eq!(
        execute_values(&p.hir, FunctionId(0), &[], Limits::default())
            .unwrap_err()
            .code,
        "E009"
    );
    assert_eq!(eval("=false?[][0]:42", &[]), Value::I64(42));
}
#[test]
fn malformed_aliases_and_unsupported_types_are_rejected() {
    for source in [
        "(i)=a",
        "()=1",
        ":i=1",
        "(float)=a",
        "(257)=#a",
        ":257=[]",
        "(08)=#a",
        ":2=[0;1]",
        ":0=[0;0]",
        ":2=[0;257]",
        "=1[0]",
        "=#[1][0]",
        "=([1])[]",
        ":2=[1,]",
        "(b,b)=a+b",
        "(2,b)=a[b]",
        "(2):2=a[0:true]",
        ":1=[true]",
        "(2):3=a",
        "(b)=a?1:[1]",
        "(2):2=@(a;true;[0];a)",
        "(2)=b(a)\n(3)=#a",
    ] {
        assert!(
            compile_with_profile(source, SourceProfile::ExprV4).is_err(),
            "{source}"
        );
    }
    for profile in [
        SourceProfile::LinesV0,
        SourceProfile::ExprV0,
        SourceProfile::ExprV1,
        SourceProfile::ExprV2,
        SourceProfile::ExprV3,
    ] {
        assert!(compile_with_profile("(2)=a[0]", profile).is_err());
    }
}
#[test]
fn typed_entry_checks_exact_array_length_and_bool_type() {
    let p = compile_with_profile("(2,b)=b?a[0]:a[1]", SourceProfile::ExprV4).unwrap();
    for args in [
        vec![array(&[1]), Value::Bool(true)],
        vec![array(&[1, 2]), Value::I64(1)],
    ] {
        assert_eq!(
            execute_values(&p.hir, FunctionId(0), &args, Limits::default())
                .unwrap_err()
                .code,
            "E007"
        );
    }
    assert_eq!(
        p.hir.program().functions[0].parameters,
        vec![Type::Array(2), Type::Bool]
    );
}
#[test]
fn seeded_arrays_agree_with_independent_algorithm_oracles() {
    let mut seed = 0x12345678u64;
    for n in [0, 1, 2, 8, 32, 128, 256] {
        let sum = format!("({n})=@(a,0,0;b<#a;a,b+1,c+a[b];c)");
        let reverse = format!("({n}):{n}=@(a,[0;{n}],0;c<#a;a,b[c:a[#a-c-1]],c+1;b)");
        let reverse = if n < 2 {
            reverse.replace(&format!("[0;{n}]"), if n == 0 { "[]" } else { "[0]" })
        } else {
            reverse
        };
        for _ in 0..10 {
            let values = (0..n)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    seed as i64
                })
                .collect::<Vec<_>>();
            let expected = values.iter().fold(0i64, |a, b| a.wrapping_add(*b));
            assert_eq!(eval(&sum, &[array(&values)]), Value::I64(expected));
            assert_eq!(
                eval(&reverse, &[array(&values)]),
                Value::array(values.iter().rev().copied().collect())
            );
        }
    }
}

#[test]
fn array_hir_and_type_diagnostic_match_reviewed_golden_fixtures() {
    let p = compile_with_profile(
        include_str!("../../../examples/expr-v4/replace.nil"),
        SourceProfile::ExprV4,
    )
    .unwrap();
    assert_eq!(
        nil_compiler::dump(&p.hir),
        include_str!("fixtures/array.hir")
    );
    let e = compile_with_profile("(2,b)=a[b]\n", SourceProfile::ExprV4).unwrap_err();
    assert_eq!(format!("{e}\n"), include_str!("fixtures/array-index.diag"));
    assert_eq!(e.expected.as_deref(), Some("I64"));
    assert_eq!(e.actual.as_deref(), Some("Bool"));
}
#[test]
fn splat_evaluates_its_operand_once_and_honors_limits() {
    let limits = Limits {
        steps: 5,
        call_depth: 2,
    };
    let p = compile_with_profile(":3=[b();3]\n=42", SourceProfile::ExprV4).unwrap();
    assert_eq!(
        execute_values(&p.hir, FunctionId(0), &[], limits).unwrap(),
        array(&[42, 42, 42])
    );
    let p = compile_with_profile(":3=[b(),b(),b()]\n=42", SourceProfile::ExprV4).unwrap();
    assert_eq!(
        execute_values(&p.hir, FunctionId(0), &[], limits)
            .unwrap_err()
            .code,
        "E008"
    );
}
