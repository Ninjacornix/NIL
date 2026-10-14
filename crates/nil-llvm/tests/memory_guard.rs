//! Ordinary resource failures are checked separately from semantic equivalence.
use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::FunctionId;
use nil_llvm::{Optimization, Options};

fn check(source: &str, guard: usize, expected: Result<i64, &str>) {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let reference = execute_values(
        &p.hir,
        FunctionId(0),
        &[],
        Limits {
            memory_bytes: guard,
            ..Limits::default()
        },
    );
    match expected {
        Ok(n) => assert_eq!(reference.unwrap(), Value::I64(n)),
        Err(code) => assert_eq!(reference.unwrap_err().code, code),
    }
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &p.hir,
            Options {
                optimization,
                memory_bytes: guard as u64,
                ..Options::default()
            },
            &[],
        )
        .unwrap();
        match expected {
            Ok(n) => {
                assert!(out.status.success(), "{:?}", out.stderr);
                assert_eq!(out.stdout, format!("{n}\n").as_bytes());
            }
            Err(code) => {
                assert_eq!(out.status.code(), Some(1));
                assert!(
                    out.stderr.starts_with(format!("{code} @").as_bytes()),
                    "{:?}",
                    out.stderr
                );
            }
        }
    }
}
#[test]
fn guard_can_be_disabled_without_changing_defined_results() {
    check("=#!bytes(4096,7)", 0, Ok(4096));
    check("=#!bytes(4096,7)", 100, Err("E013"));
}
#[test]
fn tiny_guards_fail_cleanly_instead_of_underflowing_header_arithmetic() {
    for limit in [1, 39] {
        check("=#!bytes(0,0)", limit, Err("E013"));
    }
    check("=#!bytes(0,0)", 40, Ok(0));
}
#[test]
fn unique_update_and_sort_do_not_reserve_a_phantom_copy() {
    check("=!sort(!bytes(32,7)[0:9],0)[31]", 72, Ok(9));
}
#[test]
fn live_alias_still_preserves_original_when_storage_is_reused() {
    check("=b(!bytes(32,7))\n(s)=a[0:9][0]+a[0]", 200, Ok(16));
}
#[test]
fn ordinary_guard_does_not_mask_real_bounds_or_byte_failures() {
    check("=!bytes(1,7)[2:9][0]", 41, Err("E012"));
    check("=!bytes(1,7)[0:256][0]", 41, Err("E014"));
}
#[test]
fn new_defaults_are_generous_without_becoming_language_limits() {
    assert_eq!(Limits::default().memory_bytes, 256 * 1024 * 1024);
    assert_eq!(Options::default().memory_bytes, 256 * 1024 * 1024);
}

#[test]
fn concat_growth_roots_heap_and_stack_rhs_across_collection() {
    check(
        "=b(!bytes(1,7),!bytes(64,9))\n(s,s)=#!concat(a,b)",
        512,
        Ok(65),
    );
    check("=#!concat(!bytes(1,7),!bytes(1,9))", 512, Ok(2));
    check(
        "record R(x:s)\n=b(!buffer[R](1,R(\"x\")),!buffer[R](64,R(\"y\")))\n(v[R],v[R])=#!concat(a,b)",
        4096,
        Ok(65),
    );
}

/// Exercises the shape that ADR 050 addendum C changed collection timing for:
/// a nested collection whose rows are appended to in a loop, so the allocation
/// list is long while garbage churns.
///
/// This pins **results**, not timing. Collection timing is implementation
/// defined in `EXPR_V5.md` and is not observable through program results, and
/// the 2.51x improvement addendum C records could not be reduced to a
/// synthetic case — it is measured on `course-schedule` v2 instead. So this
/// test guards against the gate changing an answer, which is what a
/// collection change can actually get wrong.
#[test]
fn a_row_updating_loop_over_a_nested_collection_keeps_its_results() {
    let source = "record Row(to:v)\n\
                  (v)=@(!buffer[Row](a[0],Row(!buffer(0,0))),0,a[0];b<c;\
                  a[b:Row(!concat(a[b].to,!buffer(1,b)))],b+1,c;#a)\n";
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &p.hir,
            Options {
                optimization,
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                ..Default::default()
            },
            &["[4000]".to_string()],
        )
        .unwrap();
        assert!(
            out.status.success(),
            "{optimization:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "4000");
    }
}

/// The same gate must not weaken the allocation guard: memory pressure still
/// collects, and a workload that genuinely exceeds the guard still reports
/// E013 rather than growing without bound.
#[test]
fn many_live_rows_still_report_the_allocation_guard() {
    check(
        "record Row(to:v)\n=#!buffer[Row](262144,Row(!buffer(256,0)))\n",
        1 << 20,
        Err("E013"),
    );
}
