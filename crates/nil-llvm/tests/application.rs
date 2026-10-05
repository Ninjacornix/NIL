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
        &mut nil_compiler::application::FileHost::default(),
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
        &mut nil_compiler::application::FileHost::default(),
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
        0
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
    let available = 67108864 - 40 - (67100000 + 40) - (path.len() + 40);
    for length in [available, available + 1] {
        std::fs::write(&input, vec![0xFF; length]).unwrap();
        let result = nil_compiler::evaluator::execute_values_with_host(
            &p.hir,
            FunctionId(0),
            &[Value::Bytes(path.as_bytes().to_vec().into())],
            Limits::default(),
            &mut nil_compiler::application::FileHost::default(),
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

#[test]
fn append_ir_extends_dead_operands_and_copies_for_old_reads() {
    let source = "1=@(!bytes(0,0),0,a;b<c;!concat(a,\"x\"),b+1,c;#a)";
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("call ptr @nil_concat_unique("));
    assert!(!ir.contains("call ptr @nil_concat("));
    let source = "(s)=b(!concat(a,\"x\"),a)\n(s,s)=#a+#b";
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("call ptr @nil_concat("));
    assert!(!ir.contains("call ptr @nil_concat_unique("));
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native(source, &["abc"], opt);
        assert!(out.status.success());
        assert_eq!(out.stdout, b"7\n");
    }
}
#[test]
fn append_relocations_preserve_binary_data_and_caller_aliases() {
    for (source, expected) in [
        (
            "(s):s=@(a,0;b<1025;!concat(a,\"\\xFF\"),b+1;a)",
            [b"abc".as_slice(), &vec![255; 1025], b"\n"].concat(),
        ),
        (
            "(s):s=!concat(b(a),a)\n(s):s=@(a,0;b<1025;!concat(a,\"\\xFF\"),b+1;a)",
            [b"abc".as_slice(), &vec![255; 1025], b"abc\n"].concat(),
        ),
        (
            "(s):s=@(a,0;b<3;!concat(a,a),b+1;a)",
            b"abcabcabcabcabcabcabcabc\n".to_vec(),
        ),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let value = execute_values(
            &p.hir,
            FunctionId(0),
            &[Value::Bytes(b"abc".to_vec().into())],
            Limits::default(),
        )
        .unwrap();
        let Value::Bytes(bytes) = value else {
            panic!("bytes result")
        };
        assert_eq!([bytes.as_ref(), b"\n"].concat(), expected);
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &["abc"], opt);
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(out.stdout, expected);
        }
    }
}
#[test]
fn native_append_quota_counts_spare_capacity() {
    let source = "=#!concat(!concat(!bytes(10000000,0),!bytes(1,0)),!bytes(19000000,0))";
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native(source, &[], opt);
        assert!(!out.status.success());
        assert!(out.stderr.starts_with(b"E013 "));
    }
}
#[test]
fn one_mib_append_has_exact_content_at_both_optimization_levels() {
    let source = "1:s=@(!bytes(0,0),0,a;b<c;!concat(a,\"x\"),b+1,c;a)";
    let expected = [vec![b'x'; 1048576], vec![b'\n']].concat();
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native(source, &["1048576"], opt);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, expected);
        assert!(out.stderr.is_empty());
    }
}

#[test]
fn nested_scalar_lazy_regions_keep_root_traffic_outside_the_loop() {
    let source = "(s)=@(a,0,0;b<#a?(true?true:false):false;a,b+1,c+(a[b]==10?(true?1:1/0):(false?a[#a]:0));c)";
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert_eq!(
        ir.matches("call void @nil_root_store(").count(),
        0,
        "allocation-free scalar functions borrow even their loop state"
    );
    assert!(ir.matches("br i1 ").count() >= 4);
    assert!(
        ir.contains("phi i64"),
        "lazy arms must still join through CFG edges"
    );
    let borrowed = compile_with_profile("(s)=true?a[0]:a[#a]", SourceProfile::ExprV5).unwrap();
    assert_eq!(
        nil_llvm::emit_llvm(&borrowed.hir)
            .matches("call void @nil_root_store(")
            .count(),
        0
    );
    let scalar = compile_with_profile("1=a>0?(a==2?3:4):5", SourceProfile::ExprV5).unwrap();
    assert!(!nil_llvm::emit_llvm(&scalar.hir).contains("call ptr @nil_roots_enter("));
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native(source, &["a\nb\n"], opt);
        assert!(out.status.success(), "{out:?}");
        assert_eq!(out.stdout, b"2\n");
        let out = native(source, &[""], opt);
        assert!(out.status.success());
        assert_eq!(out.stdout, b"0\n");
    }
}

