//! Guarded stacks must contain resource exhaustion, not hide memory bugs.
use nil_compiler::{SourceProfile, compile_with_profile};
use nil_llvm::{Instrumentation, Optimization, Options};

fn exhausted(source: &str, stack_bytes: u64, instrumentation: Instrumentation) {
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    for optimization in [Optimization::O0, Optimization::O2] {
        let output = nil_llvm::run_arguments(
            &program.hir,
            Options {
                stack_bytes,
                instrumentation,
                optimization,
                steps: u64::MAX,
                call_depth: 200_000,
                ..Options::default()
            },
            &[],
        )
        .unwrap();
        assert_eq!(
            output.status.code(),
            Some(1),
            "{optimization:?}: {:?}",
            output.stderr
        );
        assert!(output.stderr.starts_with(b"E008 "), "{:?}", output.stderr);
        assert_eq!(output.stderr.iter().filter(|b| **b == b'\n').count(), 1);
    }
}

#[test]
fn unbounded_sequence_recursion_exhausts_guarded_stack_cleanly() {
    for size in [64 * 1024, 1024 * 1024, 8 * 1024 * 1024] {
        exhausted(
            "=#b(200000)\n1:s=a<1?!bytes(1,0):!concat(b(a-1),!bytes(1,1))",
            size,
            Instrumentation::Unbounded,
        );
    }
}
#[test]
fn unbounded_mutual_recursion_exhausts_guarded_stack_cleanly() {
    exhausted(
        "=#b(200000)\n1:s=a<1?\"x\":!concat(c(a-1),\"x\")\n1:s=a<1?\"x\":!concat(b(a-1),\"x\")",
        64 * 1024,
        Instrumentation::Unbounded,
    );
}
#[test]
fn large_record_frames_probe_guard_even_at_o0() {
    let array = format!(
        "[{}]",
        (0..256)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let fields = (0..15)
        .map(|n| format!("f{n}:256"))
        .collect::<Vec<_>>()
        .join(",");
    let values = vec![array; 15].join(",");
    let source = format!(
        "record R({fields})\n=#b(200000,R({values}))\n(i,R):s=a<1?\"x\":!concat(b(a-1,b),!bytes(1,b.f0[a%256]))"
    );
    for mode in [Instrumentation::Bounded, Instrumentation::Unbounded] {
        exhausted(&source, 1024 * 1024, mode);
    }
}
#[test]
fn native_stack_size_rejects_too_small_before_building() {
    let program = compile_with_profile("=42", SourceProfile::ExprV5).unwrap();
    assert_eq!(
        nil_llvm::entry_runtime(
            &program.hir,
            Options {
                stack_bytes: 4096,
                ..Options::default()
            }
        )
        .unwrap_err()
        .code,
        "E011"
    );
}
#[test]
fn non_guard_bad_access_is_not_converted_into_e008() {
    let folder = std::env::temp_dir().join(format!("nil-stack-fault-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let source = folder.join("fault.c");
    let binary = folder.join("fault");
    let body = "#define _GNU_SOURCE 1\n#define _DARWIN_C_SOURCE 1\n#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\nstatic int nil_program_main(int argc,char **argv) { (void)argc;(void)argv; volatile uintptr_t address=0; return *(volatile int *)address; }\n";
    std::fs::write(
        &source,
        format!(
            "{body}{}",
            include_str!("../src/stack.c").replace("$STACK_BYTES", "1048576")
        ),
    )
    .unwrap();
    let clang = std::env::var_os("NIL_SANITIZER_CLANG")
        .or_else(|| std::env::var_os("NIL_CLANG"))
        .unwrap_or_else(|| "clang".into());
    let built = std::process::Command::new(clang)
        .args(["-std=c11", "-pthread", "-O0"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(built.status.success(), "{:?}", built.stderr);
    let output = std::process::Command::new(binary).output().unwrap();
    assert!(output.status.code().is_none(), "{:?}", output.stderr);
    assert!(!output.stderr.starts_with(b"E008"));
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn zero_slot_record_recursion_still_has_a_guarded_native_stack() {
    exhausted(
        "record Z(x:0)\n=#b(200000,Z([]))\n(i,Z):s=a<1?\"x\":!concat(b(a-1,b),!bytes(1,#b.x))",
        64 * 1024,
        Instrumentation::Unbounded,
    );
}

#[test]
fn reference_frames_reach_configured_depth_without_host_recursion() {
    use nil_compiler::evaluator::{Limits, Value, execute_values};
    use nil_hir::FunctionId;
    let program =
        compile_with_profile("=b(2048)\n1=a==0?0:b(a-1)+1", SourceProfile::ExprV5).unwrap();
    let limits = Limits {
        steps: u64::MAX,
        call_depth: 16_384,
        ..Limits::default()
    };
    assert_eq!(
        execute_values(&program.hir, FunctionId(0), &[], limits).unwrap(),
        Value::I64(2048)
    );
    let error = execute_values(
        &program.hir,
        FunctionId(0),
        &[],
        Limits {
            call_depth: 32,
            ..limits
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "E008");
}
