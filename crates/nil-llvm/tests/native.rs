#![cfg(any(target_os = "macos", target_os = "linux"))]
use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, execute},
};
use nil_llvm::{Optimization, Options, build, emit_llvm};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "nil-native-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(binary: &std::path::Path, args: &[i64]) -> Output {
    Command::new(binary)
        .args(args.iter().map(i64::to_string))
        .output()
        .unwrap()
}
fn differential(source: &str, cases: &[Vec<i64>], options: Options) {
    differential_profile(source, cases, options, SourceProfile::ExprV2);
}
fn differential_profile(
    source: &str,
    cases: &[Vec<i64>],
    options: Options,
    profile: SourceProfile,
) {
    let program = compile_with_profile(source, profile).unwrap();
    let directory = Directory::new();
    let binary = directory.0.join("program");
    let stats = build(&program.hir, &binary, options).unwrap();
    assert!(stats.binary_bytes > 0 && stats.llvm_codegen_ns > 0 && stats.link_ns > 0);
    for args in cases {
        let expected = execute(
            &program.hir,
            options.entry,
            args,
            Limits {
                steps: options.steps,
                call_depth: options.call_depth as usize,
            },
        );
        let actual = run(&binary, args);
        match expected {
            Ok(value) => {
                assert!(
                    actual.status.success(),
                    "{}",
                    String::from_utf8_lossy(&actual.stderr)
                );
                assert_eq!(
                    String::from_utf8(actual.stdout).unwrap(),
                    format!("{value}\n")
                );
            }
            Err(error) => {
                assert_eq!(actual.status.code(), Some(1));
                assert!(actual.stdout.is_empty());
                assert_eq!(
                    String::from_utf8(actual.stderr).unwrap(),
                    format!("{error}\n")
                );
            }
        }
    }
}
#[test]
fn arithmetic_signed_bounds_division_and_traps_agree_at_both_optimization_levels() {
    for optimization in [Optimization::O0, Optimization::O2] {
        let options = Options {
            optimization,
            ..Default::default()
        };
        for source in ["2=a+b", "2=a-b", "2=a*b", "2=a/b"] {
            differential(
                source,
                &[
                    vec![20, 22],
                    vec![-9, 2],
                    vec![i64::MIN, -1],
                    vec![i64::MAX, 1],
                    vec![0, 0],
                ],
                options,
            );
        }
    }
}
#[test]
fn lazy_branches_and_recursive_call_continuations_agree() {
    for optimization in [Optimization::O0, Optimization::O2] {
        let options = Options {
            optimization,
            ..Default::default()
        };
        differential("=true?42:1/0", &[vec![]], options);
        differential("=false?9223372036854775807+1:42", &[vec![]], options);
        differential("1=a<=1?1:a*a(a-1)", &[vec![0], vec![10], vec![21]], options);
        differential(
            "2=a>b?a:b",
            &[vec![i64::MIN, i64::MAX], vec![7, 7], vec![9, -2]],
            options,
        );
    }
}
#[test]
fn all_control_flow_algorithms_and_boundaries_agree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/paired/control-samples");
    for optimization in [Optimization::O0, Optimization::O2] {
        let options = Options {
            optimization,
            ..Default::default()
        };
        for (name, cases) in [
            ("factorial", (0..=21).map(|n| vec![n]).collect::<Vec<_>>()),
            ("fibonacci", (0..=93).map(|n| vec![n]).collect()),
            ("counted_sum", vec![vec![0], vec![1], vec![1000]]),
            (
                "max",
                vec![vec![-10, 2], vec![0, 0], vec![i64::MIN, i64::MAX]],
            ),
            (
                "gcd",
                vec![vec![1071, 462], vec![0, 0], vec![9, 0], vec![0, 9]],
            ),
            ("nested_sum", vec![vec![0], vec![1], vec![20]]),
            (
                "absolute",
                vec![vec![i64::MIN], vec![-9], vec![0], vec![i64::MAX]],
            ),
            (
                "clamp",
                vec![vec![12, 0, 10], vec![-5, 0, 10], vec![5, 0, 10]],
            ),
        ] {
            for profile in [SourceProfile::ExprV2, SourceProfile::ExprV3] {
                differential_profile(
                    &fs::read_to_string(root.join(format!("{name}.v2.nil"))).unwrap(),
                    &cases,
                    options,
                    profile,
                );
            }
        }
    }
}
#[test]
fn loop_state_is_parallel_and_bool_state_is_typed() {
    differential(
        "=@(1,2,1;c>0;b,a,c-1;a*10+b)",
        &[vec![]],
        Options::default(),
    );
    differential("=@(true,0;a;b<4,b+1;b)", &[vec![]], Options::default());
    differential(
        "1=@(0,a;b>0?(b<3?true:false):false;a+1,b-1;a)",
        &[vec![0], vec![1], vec![2], vec![4]],
        Options::default(),
    );
}
#[test]
fn exact_fuel_costs_and_call_depth_failures_agree() {
    for optimization in [Optimization::O0, Optimization::O2] {
        for steps in [0, 1, 5, 6, 7, 30] {
            let options = Options {
                steps,
                optimization,
                ..Default::default()
            };
            differential("=@(;false;;42)", &[vec![]], options);
            differential("=@(;true;;42)", &[vec![]], options);
            differential("=true?(false?0:42):0", &[vec![]], options);
        }
        for call_depth in [0, 1, 3] {
            differential(
                "1=a<=0?42:a(a-1)",
                &[vec![0], vec![10]],
                Options {
                    call_depth,
                    optimization,
                    ..Default::default()
                },
            );
        }
    }
}
#[test]
fn emission_is_deterministic_and_contains_typed_phi_joins() {
    let p = compile_with_profile("1=@(0,a;b>0;a+b,b-1;a)", SourceProfile::ExprV2).unwrap();
    let ir = emit_llvm(&p.hir);
    assert_eq!(ir, emit_llvm(&p.hir));
    assert!(ir.contains("phi i64") && ir.contains("llvm.sadd.with.overflow.i64"));
    assert!(!ir.contains("; phi placeholder"));
}