#[test]
fn scalar_lazy_traps_and_short_circuit_stay_on_selected_edges() {
    for (source, input, expected) in [
        ("(s)=false?b(a):42\n(s)=a[#a]", "Z", b"42\n".as_slice()),
        ("(s)=true?b(a):1/0\n(s)=a[0]", "Z", b"90\n".as_slice()),
        ("(s)=#a==0?7:(true?a[0]:1/0)", "", b"7\n".as_slice()),
        ("(s)=#a==0?7:(false?a[#a]:a[0])", "Z", b"90\n".as_slice()),
        ("(s)=(false?(a[#a]==0):false)?1:9", "Z", b"9\n".as_slice()),
        ("(s)=(true?true:(1/0==0))?8:1", "Z", b"8\n".as_slice()),
    ] {
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &[input], opt);
            assert!(out.status.success(), "{source}: {out:?}");
            assert_eq!(out.stdout, expected);
            assert!(out.stderr.is_empty());
        }
    }
    for (source, code) in [
        ("(s)=true?1/0:a[#a]", "E009"),
        ("(s)=false?1/0:a[#a]", "E012"),
    ] {
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &["Z"], opt);
            assert!(!out.status.success());
            assert!(String::from_utf8_lossy(&out.stderr).starts_with(code));
        }
    }
}

#[test]
fn lazy_host_effects_and_failure_order_match_the_reference() {
    use nil_compiler::{application::Host, evaluator::execute_values_with_host};
    #[derive(Default)]
    struct Capture(Vec<u8>);
    impl Host for Capture {
        fn out(&mut self, bytes: &[u8]) -> Result<(), nil_hir::Diagnostic> {
            self.0.extend_from_slice(bytes);
            Ok(())
        }
    }
    for (source, effects, result) in [
        (
            "=!out(\"A\")+(false?b():c())+!out(\"D\")\n=!out(\"BAD\")+1/0\n=!out(\"B\")+!out(\"C\")",
            b"ABCD".as_slice(),
            Ok(4),
        ),
        (
            "=!out(\"A\")+(true?b():c())+!out(\"D\")\n=!out(\"B\")+1/0\n=!out(\"BAD\")",
            b"AB".as_slice(),
            Err("E009"),
        ),
        (
            "=!out(\"A\")+(true?!out(\"B\"):!out(\"X\"))+!out(\"C\")",
            b"ABC".as_slice(),
            Ok(3),
        ),
        (
            "=!out(\"A\")+(false?!out(\"BAD\"):1)+!out(\"C\")",
            b"AC".as_slice(),
            Ok(3),
        ),
        (
            "=b(!bytes(3,90))\n(s)=!out(\"A\")+@(a,0,0;b<#a;a,b+1,c+(b==1?!out(\"B\"):!out(\"C\"));!out(a)+c)+!out(\"D\")",
            b"ACBCZZZD".as_slice(),
            Ok(8),
        ),
        (
            "=!out(\"A\")+(true?!out(\"B\")+1/0:!out(\"X\"))+!out(\"C\")",
            b"AB".as_slice(),
            Err("E009"),
        ),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let mut host = Capture::default();
        let reference =
            execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), &mut host);
        assert_eq!(host.0, effects);
        match result {
            Ok(n) => assert_eq!(reference.unwrap(), Value::I64(n)),
            Err(code) => assert_eq!(reference.unwrap_err().code, code),
        }
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &[], opt);
            let mut expected = effects.to_vec();
            match result {
                Ok(n) => {
                    expected.extend_from_slice(format!("{n}\n").as_bytes());
                    assert!(out.status.success());
                    assert!(out.stderr.is_empty());
                }
                Err(code) => {
                    assert!(!out.status.success());
                    assert!(String::from_utf8_lossy(&out.stderr).starts_with(code));
                }
            }
            assert_eq!(out.stdout, expected);
        }
    }
}

