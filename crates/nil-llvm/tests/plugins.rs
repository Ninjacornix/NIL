use nil_compiler::{
    SourceProfile,
    application::Host,
    compile_with_plugins, compile_with_profile,
    evaluator::{Limits, Value, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId};
use nil_llvm::{Optimization, Options};
use std::path::PathBuf;
fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/example/plugin.nil-plugin")
}
#[derive(Default)]
struct MemoryHost(Vec<u8>);
impl Host for MemoryHost {
    fn read(&mut self, _: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        panic!("unexpected host read")
    }
    fn write(&mut self, _: &[u8], _: &[u8]) -> Result<(), Diagnostic> {
        panic!("unexpected host write")
    }
    fn out(&mut self, v: &[u8]) -> Result<(), Diagnostic> {
        self.0.extend_from_slice(v);
        Ok(())
    }
}
fn parity(source: &str, expected: &[u8], code: Option<&str>, steps: u64, depth: usize) {
    let p = if source.contains("!plugin(1,") {
        compile_with_plugins(source, SourceProfile::ExprV5, &[manifest()])
    } else {
        compile_with_profile(source, SourceProfile::ExprV5)
    }
    .unwrap();
    let mut host = MemoryHost::default();
    let result = execute_values_with_host(
        &p.hir,
        FunctionId(0),
        &[],
        Limits {
            steps,
            call_depth: depth,
        },
        &mut host,
    );
    if let Some(code) = code {
        assert_eq!(result.unwrap_err().code, code);
        assert_eq!(host.0, expected);
    } else {
        let rendered = match result.unwrap() {
            Value::I64(n) => format!("{n}\n"),
            Value::Bool(b) => format!("{b}\n"),
            Value::Bytes(b) => String::from_utf8([b.as_ref(), b"\n"].concat()).unwrap(),
            _ => panic!("entry wrapper"),
        };
        assert_eq!([host.0, rendered.into_bytes()].concat(), expected);
    }
    for optimization in [Optimization::O0, Optimization::O2] {
        let r = nil_llvm::run_arguments(
            &p.hir,
            Options {
                instrumentation: if depth == 1 {
                    nil_llvm::Instrumentation::Bounded
                } else {
                    nil_llvm::Instrumentation::ProfileDefault
                },
                optimization,
                steps,
                call_depth: depth as u64,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert_eq!(r.stdout, expected, "{optimization:?} {source}");
        if let Some(code) = code {
            assert!(!r.status.success());
            assert!(String::from_utf8_lossy(&r.stderr).starts_with(code));
        } else {
            assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
            assert!(r.stderr.is_empty());
        }
    }
}
fn run(source: &str, expected: &[u8]) {
    parity(source, expected, None, 1_000_000, 256)
}
#[test]
fn migrated_equality_matches_empty_binary_length_and_integer_cases() {
    for (source, expected) in [
        (":b=!plugin(0,0,\"x\",\"x\")", b"true\n".as_slice()),
        (":b=!equal(\"\",\"\")", b"true\n".as_slice()),
        (":b=!equal(\"\\xff\\0\",\"\\xff\\0\")", b"true\n"),
        (":b=!equal(\"a\",\"aa\")", b"false\n"),
        (":b=!equal(!buffer(2,-9),!buffer(2,-8))", b"false\n"),
    ] {
        run(source, expected);
    }
}
#[test]
fn equality_internal_loop_preserves_caller_fuel_and_near_depth_limit() {
    parity(
        ":b=!equal(!bytes(10000,9),!bytes(10000,9))",
        b"true\n",
        None,
        8,
        1,
    );
    parity(
        ":b=!equal(!bytes(10000,9),!bytes(10000,9))",
        b"",
        Some("E008"),
        7,
        1,
    );
}
#[test]
fn plugin_record_result_retains_old_aliases_and_nested_dynamic_fields() {
    run(
        "record Packet(count:i,text:s)\n=b(Packet(7,\"old\"))\n(Packet)=c(a,!plugin(1,0,a))\n(Packet,Packet)=a.count+b.count+#!concat(a.text,b.text)",
        b"21\n",
    );
}
#[test]
fn plugin_records_pass_every_existing_type_and_preserve_numeric_bits() {
    run(
        "record Inner(value:i)\nrecord Packet(count:i,text:s,buffer:v,map:m,bytemap:t,wide:u128,word:u64,number:f64,flag:b,fixed:2,inner:Inner,typed:map[Inner])\n=b(!plugin(1,0,Packet(41,\"x\",!buffer(2,3),!put(!map(),\"k\",7),!put(!bytemap(),\"k\",\"z\"),340282366920938463463374607431768211455u128,18446744073709551615u64,-0.0,true,[5,6],Inner(9),!put(!map[Inner](),\"k\",Inner(11)))))\n(Packet)=a.count+!get(a.map,\"k\")+#!get(a.bytemap,\"k\")+a.buffer[1]+a.fixed[1]+a.inner.value+!get(a.typed,\"k\").value+(!bits(a.number)==9223372036854775808u64?1:0)+(!trunci64(a.wide)==-1?1:0)+(a.flag?1:0)+(a.word==18446744073709551615u64?1:0)",
        b"83\n",
    );
}
#[test]
fn plugin_record_loop_and_returned_alias_cross_function_calls() {
    run(
        "record Packet(count:i,text:s)\n=b(Packet(0,\"abc\"))\n(Packet)=@(a; a.count<10;!plugin(1,0,a);a.count+#a.text)",
        b"13\n",
    );
}
#[test]
fn lazy_plugin_calls_do_not_expose_argument_traps_or_host_effects() {
    run(
        "record Packet(count:i,text:s)\n=!out(\"A\")+(false?!plugin(1,0,Packet(1/0,!read(\"missing\"))).count:!plugin(1,0,Packet(!out(\"B\"),\"x\")).count)+!out(\"C\")",
        b"ABC4\n",
    );
}
#[test]
fn plugin_argument_traps_preserve_left_to_right_diagnostic_order() {
    parity(
        "record Packet(count:i,text:s)\n=!plugin(1,0,Packet(!parse(\"bad\"),!bytes(-1,256))).count",
        b"",
        Some("E016"),
        1000000,
        256,
    );
    parity(
        ":b=!equal(!bytes(-1,256),!bytes(67108864,0))",
        b"",
        Some("E013"),
        1000000,
        256,
    );
}
#[test]
fn plugin_returned_alias_keeps_capacity_live_at_quota_boundary() {
    parity(
        "record Packet(count:i,text:s)\n=b(Packet(0,!bytes(40000000,0)))\n(Packet)=c(!plugin(1,0,a),!bytes(30000000,0))\n(Packet,s)=#a.text+#b",
        b"",
        Some("E013"),
        1000000,
        256,
    );
}
#[test]
fn dead_plugin_result_releases_capacity_before_next_allocation() {
    run(
        "record Packet(count:i,text:s)\n=b(Packet(0,!bytes(40000000,0)))\n(Packet)=#!plugin(1,0,a).text+#!bytes(30000000,0)",
        b"70000000\n",
    );
}
#[test]
fn plugin_borrowing_and_unique_reuse_are_visible_before_llvm_inlining() {
    let p = compile_with_profile(
        "(s,s)=@(a,b,0,0;c<4;a,b,c+1,d+(!equal(a,b)?1:0);d)",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert!(nil_hir::borrowing::Summaries::analyze(&p.hir).function(0));
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("call i1 @nil_plugin0_fn0("));
    assert!(!ir.contains("call ptr @nil_roots_enter("));
    assert!(!ir.contains("@nil_equal"));
    let p = compile_with_profile(
        "(s,s):s=b(a,!equal(a,a))\n(s,b):s=!concat(a,\"x\")",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert!(nil_llvm::emit_llvm(&p.hir).contains("call ptr @nil_concat_unique("));
}

#[test]
fn provider_internal_checked_traps_and_lazy_arms_preserve_order_at_o0_o2() {
    let dir = std::env::temp_dir().join(format!("nil-plugin-trap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest"),
        "nil-plugin 1\nid 7\nsource provider.nil\neffect borrow\nexport 0 0\n",
    )
    .unwrap();
    for (body, source, expected, code) in [
        (
            "1=a/a",
            "=!out(\"A\")+!plugin(7,0,!out(\"B\")-1)+!out(\"C\")",
            b"AB".as_slice(),
            Some("E009"),
        ),
        (
            "1=a==0?42:1/0",
            "=!out(\"A\")+!plugin(7,0,0)+!out(\"C\")",
            b"AC44\n",
            None,
        ),
    ] {
        std::fs::write(dir.join("provider.nil"), body).unwrap();
        let p =
            compile_with_plugins(source, SourceProfile::ExprV5, &[dir.join("manifest")]).unwrap();
        let mut host = MemoryHost::default();
        let result =
            execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), &mut host);
        if let Some(code) = code {
            assert_eq!(result.unwrap_err().code, code);
            assert_eq!(host.0, expected);
        } else {
            assert_eq!(
                [
                    host.0,
                    format!(
                        "{}\n",
                        match result.unwrap() {
                            Value::I64(v) => v,
                            _ => panic!("scalar wrapper"),
                        }
                    )
                    .into_bytes()
                ]
                .concat(),
                expected
            );
        }
        for optimization in [Optimization::O0, Optimization::O2] {
            let r = nil_llvm::run_arguments(
                &p.hir,
                Options {
                    optimization,
                    ..Default::default()
                },
                &[],
            )
            .unwrap();
            assert_eq!(r.stdout, expected);
            if let Some(code) = code {
                assert!(!r.status.success());
                assert!(String::from_utf8_lossy(&r.stderr).starts_with(code));
            } else {
                assert!(r.status.success());
                assert!(r.stderr.is_empty());
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