#[test]
fn bool_function_results_lower_without_surface_syntax_dependencies() {
    use nil_compiler::hir::*;
    let instruction = |operation, ty| Instruction {
        operation,
        ty,
        span: None,
    };
    let p = validate(Program {
        arithmetic: nil_hir::Arithmetic::Checked,
        functions: vec![
            Function {
                parameters: vec![Type::I64],
                result_type: Type::I64,
                instructions: vec![
                    instruction(
                        Operation::Call {
                            function: FunctionId(1),
                            arguments: vec![ValueId(0)],
                        },
                        Type::Bool,
                    ),
                    instruction(
                        Operation::If {
                            condition: ValueId(1),
                            then_region: Region {
                                instructions: vec![],
                                results: vec![ValueId(0)],
                            },
                            else_region: Region {
                                instructions: vec![instruction(Operation::Constant(0), Type::I64)],
                                results: vec![ValueId(2)],
                            },
                        },
                        Type::I64,
                    ),
                ],
                result: ValueId(2),
                return_span: None,
            },
            Function {
                parameters: vec![Type::I64],
                result_type: Type::Bool,
                instructions: vec![
                    instruction(Operation::Constant(0), Type::I64),
                    instruction(
                        Operation::Compare {
                            op: CompareOp::Lt,
                            lhs: ValueId(0),
                            rhs: ValueId(1),
                        },
                        Type::Bool,
                    ),
                ],
                result: ValueId(2),
                return_span: None,
            },
        ],
    })
    .unwrap();
    let directory = Directory::new();
    let binary = directory.0.join("program");
    build(&p, &binary, Options::default()).unwrap();
    assert_eq!(run(&binary, &[-5]).stdout, b"-5\n");
    assert_eq!(run(&binary, &[5]).stdout, b"0\n");
    assert!(
        build(
            &p,
            &binary,
            Options {
                entry: FunctionId(1),
                ..Default::default()
            }
        )
        .is_err()
    );
}
#[test]
fn native_argument_validation_rejects_invalid_and_out_of_range_values() {
    let p = compile_with_profile("1=a", SourceProfile::ExprV2).unwrap();
    let directory = Directory::new();
    let binary = directory.0.join("program");
    build(&p.hir, &binary, Options::default()).unwrap();
    for arg in [
        "",
        " 1",
        "1 ",
        "0x10",
        "9223372036854775808",
        "-9223372036854775809",
    ] {
        let output = Command::new(&binary).arg(arg).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    for arg in [i64::MIN, i64::MAX] {
        assert_eq!(run(&binary, &[arg]).stdout, format!("{arg}\n").as_bytes());
    }
}

#[test]
fn v3_wrapping_native_matches_reference_for_boundaries_at_o0_and_o2() {
    for optimization in [Optimization::O0, Optimization::O2] {
        for source in ["2=a+b", "2=a-b", "2=a*b", "2=a/b", "2=a-a/b*b"] {
            let mut cases = Vec::new();
            for a in [i64::MIN, i64::MIN + 1, -7, -1, 0, 1, 7, i64::MAX] {
                for b in [i64::MIN, -7, -1, 0, 1, 7, i64::MAX] {
                    cases.push(vec![a, b]);
                }
            }
            differential_profile(
                source,
                &cases,
                Options {
                    optimization,
                    ..Default::default()
                },
                SourceProfile::ExprV3,
            );
        }
        differential_profile(
            "=b(20,22)\n2=a+b",
            &[vec![]],
            Options {
                optimization,
                ..Default::default()
            },
            SourceProfile::ExprV3,
        );
    }
}
#[test]
fn v3_native_resource_instrumentation_is_explicit_and_semantics_are_unchanged() {
    let p = compile_with_profile("2=a+b", SourceProfile::ExprV3).unwrap();
    let ir = emit_llvm(&p.hir);
    assert!(!ir.contains("call void @nil_tick"));
    assert!(!ir.contains("call void @nil_enter"));
    assert!(!ir.contains("add nsw"));
    assert!(!ir.contains("add nuw"));
    assert!(ir.contains("add i64"));
    for steps in [0, 1, 2, 3] {
        differential_profile(
            "2=a+b",
            &[vec![i64::MAX, 1]],
            Options {
                steps,
                instrumentation: nil_llvm::Instrumentation::Bounded,
                ..Default::default()
            },
            SourceProfile::ExprV3,
        );
    }
    for call_depth in [0, 1, 3] {
        differential_profile(
            "1=a<=0?0:b(a-1)\n1=a<=0?0:a(a-1)",
            &[vec![4]],
            Options {
                call_depth,
                instrumentation: nil_llvm::Instrumentation::Bounded,
                ..Default::default()
            },
            SourceProfile::ExprV3,
        );
    }
    // No implicit budgets in v3, even when the unused budget fields are zero.
    differential_profile(
        "2=a+b",
        &[vec![i64::MAX, 1]],
        Options::default(),
        SourceProfile::ExprV3,
    );
    let out = nil_llvm::run(
        &p.hir,
        Options {
            steps: 0,
            call_depth: 0,
            ..Default::default()
        },
        &[i64::MAX, 1],
    )
    .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"-9223372036854775808\n");
}

#[test]
fn v3_regions_are_lazy_parallel_and_bool_typed_in_both_execution_modes() {
    for optimization in [Optimization::O0, Optimization::O2] {
        for instrumentation in [
            nil_llvm::Instrumentation::Bounded,
            nil_llvm::Instrumentation::Unbounded,
        ] {
            for source in [
                "=true?42:1/0",
                "=false?1/0:42",
                "=@(1,2,1;c>0;b,a,c-1;a*10+b)",
                "=@(true,0;a;b<4,b+1;b)",
                "1=a<=1?1:a*a(a-1)",
            ] {
                let cases = if source.starts_with("1=") {
                    vec![vec![0], vec![10], vec![21]]
                } else {
                    vec![vec![]]
                };
                differential_profile(
                    source,
                    &cases,
                    Options {
                        optimization,
                        instrumentation,
                        ..Default::default()
                    },
                    SourceProfile::ExprV3,
                );
            }
        }
    }
}