#[test]
fn lazy_allocations_and_sequence_results_keep_conservative_roots() {
    for (source, expected) in [
        ("(s)=(true?a[0]:0)+#!bytes(1,0)", b"91\n".as_slice()),
        (
            "(s)=(true?!parse(!format(a[0])):0)+a[0]",
            b"180\n".as_slice(),
        ),
        ("(s):s=@(a,0;b<2;(b==0?a:a),b+1;a)", b"Z\n".as_slice()),
    ] {
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &["Z"], opt);
            assert!(out.status.success(), "{source}: {out:?}");
            assert_eq!(out.stdout, expected);
        }
    }
    let p = compile_with_profile(
        "(s):s=@(a,0;b<2;(b==0?a:!bytes(1,0)),b+1;a)",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert!(
        nil_llvm::emit_llvm(&p.hir)
            .matches("call void @nil_root_store(")
            .count()
            > 4
    );
}

#[test]
fn borrowing_summary_crosses_calls_and_keeps_allocating_callees_conservative() {
    for (source, eligible) in [
        ("(s)=b(a,0)\n(s,i)=a[b]==10?1:0", vec![true, true]),
        ("(s)=b(a)\n(s)=!parse(a)", vec![true, true]),
        ("(s)=b(a)\n(s)=#!concat(a,a)", vec![false, false]),
        ("(s)=b(a)\n(s)=!out(a)", vec![false, false]),
        ("(s):s=b(a)\n(s):s=a", vec![true, true]),
        (
            "(s,i)=b(a,b)\n(s,i)=b>0?c(a,b-1)+a[0]:#a\n(s,i)=b>0?b(a,b-1)+a[0]:#a",
            vec![true, true, true],
        ),
        (
            "(s,i)=b(a,b)\n(s,i)=b>0?c(a,b-1):#a\n(s,i)=b>0?b(a,b-1):#!bytes(1,0)",
            vec![false, false, false],
        ),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let proof = nil_hir::borrowing::Summaries::analyze(&p.hir);
        for (i, expected) in eligible.into_iter().enumerate() {
            assert_eq!(proof.function(i), expected, "{source}: function {i}");
        }
    }
}

#[test]
fn scalar_helpers_and_nested_identity_loops_have_no_root_frame() {
    for (source, expected) in [
        (
            "(s)=@(a,0,0;b<#a;a,b+1,c+b(a,b);c)\n(s,i)=a[b]==10?1:0",
            b"2\n".as_slice(),
        ),
        (
            "(s)=@(a,0,0;b<#a;a,b+1,c+@(a,b,0,0;c<1;a,b,c+1,d+b(a,b);d);c)\n(s,i)=a[b]==10?1:0",
            b"2\n".as_slice(),
        ),
        (
            "(s)=@(a,0,0;b<3;a,b+1,c+b(a);c)\n(s)=!parse(a)",
            b"126\n".as_slice(),
        ),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let ir = nil_llvm::emit_llvm(&p.hir);
        assert!(!ir.contains("call void @nil_root_store("), "{source}");
        assert!(!ir.contains("call ptr @nil_roots_enter("), "{source}");
        // The emitter keeps calls: this is not a source-level inline transformation.
        assert!(ir.contains("call i64 @nil_fn1("));
        for opt in [Optimization::O0, Optimization::O2] {
            let arg = if source.contains("!parse") {
                "42"
            } else {
                "a\nb\n"
            };
            let out = native(source, &[arg], opt);
            assert!(out.status.success(), "{source}: {out:?}");
            assert_eq!(out.stdout, expected);
        }
    }
}

