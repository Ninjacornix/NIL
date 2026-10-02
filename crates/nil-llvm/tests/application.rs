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
fn native_live_allocation_quota_and_stdout_effects_are_observable() {
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native("=@(0;a<8192;a+1+#!bytes(8192,0)*0;a)", &[], opt);
        assert!(out.status.success());
        assert_eq!(out.stdout, b"8192\n");
        let out = native(
            "=b(!buffer(3000000,1),!buffer(3000000,2),!buffer(3000000,3))\n(v,v,v)=#a+#b+#c",
            &[],
            opt,
        );
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

#[test]
fn dynamic_ir_reuses_proven_dead_replacements() {
    let safe = compile_with_profile(
        "(s):s=@(a,0;b<#a;a[b:255-a[b]],b+1;a)",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let ir = nil_llvm::emit_llvm(&safe.hir);
    assert!(ir.contains("call ptr @nil_set_unique("));
    assert!(!ir.contains("call ptr @nil_set("));
}

#[test]
fn dynamic_ir_keeps_copying_when_original_reads_remain_live() {
    let aliased = compile_with_profile(
        "(s)=@(a,0,0;b<#a;a[b:255],b+1,c+a[b];c)",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let ir = nil_llvm::emit_llvm(&aliased.hir);
    assert!(ir.contains("call ptr @nil_set("));
    assert!(!ir.contains("call ptr @nil_set_unique("));
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native("(s)=@(a,0,0;b<#a;a[b:255],b+1,c+a[b];c)", &["abc"], opt);
        assert!(out.status.success());
        assert_eq!(out.stdout, b"294\n");
    }
}

#[test]
fn one_mib_file_transform_preserves_every_byte_in_reference_and_native() {
    let source = include_str!("../../../examples/expr-v5/transform_file.nil");
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let folder = std::env::temp_dir().join(format!("nil-v5-file-transform-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let input = folder.join("input");
    let output = folder.join("output");
    let bytes = (0..1024 * 1024)
        .map(|i| (i % 256) as u8)
        .collect::<Vec<_>>();
    let expected = bytes.iter().map(|b| 255 - b).collect::<Vec<_>>();
    std::fs::write(&input, &bytes).unwrap();
    let values = [
        Value::Bytes(input.to_str().unwrap().as_bytes().to_vec().into()),
        Value::Bytes(output.to_str().unwrap().as_bytes().to_vec().into()),
    ];
    let result = nil_compiler::evaluator::execute_values_with_host(
        &p.hir,
        FunctionId(0),
        &values,
        Limits {
            steps: 25_000_000,
            ..Default::default()
        },
        &mut nil_compiler::application::FileHost,
    )
    .unwrap();
    assert_eq!(result, Value::I64(1048576));
    assert_eq!(std::fs::read(&output).unwrap(), expected);
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native(
            source,
            &[input.to_str().unwrap(), output.to_str().unwrap()],
            opt,
        );
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, b"1048576\n");
        assert_eq!(std::fs::read(&output).unwrap(), expected);
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
        let roundtrip = folder.join("roundtrip");
        let out = native(
            source,
            &[output.to_str().unwrap(), roundtrip.to_str().unwrap()],
            opt,
        );
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, b"1048576\n");
        assert_eq!(std::fs::read(roundtrip).unwrap(), bytes);
    }
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn dynamic_length_is_hoisted_and_dead_roots_are_cleared_once() {
    let p = compile_with_profile(
        "(s):s=@(a,0;b<#a;a[b:255-a[b]],b+1;a)",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert_eq!(ir.matches("call i64 @nil_length(").count(), 1);
    let length = ir.find("call i64 @nil_length(").unwrap();
    assert!(
        length < ir.find("phi ptr").unwrap(),
        "length must dominate the loop"
    );
    let p = compile_with_profile("(s)=b(a)\n(s)=#a", SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert_eq!(
        ir.lines()
            .filter(
                |line| line.contains("call void @nil_root_store(") && line.contains(", ptr null)")
            )
            .count(),
        2
    );
}

#[test]
fn changing_sequence_length_is_not_hoisted() {
    let source = "(s)=@(a,0;b<3;!concat(a,\"x\"),b+1;#a)";
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native(source, &["abc"], opt);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, b"6\n");
    }
}

#[test]
fn native_bulk_read_streams_binary_input_through_eof() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let p = compile_with_profile("(s):s=!read(a)", SourceProfile::ExprV5).unwrap();
    let folder = std::env::temp_dir().join(format!("nil-v5-pipe-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    for opt in [Optimization::O0, Optimization::O2] {
        let executable = folder.join("read");
        nil_llvm::build(
            &p.hir,
            &executable,
            Options {
                optimization: opt,
                ..Default::default()
            },
        )
        .unwrap();
        for size in [0, 65535, 65536, 65537, 131073] {
            let bytes = (0..size).map(|i| (i % 256) as u8).collect::<Vec<_>>();
            let mut child = Command::new(&executable)
                .arg("/dev/stdin")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let mut stdin = child.stdin.take().unwrap();
            let sent = bytes.clone();
            let writer = std::thread::spawn(move || stdin.write_all(&sent).unwrap());
            let out = child.wait_with_output().unwrap();
            writer.join().unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(out.stderr.is_empty());
            assert_eq!(out.stdout, [bytes, vec![b'\n']].concat());
        }
    }
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn bulk_read_quota_admission_matches_reference_at_exact_boundary() {
    let source = "(s)=b(!bytes(67100000,0),a)\n(s,s)=#!read(b)+#a";
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let folder = std::env::temp_dir().join(format!("nil-v5-read-budget-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let input = folder.join("input");
    let path = input.to_str().unwrap();
    let available = 67108864 - 32 - (67100000 + 32) - (path.len() + 32);
    for length in [available, available + 1] {
        std::fs::write(&input, vec![0xFF; length]).unwrap();
        let result = nil_compiler::evaluator::execute_values_with_host(
            &p.hir,
            FunctionId(0),
            &[Value::Bytes(path.as_bytes().to_vec().into())],
            Limits::default(),
            &mut nil_compiler::application::FileHost,
        );
        if length == available {
            assert_eq!(result.unwrap(), Value::I64((67100000 + length) as i64));
        } else {
            assert_eq!(result.unwrap_err().code, "E013");
        }
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &[path], opt);
            if length == available {
                assert!(
                    out.status.success(),
                    "{}",
                    String::from_utf8_lossy(&out.stderr)
                );
                assert_eq!(out.stdout, format!("{}\n", 67100000 + length).as_bytes());
            } else {
                assert!(!out.status.success());
                assert!(out.stderr.starts_with(b"E013 "));
            }
        }
    }
    std::fs::remove_dir_all(folder).unwrap();
}
