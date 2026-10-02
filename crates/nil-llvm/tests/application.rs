use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::FunctionId;
use nil_llvm::{Instrumentation, Optimization, Options};
use std::process::Output;

fn native(source: &str, args: &[&str], optimization: Optimization) -> Output {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    nil_llvm::run_arguments(
        &p.hir,
        Options {
            optimization,
            ..Default::default()
        },
        &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
    .unwrap()
}
#[test]
fn application_native_matches_reference_at_both_optimization_levels() {
    for (source, args, values, expected) in [
        (
            "(v)=@(a,0,0;b<#a;a,b+1,c+a[b];c)",
            vec!["[1,-2,3]"],
            vec![Value::Buffer(vec![1, -2, 3].into())],
            b"2\n".to_vec(),
        ),
        (
            "(s):s=!concat(\"hello \",a)",
            vec!["世界"],
            vec![Value::Bytes("世界".as_bytes().to_vec().into())],
            "hello 世界\n".as_bytes().to_vec(),
        ),
        (
            "(v):v=!concat(a,!slice(a,0,1))",
            vec!["[4,5]"],
            vec![Value::Buffer(vec![4, 5].into())],
            b"[4,5,4]\n".to_vec(),
        ),
        (
            "(v)=b(a[0:9])+a[0]\n(v)=a[0]",
            vec!["[3]"],
            vec![Value::Buffer(vec![3].into())],
            b"12\n".to_vec(),
        ),
        (":s=\"a\\0b\\n\"", vec![], vec![], b"a\0b\n\n".to_vec()),
        (
            "=!parse(!format(-9223372036854775808))",
            vec![],
            vec![],
            b"-9223372036854775808\n".to_vec(),
        ),
        ("=false?!parse(\"bad\"):7", vec![], vec![], b"7\n".to_vec()),
        (
            "1:s=a>0?b(a):\"none\"\n1:s=!format(a)",
            vec!["42"],
            vec![Value::I64(42)],
            b"42\n".to_vec(),
        ),
        (
            "(s):s=@(a,0;b<#a;a[b:a[b]>=97?(a[b]<=122?a[b]-32:a[b]):a[b]],b+1;a)",
            vec!["héllo"],
            vec![Value::Bytes("héllo".as_bytes().to_vec().into())],
            "HéLLO\n".as_bytes().to_vec(),
        ),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let result = execute_values(&p.hir, FunctionId(0), &values, Limits::default()).unwrap();
        let rendered = match result {
            Value::I64(v) => format!("{v}\n").into_bytes(),
            Value::Bytes(v) => [v.as_ref(), b"\n"].concat(),
            Value::Buffer(v) => format!(
                "[{}]\n",
                v.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
            )
            .into_bytes(),
            _ => unreachable!(),
        };
        assert_eq!(rendered, expected);
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &args, opt);
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(out.stdout, expected, "{source}");
        }
    }
}
#[test]
fn native_buffer_length_is_runtime_sized() {
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native("1=#!buffer(a,7)", &["1024"], opt);
        assert!(out.status.success());
        assert_eq!(out.stdout, b"1024\n");
    }
}
#[test]
fn native_dynamic_failures_match_reference_codes() {
    for (source, code) in [
        ("=!buffer(2,0)[2]", "E012"),
        ("=\"x\"[-1]", "E012"),
        (":s=!slice(\"x\",1,1)", "E012"),
        (":s=\"x\"[0:256]", "E014"),
        (":s=!bytes(1,-1)", "E014"),
        (":v=!buffer(-1,0)", "E013"),
        (":v=!buffer(9223372036854775807,0)", "E013"),
        (":v=!buffer(8388608,0)", "E013"),
        (":s=!bytes(67108864,0)", "E013"),
        ("=!parse(\"01\")", "E016"),
        ("=!parse(\"+1\")", "E016"),
        ("=!parse(\"-0\")", "E016"),
        ("=!parse(\"9223372036854775808\")", "E016"),
        (":s=!read(\"a\\0b\")", "E017"),
    ] {
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &[], opt);
            assert!(!out.status.success(), "{source}");
            assert!(
                out.stderr.starts_with(code.as_bytes()),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}
#[test]
fn native_typed_input_validation_and_empty_sequences() {
    for args in ["[1,]", "[9223372036854775808]", "[x]", "[1]extra"] {
        let out = native("(v)=#a", &[args], Optimization::O2);
        assert!(!out.status.success());
        assert!(out.stderr.starts_with(b"E010"));
    }
    for (source, args, expected) in [
        ("(v)=#a", "[]", b"0\n".as_slice()),
        ("(s)=#a", "", b"0\n".as_slice()),
    ] {
        let out = native(source, &[args], Optimization::O2);
        assert!(out.status.success());
        assert_eq!(out.stdout, expected);
    }
}
#[test]
fn dynamic_operations_retain_bounded_execution_checks() {
    let p = compile_with_profile("1=#!buffer(a,0)", SourceProfile::ExprV5).unwrap();
    let out = nil_llvm::run_arguments(
        &p.hir,
        Options {
            instrumentation: Instrumentation::Bounded,
            steps: 1,
            ..Default::default()
        },
        &["1024".into()],
    )
    .unwrap();
    assert!(!out.status.success());
    assert!(out.stderr.starts_with(b"E008"));
}
#[test]
fn native_file_copy_is_binary_exact_and_missing_files_are_diagnosed() {
    let folder = std::env::temp_dir().join(format!("nil-v5-files-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let input = folder.join("input");
    let output = folder.join("output");
    std::fs::write(&input, [0, 255, b'x', b'\n']).unwrap();
    let p = compile_with_profile("(s,s)=!write(b,!read(a))", SourceProfile::ExprV5).unwrap();
    for opt in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &p.hir,
            Options {
                optimization: opt,
                ..Default::default()
            },
            &[
                input.to_string_lossy().into_owned(),
                output.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, b"4\n");
        assert_eq!(std::fs::read(&output).unwrap(), [0, 255, b'x', b'\n']);
    }
    let reference = nil_compiler::evaluator::execute_values_with_host(
        &p.hir,
        FunctionId(0),
        &[
            Value::Bytes(input.to_string_lossy().as_bytes().to_vec().into()),
            Value::Bytes(output.to_string_lossy().as_bytes().to_vec().into()),
        ],
        Limits::default(),
        &mut nil_compiler::application::FileHost,
    )
    .unwrap();
    assert_eq!(reference, Value::I64(4));
    assert_eq!(std::fs::read(&output).unwrap(), [0, 255, b'x', b'\n']);
    let out = native(
        "(s):s=!read(a)",
        &[folder.join("missing").to_str().unwrap()],
        Optimization::O2,
    );
    assert!(!out.status.success());
    assert!(out.stderr.starts_with(b"E015"));
    let untouched = folder.join("untouched");
    let out = native(
        "(s)=false?!write(a,\"bad\"):7",
        &[untouched.to_str().unwrap()],
        Optimization::O2,
    );
    assert!(out.status.success());
    assert!(!untouched.exists());
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn seeded_runtime_buffer_transforms_match_an_independent_oracle() {
    let source = "(v,i):v=@(a,0,b;b<#a;a[b:a[b]*2+c],b+1,c;a)";
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let folder = std::env::temp_dir().join(format!("nil-v5-transform-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    for opt in [Optimization::O0, Optimization::O2] {
        let binary = folder.join("transform");
        nil_llvm::build(
            &p.hir,
            &binary,
            Options {
                optimization: opt,
                ..Default::default()
            },
        )
        .unwrap();
        let mut seed = 0x5130572_u64;
        for n in [0, 1, 2, 7, 31, 257, 1024] {
            let values = (0..n)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    seed as i64
                })
                .collect::<Vec<_>>();
            let delta = seed as i64;
            let expected = values
                .iter()
                .map(|v| v.wrapping_mul(2).wrapping_add(delta))
                .collect::<Vec<_>>();
            assert_eq!(
                execute_values(
                    &p.hir,
                    FunctionId(0),
                    &[Value::Buffer(values.clone().into()), Value::I64(delta)],
                    Limits::default()
                )
                .unwrap(),
                Value::Buffer(expected.clone().into())
            );
            let input = format!(
                "[{}]",
                values
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let output = std::process::Command::new(&binary)
                .args([input, delta.to_string()])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                output.stdout,
                format!(
                    "[{}]\n",
                    expected
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                )
                .as_bytes()
            );
        }
    }
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn native_allocation_quota_and_stdout_effects_are_observable() {
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native("=@(0;a<8192;a+1+#!bytes(8192,0)*0;a)", &[], opt);
        assert!(!out.status.success());
        assert!(out.stderr.starts_with(b"E013"));
        let out = native("=!out(\"hi\")", &[], opt);
        assert!(out.status.success());
        assert_eq!(out.stdout, b"hi2\n");
    }
}

#[test]
fn native_hexadecimal_literals_round_trip_non_utf8_bytes() {
    let source = r#":s=!concat("\xFF\x00", "\x80\x7f")"#;
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    assert_eq!(
        execute_values(&p.hir, FunctionId(0), &[], Limits::default()).unwrap(),
        Value::Bytes(vec![255, 0, 128, 127].into())
    );
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native(source, &[], opt);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, [255, 0, 128, 127, b'\n']);
        assert!(out.stderr.is_empty());
    }
}

#[test]
fn byte_construction_diagnostic_priority_matches_reference() {
    for (source, code) in [
        (":s=!bytes(67108864,256)", "E014"),
        (":s=!bytes(-1,256)", "E013"),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        assert_eq!(
            execute_values(&p.hir, FunctionId(0), &[], Limits::default())
                .unwrap_err()
                .code,
            code
        );
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &[], opt);
            assert!(!out.status.success());
            assert!(out.stderr.starts_with(code.as_bytes()));
        }
    }
}