#[test]
fn borrowing_recursion_keeps_depth_checks_and_sequence_reads() {
    for source in [
        "(s,i)=b(a,b)\n(s,i)=b>0?b(a,b-1)+a[0]:#a",
        "(s,i)=b(a,b)\n(s,i)=b>0?c(a,b-1)+a[0]:#a\n(s,i)=b>0?b(a,b-1)+a[0]:#a",
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let ir = nil_llvm::emit_llvm_with_instrumentation(&p.hir, Instrumentation::Bounded);
        assert!(!ir.contains("call ptr @nil_roots_enter("));
        assert!(ir.contains("call void @nil_enter("));
        for opt in [Optimization::O0, Optimization::O2] {
            for depth in [0, 1, 3, 253, 254, 255] {
                let args = vec!["Z".to_string(), depth.to_string()];
                let reference = execute_values(
                    &p.hir,
                    FunctionId(0),
                    &[Value::Bytes(vec![90].into()), Value::I64(depth)],
                    Limits::default(),
                );
                let out = nil_llvm::run_arguments(
                    &p.hir,
                    Options {
                        optimization: opt,
                        instrumentation: Instrumentation::Bounded,
                        ..Default::default()
                    },
                    &args,
                )
                .unwrap();
                match reference {
                    Ok(Value::I64(n)) => {
                        assert!(out.status.success(), "{out:?}");
                        assert_eq!(out.stdout, format!("{n}\n").as_bytes());
                    }
                    Err(error) => {
                        assert!(!out.status.success());
                        assert!(String::from_utf8_lossy(&out.stderr).starts_with(error.code));
                    }
                    other => panic!("unexpected result {other:?}"),
                }
            }
        }
    }
}

#[test]
fn allocating_recursive_frames_and_call_aliases_stay_rooted() {
    for source in [
        "(s,i)=b(a,b)+a[0]\n(s,i)=b>0?b(!concat(a,\"x\"),b-1)+a[0]:#a",
        "(s)=@(a,0,0;b<3;a,b+1,c+b(a)+a[0];c)\n(s)=#!concat(a,a)",
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        assert!(!nil_hir::borrowing::Summaries::analyze(&p.hir).function(1));
        assert!(nil_llvm::emit_llvm(&p.hir).contains("call ptr @nil_roots_enter("));
        let args = if source.starts_with("(s,i)") {
            vec!["Z", "17"]
        } else {
            vec!["Z"]
        };
        let mut values = vec![Value::Bytes(vec![90].into())];
        if args.len() == 2 {
            values.push(Value::I64(17));
        }
        let Value::I64(expected) =
            execute_values(&p.hir, FunctionId(0), &values, Limits::default()).unwrap()
        else {
            panic!()
        };
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &args, opt);
            assert!(out.status.success(), "{out:?}");
            assert_eq!(out.stdout, format!("{expected}\n").as_bytes());
        }
    }
}

#[test]
fn borrowed_sequence_returns_and_allocating_counterexamples_keep_lifetimes() {
    for (source, borrowing, args) in [
        ("(s):s=b(a)\n(s):s=a", true, vec!["abc"]),
        ("(s):s=b(a,false)\n(s,b):s=b?a:a", true, vec!["abc"]),
        ("(s):s=b(a)\n(s):s=!slice(a,0,#a)", false, vec!["abc"]),
        ("(s):s=b(a)\n(s):s=true?a:!bytes(1,0)", false, vec!["abc"]),
        (
            "(s,i):s=b(a,b)\n(s,i):s=b>0?b(a,b-1):a",
            true,
            vec!["abc", "23"],
        ),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        assert_eq!(
            nil_hir::borrowing::Summaries::analyze(&p.hir).function(1),
            borrowing
        );
        let ir = nil_llvm::emit_llvm(&p.hir);
        assert_eq!(!ir.contains("call void @nil_root_store("), borrowing);
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &args, opt);
            assert!(out.status.success(), "{out:?}");
            assert_eq!(out.stdout, b"abc\n");
        }
    }
    // Borrowed results must be rooted by an allocating caller before collection.
    let source = "(s)=c(b(a),!bytes(67108600,0))+a[0]\n(s):s=a\n(s,s)=a[0]+#b";
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native(source, &["Z"], opt);
        assert!(out.status.success(), "{out:?}");
        assert_eq!(out.stdout, b"67108780\n");
    }
}

