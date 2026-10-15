use nil_compiler::{
    SourceProfile,
    application::Host,
    compile_with_profile,
    evaluator::{Limits, Value, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId};
use nil_llvm::{Instrumentation, Optimization, Options};
#[derive(Default)]
struct MemoryHost {
    stdout: Vec<u8>,
}
impl Host for MemoryHost {
    fn out(&mut self, bytes: &[u8]) -> Result<(), Diagnostic> {
        self.stdout.extend_from_slice(bytes);
        Ok(())
    }
}
fn parity(source: &str, expected: &[u8], code: Option<&str>, depth: usize) {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    check(&p.hir, expected, code, depth);
}
fn check(p: &nil_hir::ValidatedProgram, expected: &[u8], code: Option<&str>, depth: usize) {
    let mut host = MemoryHost::default();
    let result = execute_values_with_host(
        p,
        FunctionId(0),
        &[],
        Limits {
            steps: 1000000,
            call_depth: depth,
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
            defer_slot_writes: false,
        },
        &mut host,
    );
    if let Some(code) = code {
        assert_eq!(result.unwrap_err().code, code);
        assert_eq!(host.stdout, expected);
    } else {
        let tail = match result.unwrap() {
            Value::I64(v) => format!("{v}\n").into_bytes(),
            Value::Bytes(v) => [v.as_ref(), b"\n"].concat(),
            v => panic!("unexpected test result {v:?}"),
        };
        assert_eq!([host.stdout, tail].concat(), expected);
    }
    for optimization in [Optimization::O0, Optimization::O2] {
        let actual = nil_llvm::run_arguments(
            p,
            Options {
                optimization,
                instrumentation: Instrumentation::Bounded,
                steps: 1000000,
                call_depth: depth as u64,
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert_eq!(actual.stdout, expected, "{optimization:?}");
        if let Some(code) = code {
            assert!(!actual.status.success());
            assert!(
                actual.stderr.starts_with(code.as_bytes()),
                "{:?}",
                actual.stderr
            );
        } else {
            assert!(actual.status.success(), "{:?}", actual.stderr);
            assert!(actual.stderr.is_empty());
        }
    }
}
#[test]
fn std_bytes_map_filter_fold_match_reference_at_o0_and_o2() {
    parity(":s=!map(&b,\"abc\")\n1=a+1", b"bcd\n", None, 256);
    parity(":s=!filter(&b,\"abca\")\n1:b=a==97", b"aa\n", None, 256);
    parity("=!fold(&b,\"abc\",0)\n2=a+b", b"294\n", None, 256);
}
#[test]
fn std_buffer_overloads_empty_sequences_and_aliases_match() {
    parity(
        "=!fold(&b,!map(&c,!buffer(3,2)),0)\n2=a+b\n1=a*3",
        b"18\n",
        None,
        256,
    );
    parity(":s=!filter(&b,\"\")\n1:b=1/0==0", b"\n", None, 256);
    parity("=!fold(&b,!buffer(0,0),7)\n2=a/0", b"7\n", None, 256);
    parity(
        "=b(\"ab\")\n(s)=!map(&c,a)[0]+a[0]\n1=a+1",
        b"195\n",
        None,
        256,
    );
}
#[test]
fn std_callback_effect_order_and_unselected_arms_stay_lazy() {
    parity(
        "=!fold(&b,\"ABC\",0)\n2=a+!out(!bytes(1,b))",
        b"ABC3\n",
        None,
        256,
    );
    parity(
        ":s=false?!map(&b,\"x\"):\"safe\"\n1=!out(\"BAD\")+a/0",
        b"safe\n",
        None,
        256,
    );
    parity(
        ":s=!filter(&b,\"abc\")\n1:b=!out(!bytes(1,a))>=0",
        b"abcabc\n",
        None,
        256,
    );
}
#[test]
fn std_callback_allocation_quota_and_byte_range_keep_failure_order() {
    parity(":s=!map(&b,\"a\")\n1=#!bytes(3,a)", b"\x03\n", None, 256);
    parity(
        ":s=!map(&b,\"a\")\n1=#!bytes(67108865,a)",
        b"",
        Some("E013"),
        256,
    );
    parity(":s=!map(&b,\"a\")\n1=256", b"", Some("E014"), 256);
    parity(
        "=!fold(&b,\"a\",0)\n2=a+!parse(\"invalid\")",
        b"",
        Some("E016"),
        256,
    );
}
#[test]
fn std_inside_module_uses_the_same_loader_and_caller_callback() {
    let dir = std::env::temp_dir().join(format!("nil-std-module-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("module.nil"), "([i:i],s):s=!map(^a,b)").unwrap();
    std::fs::write(
        dir.join("module.nil-module"),
        "nil-module 1\nid 7\nsource module.nil\nexport 0 0\n",
    )
    .unwrap();
    let p = nil_compiler::compile_with_plugins(
        ":s=!7.0(&b,\"ab\")\n1=a+1",
        SourceProfile::ExprV5,
        &[dir.join("module.nil-module")],
    )
    .unwrap();
    check(&p.hir, b"bc\n", None, 256);
    std::fs::remove_dir_all(dir).unwrap();
}

// New update controls cover both instrumented and production lowering. Compare
// diagnostic codes and spans, and the host prefix before a trap.
fn update_parity(source: &str) {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let mut host = MemoryHost::default();
    let reference = execute_values_with_host(
        &p.hir,
        FunctionId(0),
        &[],
        Limits {
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
            ..Limits::default()
        },
        &mut host,
    );
    let (expected, diagnostic) = match reference {
        Ok(Value::Bytes(value)) => ([host.stdout, value.to_vec(), b"\n".to_vec()].concat(), None),
        Ok(Value::I64(value)) => (
            [host.stdout, format!("{value}\n").into_bytes()].concat(),
            None,
        ),
        Ok(other) => panic!("unexpected result {other:?}"),
        Err(error) => (host.stdout, Some(error.to_string())),
    };
    for optimization in [Optimization::O0, Optimization::O2] {
        for instrumentation in [Instrumentation::Bounded, Instrumentation::Unbounded] {
            let actual = nil_llvm::run_arguments(
                &p.hir,
                Options {
                    optimization,
                    instrumentation,
                    memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                    ..Default::default()
                },
                &[],
            )
            .unwrap();
            assert_eq!(
                actual.stdout, expected,
                "{source}: {optimization:?}/{instrumentation:?}"
            );
            if let Some(error) = &diagnostic {
                assert!(!actual.status.success());
                let native = String::from_utf8(actual.stderr).unwrap();
                assert_eq!(
                    native.split_whitespace().take(2).collect::<Vec<_>>(),
                    error.split_whitespace().take(2).collect::<Vec<_>>(),
                    "{source}: {native}"
                );
            } else {
                assert!(actual.status.success(), "{source}: {:?}", actual.stderr);
                assert!(actual.stderr.is_empty());
            }
        }
    }
}
#[test]
fn std_swap_checks_reads_before_writes_and_preserves_aliases() {
    for source in [
        ":s=!swap(\"abcd\",0,3)",
        ":s=!swap(\"abcd\",1,1)",
        ":s=old=\"abcd\";new=!swap(old,0,3);!concat(new,old)",
        "=!swap(!buffer(4,7)[3:9],0,3)[0]",
        ":s=!swap(\"abcd\",-1,9)",
        ":s=!swap(\"abcd\",-1,1)",
        ":s=false?!swap(\"\",0,0):\"ok\"",
        ":s=!swap(\"ab\",(ignored=!out(\"left\");0),(ignored=!out(\"right\");9))",
    ] {
        update_parity(source);
    }
}
#[test]
fn std_pop_and_last_use_slices_preserve_aliases_and_bounds() {
    for source in [
        ":s=!pop(\"a\")",
        ":s=!pop(\"\")",
        "=#!pop(!buffer(3,7))",
        ":s=old=\"abcd\";new=!pop(old);!concat(new,old)",
        ":s=!slice(\"abcdef\",2,3)",
        ":s=old=\"abcdef\";new=!slice(old,2,3);!concat(new,old)",
        ":s=!slice(\"a\",2,0)",
        ":s=@(\"abcd\",0;b<4;!pop(a),b+1;a)",
        ":s=true?\"ok\":!pop(\"\")",
    ] {
        update_parity(source);
    }
}

#[test]
fn last_use_slice_uses_unique_storage_but_later_read_keeps_copying() {
    let unique = compile_with_profile(":s=!slice(\"abcdef\",2,3)", SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&unique.hir);
    assert!(ir.contains("call ptr @nil_slice_unique("));
    assert!(!ir.contains("call ptr @nil_slice("));
    let alias = compile_with_profile(
        ":s=old=\"abcdef\";new=!slice(old,2,3);!concat(new,old)",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert!(nil_llvm::emit_llvm(&alias.hir).contains("call ptr @nil_slice("));
}
#[test]
fn std_updates_preserve_parent_children_and_zero_slot_records() {
    for source in [
        "record Z(empty:0)\nrecord R(text:s,zero:Z)\n:s=old=R(\"abcd\",Z([]));new=!pop(!swap(old.text,0,3));!concat(new,old.text)",
        ":s=old=!put(!bytemap(),\"x\",\"abcd\");new=!pop(!swap(!get(old,\"x\"),0,3));!concat(new,!get(old,\"x\"))",
        ":s=@(\"abcd\",0;b<4;!swap(a,0,3),b+1;a)",
    ] {
        update_parity(source);
    }
}

#[test]
fn checked_leaf_expansion_removes_unbounded_std_calls_but_keeps_bounded_calls() {
    for source in [
        ":s=!swap(\"abcd\",0,3)",
        ":s=!pop(\"abcd\")",
        ":s=b(\"abcd\",0,3)\n(s,i,i):s=right=a[c];left=a[b];a[b:right][c:left]",
        ":s=b(\"abcd\",0,3)\n(s,i,i):s=(a[c:a[b]])[b:a[c]]",
        ":s=b(\"abcd\",0,3)\n(s,i,i):s=a[b:a[c]+0][c:a[b]+0]",
        ":s=b(\"abcd\",9)\n(s,i):s=!slice(a,0,#a-1)",
    ] {
        let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let body = |ir: String| {
            ir.split("@nil_fn0(")
                .nth(1)
                .unwrap()
                .split("\n}")
                .next()
                .unwrap()
                .to_owned()
        };
        let unbounded = body(nil_llvm::emit_llvm_with_instrumentation(
            &program.hir,
            Instrumentation::Unbounded,
        ));
        assert!(
            !unbounded.contains("call ptr @nil_fn"),
            "{source}: {unbounded}"
        );
        let bounded = body(nil_llvm::emit_llvm_with_instrumentation(
            &program.hir,
            Instrumentation::Bounded,
        ));
        assert!(bounded.contains("call ptr @nil_fn"), "{source}: {bounded}");
        update_parity(source);
    }
}

#[test]
fn checked_leaf_expansion_keeps_argument_effects_once_and_in_order() {
    for source in [
        ":s=b((unused=!out(\"A\");\"abcd\"),(unused=!out(\"B\");0),(unused=!out(\"C\");3))\n(s,i,i):s=a[b:a[c]][c:a[b]]",
        ":s=b((unused=!out(\"A\");\"abcd\"),(unused=!out(\"B\");9))\n(s,i):s=!slice(a,0,#a-1)",
        ":s=false?b((unused=!out(\"BAD\");\"\"),0,1):\"ok\"\n(s,i,i):s=a[b:a[c]][c:a[b]]",
        ":s=@(\"abcd\",0;b<4;true?b(a,0,3):a,b+1;a)\n(s,i,i):s=a[b:a[c]][c:a[b]]",
        ":s=old=\"abcd\";new=b(old,0,3);!concat(new,old)\n(s,i,i):s=a[b:a[c]][c:a[b]]",
        "=old=!buffer(2,3);new=b(old,0,1);new[0]+old[1]\n(v,i,i):v=a[b:a[c]+1][c:a[b]]",
        ":s=b(\"abcd\",-1,9)\n(s,i,i):s=right=a[c];left=a[b];a[b:right][c:left]",
        ":s=b(\"abcd\",0,256)\n(s,i,i):s=a[b:c]",
        "record Z(empty:0)\nrecord R(text:s,zero:Z)\n:s=old=R(\"abcd\",Z([]));new=b(old.text,0,3);!concat(new,old.text)\n(s,i,i):s=a[b:a[c]][c:a[b]]",
    ] {
        update_parity(source);
    }
}
