use nil_compiler::{
    SourceProfile, compile, compile_with_profile,
    evaluator::{Limits, execute},
};
fn run(source: &str, args: &[i64]) -> Result<i64, nil_compiler::hir::Diagnostic> {
    let p = compile_with_profile(source, SourceProfile::ExprV2)?;
    execute(&p.hir, p.function(0).unwrap(), args, Limits::default())
}
#[test]
fn acceptance_programs_match_independent_oracles_over_their_domains() {
    let factorial = compile(include_str!("../../../examples/factorial.nil")).unwrap();
    for n in 0..=20 {
        let expected = (1..=n).product::<i64>();
        assert_eq!(
            execute(
                &factorial.hir,
                factorial.function(1).unwrap(),
                &[n],
                Limits::default()
            )
            .unwrap(),
            expected
        );
    }
    let fibonacci = compile(include_str!("../../../examples/fibonacci.nil")).unwrap();
    let (mut a, mut b) = (0u128, 1u128);
    for n in 0..=92 {
        assert_eq!(
            execute(
                &fibonacci.hir,
                fibonacci.function(1).unwrap(),
                &[n],
                Limits::default()
            )
            .unwrap(),
            i64::try_from(a).unwrap()
        );
        if n < 92 {
            (a, b) = (b, a.checked_add(b).unwrap());
        }
    }
    let sum = compile(include_str!("../../../examples/counted_sum.nil")).unwrap();
    for n in 0..=1000 {
        assert_eq!(
            execute(&sum.hir, sum.function(1).unwrap(), &[n], Limits::default()).unwrap(),
            n * (n + 1) / 2
        );
    }
    let max = compile(include_str!("../../../examples/max.nil")).unwrap();
    for a in [i64::MIN, -1, 0, 1, i64::MAX] {
        for b in [i64::MIN, -1, 0, 1, i64::MAX] {
            assert_eq!(
                execute(
                    &max.hir,
                    max.function(1).unwrap(),
                    &[a, b],
                    Limits::default()
                )
                .unwrap(),
                a.max(b)
            );
        }
    }
}
#[test]
fn comparisons_are_signed_and_boolean() {
    for (op, predicate) in [
        ("==", (i64::eq as fn(&i64, &i64) -> bool)),
        ("!=", i64::ne),
        ("<", i64::lt),
        ("<=", i64::le),
        (">", i64::gt),
        (">=", i64::ge),
    ] {
        for (a, b) in [(i64::MIN, i64::MAX), (0, 0), (7, -2)] {
            assert_eq!(
                run(&format!("2=a{op}b?1:0"), &[a, b]).unwrap(),
                i64::from(predicate(&a, &b))
            );
        }
    }
}
#[test]
fn branches_are_lazy_but_both_are_checked() {
    assert_eq!(run("=true?42:1/0", &[]).unwrap(), 42);
    assert_eq!(run("=false?9223372036854775807+1:42", &[]).unwrap(), 42);
    assert_eq!(run("=true?42:a()", &[]).unwrap(), 42);
    assert_eq!(
        compile_with_profile("=true?42:b()", SourceProfile::ExprV2)
            .unwrap_err()
            .code,
        "E004"
    );
    assert_eq!(run("=false?42:1/0", &[]).unwrap_err().code, "E009");
}
#[test]
fn nested_branches_precedence_and_region_scope_are_correct() {
    for (a, expected) in [(-5, 0), (0, 0), (5, 10)] {
        assert_eq!(run("1=(a>0?a:0)+(a<0?0:a)", &[a]).unwrap(), expected);
    }
    assert_eq!(run("1=a==0?1:a<0?2:3", &[-1]).unwrap(), 2);
    assert_eq!(run("1=(a>0?true:false)?1:0", &[7]).unwrap(), 1);
    assert_eq!(run("2=a+b*2>=7?42:0", &[3, 2]).unwrap(), 42);
}
#[test]
fn loop_updates_are_simultaneous_and_nested_loops_are_scoped() {
    assert_eq!(run("=@(1,2,1;c>0;b,a,c-1;a*10+b)", &[]).unwrap(), 21);
    assert_eq!(
        run("1=@(0,a;b>0;a+@(0,b;b>0;a+1,b-1;a),b-1;a)", &[5]).unwrap(),
        15
    );
    assert_eq!(run("=@(true,0;a;b<4,b+1;b)", &[]).unwrap(), 5);
    assert_eq!(run("=@(1;false;1/0;a)", &[]).unwrap(), 1);
}
#[test]
fn loop_functions_and_outer_initializers_work_in_each_profile() {
    for (profile, source) in [
        (
            SourceProfile::ExprV0,
            "f0(x)=loop(0,x;b>0;f1(a),b-1;a)\nf1(x)=x+2",
        ),
        (SourceProfile::ExprV1, "x=loop(0,x;b>0;b(a),b-1;a)\nx=x+2"),
        (SourceProfile::ExprV2, "1=@(0,a;b>0;b(a),b-1;a)\n1=a+2"),
    ] {
        let p = compile_with_profile(source, profile).unwrap();
        assert_eq!(
            execute(&p.hir, p.function(0).unwrap(), &[3], Limits::default()).unwrap(),
            6
        );
    }
}
#[test]
fn invalid_control_flow_never_reaches_execution() {
    for (source, code) in [
        ("=1?2:3", "E007"),
        ("=true?1:false", "E007"),
        ("=@(0;1;a;a)", "E007"),
        ("=@(0;true;false;a)", "E007"),
        ("=@(0;true;a,a;a)", "E006"),
        ("=@(0;false;;a)", "E006"),
        ("=@(0;false;a;b)", "E005"),
        ("=true?1", "E001"),
        ("=true?:1", "E001"),
        ("=@(0;true;a;)", "E001"),
        ("=true+1", "E007"),
        ("=1<2<3?1:0", "E007"),
        ("=true", "E007"),
    ] {
        assert_eq!(
            compile_with_profile(source, SourceProfile::ExprV2)
                .unwrap_err()
                .code,
            code,
            "{source}"
        );
    }
    assert_eq!(compile("f0(x)=loop(0;x>0;a+1;a)").unwrap_err().code, "E005");
}
#[test]
fn empty_and_nonempty_infinite_loops_exhaust_fuel() {
    for source in ["=@(;true;;42)", "=@(0;true;a;a)"] {
        let p = compile_with_profile(source, SourceProfile::ExprV2).unwrap();
        let error = execute(
            &p.hir,
            p.function(0).unwrap(),
            &[],
            Limits {
                steps: 30,
                call_depth: 1,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "E008");
    }
}
#[test]
fn regions_do_not_consume_function_call_depth() {
    let p = compile_with_profile("=true?(false?0:42):0", SourceProfile::ExprV2).unwrap();
    assert_eq!(
        execute(
            &p.hir,
            p.function(0).unwrap(),
            &[],
            Limits {
                steps: 100,
                call_depth: 1
            }
        )
        .unwrap(),
        42
    );
}
#[test]
fn overflow_boundaries_are_preserved() {
    let f = compile(include_str!("../../../examples/factorial.nil")).unwrap();
    assert_eq!(
        execute(&f.hir, f.function(1).unwrap(), &[21], Limits::default())
            .unwrap_err()
            .code,
        "E009"
    );
    let f = compile(include_str!("../../../examples/fibonacci.nil")).unwrap();
    assert_eq!(
        execute(&f.hir, f.function(1).unwrap(), &[93], Limits::default())
            .unwrap_err()
            .code,
        "E009"
    );
}
#[test]
fn parser_nesting_limit_covers_control_regions() {
    let source = format!("={}42{}", "true?".repeat(150), ":0".repeat(150));
    assert_eq!(
        compile_with_profile(&source, SourceProfile::ExprV2)
            .unwrap_err()
            .code,
        "E001"
    );
}

#[test]
fn hir_region_limit_is_enforced_at_the_boundary() {
    for (depth, valid) in [(32, true), (33, false)] {
        let source = format!("={}42{}", "true?".repeat(depth), ":0".repeat(depth));
        let result = compile_with_profile(&source, SourceProfile::ExprV2);
        assert_eq!(result.is_ok(), valid);
        if !valid {
            assert_eq!(result.unwrap_err().code, "E008");
        }
    }
}
#[test]
fn each_profile_accepts_only_its_canonical_loop_spelling() {
    assert!(compile_with_profile("=@(0;false;a;a)", SourceProfile::ExprV2).is_ok());
    assert!(compile_with_profile("=loop(0;false;a;a)", SourceProfile::ExprV2).is_err());
    assert!(compile("f0()=@(0;false;a;a)").is_err());
}

#[test]
fn control_corpus_profiles_lower_to_identical_semantics() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/paired/control-samples");
    for name in [
        "factorial",
        "fibonacci",
        "max",
        "counted_sum",
        "absolute",
        "clamp",
        "gcd",
        "nested_sum",
    ] {
        let mut dumps = Vec::new();
        for (v, profile) in [
            (0, SourceProfile::ExprV0),
            (1, SourceProfile::ExprV1),
            (2, SourceProfile::ExprV2),
        ] {
            let source = std::fs::read_to_string(root.join(format!("{name}.v{v}.nil"))).unwrap();
            let p = compile_with_profile(&source, profile).unwrap();
            dumps.push(nil_compiler::dump(&p.hir));
        }
        assert_eq!(dumps[0], dumps[1], "{name}");
        assert_eq!(dumps[0], dumps[2], "{name}");
    }
}
#[test]
fn control_flow_diagnostics_and_ir_are_golden() {
    let error = compile_with_profile("=1?2:3", SourceProfile::ExprV2).unwrap_err();
    assert_eq!(
        format!("{error}\n"),
        include_str!("fixtures/control-condition.diag")
    );
    let p = compile_with_profile("2=a>b?a:b", SourceProfile::ExprV2).unwrap();
    assert_eq!(
        nil_compiler::dump(&p.hir),
        include_str!("fixtures/control.hir")
    );
}

#[test]
fn recursive_calls_inside_branches_preserve_continuations_and_call_limits() {
    let p = compile_with_profile("1=a<=1?1:a*a(a-1)", SourceProfile::ExprV2).unwrap();
    assert_eq!(
        execute(&p.hir, p.function(0).unwrap(), &[10], Limits::default()).unwrap(),
        3628800
    );
    assert_eq!(
        execute(
            &p.hir,
            p.function(0).unwrap(),
            &[10],
            Limits {
                steps: 1000,
                call_depth: 5
            }
        )
        .unwrap_err()
        .code,
        "E008"
    );
}
#[test]
fn empty_region_yields_have_exact_fuel_cost() {
    let p = compile_with_profile("=@(;false;;42)", SourceProfile::ExprV2).unwrap();
    let entry = p.function(0).unwrap();
    assert_eq!(
        execute(
            &p.hir,
            entry,
            &[],
            Limits {
                steps: 6,
                call_depth: 1
            }
        )
        .unwrap(),
        42
    );
    assert_eq!(
        execute(
            &p.hir,
            entry,
            &[],
            Limits {
                steps: 5,
                call_depth: 1
            }
        )
        .unwrap_err()
        .code,
        "E008"
    );
}