#[test]
fn induction_reads_are_direct_but_exit_computed_and_mismatched_ranges_stay_checked() {
    let direct = "(s)=@(a,0,0;b<#a;a,b+1,c+a[b];c)";
    let p = compile_with_profile(direct, SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("load i8") && ir.contains("i64 40"));
    assert!(!ir.contains("call i64 @nil_get("));
    for (source, args, code) in [
        ("(s)=@(a,0;b<#a;a,b+1;a[b])", vec!["abc"], "E012"),
        (
            "(s,i)=@(a,b,0;b<#a;a,b+1,c+a[b];c)",
            vec!["abc", "-1"],
            "E012",
        ),
        ("(s)=@(a,0,0;b<#a+1;a,b+1,c+a[b];c)", vec!["abc"], "E012"),
        ("(s)=@(a,0,0;b<#a;a,b+1,c+a[b+1];c)", vec!["abc"], "E012"),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        assert!(nil_llvm::emit_llvm(&p.hir).contains("call i64 @nil_get("));
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &args, opt);
            assert!(!out.status.success());
            assert!(String::from_utf8_lossy(&out.stderr).starts_with(code));
        }
    }
    for args in [vec![""], vec!["abc"]] {
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(direct, &args, opt);
            assert!(out.status.success());
            assert_eq!(
                out.stdout,
                if args[0].is_empty() {
                    b"0\n".as_slice()
                } else {
                    b"294\n"
                }
            );
        }
    }
    for opt in [Optimization::O0, Optimization::O2] {
        let out = native("(v)=@(a,0,0;b<#a;a,b+1,c+a[b];c)", &["[1,-2,3]"], opt);
        assert!(out.status.success());
        assert_eq!(out.stdout, b"2\n");
    }
}

#[test]
fn new_sequence_operations_match_reference_at_o0_and_o2() {
    for (source, expected) in [
        (":b=!equal(\"\\xFF\\0\",\"\\xFF\\0\")", "true\n"),
        (":b=!equal(\"abc\",\"abd\")", "false\n"),
        (":b=!equal(!buffer(0,9),!buffer(0,1))", "true\n"),
        (":b=!equal(!buffer(2,-1),!buffer(2,1))", "false\n"),
        ("=!find(\"x\\0\\xFF\",255,0)", "2\n"),
        ("=!find(\"aba\",97,1)", "2\n"),
        ("=!find(\"aba\",97,3)", "3\n"),
        ("=!find(!buffer(2,-7),-7,1)", "1\n"),
        ("=!find(\"\",0,0)", "0\n"),
        (":v=!parsebuf(\"\",\"\\n\")", "[]\n"),
        (":v=!parsebuf(\"1,-2\\n3\\n\",\",\\n\")", "[1,-2,3]\n"),
        (":v=!parsebuf(\"1\\xFF2\",\"\\xFF\")", "[1,2]\n"),
        (":v=!parsebuf(\"42\",\"\")", "[42]\n"),
        (
            ":v=!parsebuf(\"-9223372036854775808,9223372036854775807\",\",\")",
            "[-9223372036854775808,9223372036854775807]\n",
        ),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let result = execute_values(&p.hir, FunctionId(0), &[], Limits::default()).unwrap();
        let rendered = match result {
            Value::I64(value) => format!("{value}\n"),
            Value::Bool(value) => format!("{value}\n"),
            Value::Buffer(values) => format!(
                "[{}]\n",
                values
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            _ => panic!("unexpected new operation result"),
        };
        assert_eq!(rendered, expected, "{source}");
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &[], opt);
            assert!(
                out.status.success(),
                "{source}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(out.stdout, expected.as_bytes(), "{source}");
        }
    }
}

#[test]
fn new_sequence_failures_match_codes_and_priority_at_o0_and_o2() {
    for (source, code) in [
        ("=!find(\"x\",256,-1)", "E012"),
        ("=!find(\"\",256,0)", "E014"),
        ("=!find(\"x\",0,2)", "E012"),
        (":v=!parsebuf(\"1,,2\",\",\")", "E016"),
        (":v=!parsebuf(\",1\",\",\")", "E016"),
        (":v=!parsebuf(\"1,,\",\",\")", "E016"),
        (":v=!parsebuf(\"01\",\",\")", "E016"),
        (":v=!parsebuf(\"-0\",\",\")", "E016"),
        (":v=!parsebuf(\"9223372036854775808\",\",\")", "E016"),
        (":v=!parsebuf(!bytes(8388608,10),\"\\n\")", "E013"),
        ("=!find(!bytes(1,256),256,-1)", "E014"),
        (
            "=b(!bytes(33554432,0))\n(s)=#!parsebuf(!bytes(4194304,10),\"\\n\")+#a",
            "E013",
        ),
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
            assert!(
                String::from_utf8_lossy(&out.stderr).starts_with(code),
                "{source}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}

#[test]
fn new_operations_preserve_lazy_host_effects_and_aliases_at_o0_and_o2() {
    for (source, expected) in [
        ("=false?#!parsebuf(\"bad\",\",\"):7", b"7\n".as_slice()),
        (
            "=!out(\"A\")+(!equal(\"x\",\"x\")?b():!out(\"BAD\"))+!out(\"C\")\n=!out(\"B\")",
            b"ABC3\n".as_slice(),
        ),
        (
            "=b(!parsebuf(\"1,2\",\",\"))\n(v)=#!concat(a,!parsebuf(\"3\",\",\"))+a[1]",
            b"5\n".as_slice(),
        ),
        (
            "(v):v=@(a,0;b<10;!concat(a,!parsebuf(\"3\",\",\")),b+1;a)",
            b"[1,2,3,3,3,3,3,3,3,3,3,3]\n".as_slice(),
        ),
    ] {
        let args = if source.starts_with("(v)") {
            vec!["[1,2]"]
        } else {
            vec![]
        };
        for opt in [Optimization::O0, Optimization::O2] {
            let out = native(source, &args, opt);
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(out.stdout, expected);
        }
    }
}

#[test]
fn equality_search_are_borrowing_but_bulk_parse_retains_roots_and_concat_reuse() {
    let borrowed = compile_with_profile(
        "(s)=@(a,0;b<#a;a,!find(a,10,b)+1;#a)",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert!(nil_hir::borrowing::Summaries::analyze(&borrowed.hir).function(0));
    assert!(!nil_llvm::emit_llvm(&borrowed.hir).contains("call ptr @nil_roots_enter("));
    let allocating =
        compile_with_profile("(s):v=!parsebuf(a,\",\")", SourceProfile::ExprV5).unwrap();
    assert!(!nil_hir::borrowing::Summaries::analyze(&allocating.hir).function(0));
    assert!(nil_llvm::emit_llvm(&allocating.hir).contains("call ptr @nil_roots_enter("));
    let append = compile_with_profile(
        "(v):v=@(a,0;b<3;!concat(a,!parsebuf(\"3\",\",\")),b+1;a)",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert!(nil_llvm::emit_llvm(&append.hir).contains("call ptr @nil_concat_unique("));
}

#[test]
fn parsed_buffer_concat_keeps_copying_when_old_reads_are_live() {
    let source = "=b(!parsebuf(\"1,2\",\",\"))\n(v)=#!concat(a,!parsebuf(\"3\",\",\"))+a[1]";
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("call ptr @nil_concat("));
    assert!(!ir.contains("call ptr @nil_concat_unique("));
}

#[test]
fn each_and_sort_preserve_called_lazy_effects_and_left_to_right_state_order() {
    use nil_compiler::{application::Host, evaluator::execute_values_with_host};
    #[derive(Default)]
    struct Capture(Vec<u8>);
    impl Host for Capture {
        fn out(&mut self, bytes: &[u8]) -> Result<(), nil_hir::Diagnostic> {
            self.0.extend_from_slice(bytes);
            Ok(())
        }
    }
    for (source, effects, expected) in [
        (
            "=!each(\"xy\",!out(\"I\"),!out(\"J\");c+(a==0?b():!out(\"C\")),d+!out(\"D\");!out(\"F\")+a+b)+!out(\"G\")\n=!out(\"B\")+(false?!out(\"BAD\"):0)",
            "IJBDC DFG".replace(' ', ""),
            8,
        ),
        (
            "=!out(\"A\")+(false?!each([1],0;!out(\"BAD\")+1/0;a):#!sort(\"ba\",!out(\"B\")-1))+!out(\"C\")",
            "ABC".to_string(),
            4,
        ),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let mut host = Capture::default();
        let value =
            execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), &mut host)
                .unwrap();
        assert_eq!(value, Value::I64(expected));
        assert_eq!(host.0, effects.as_bytes());
        for opt in [Optimization::O0, Optimization::O2] {
            let output = native(source, &[], opt);
            assert!(output.status.success());
            assert_eq!(output.stdout, format!("{effects}{expected}\n").as_bytes());
        }
    }
}
