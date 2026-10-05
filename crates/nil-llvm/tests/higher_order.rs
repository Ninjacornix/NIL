use nil_compiler::{
    SourceProfile,
    application::Host,
    compile_with_profile,
    evaluator::{Limits, Value, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId, Phase};
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
fn hof_evaluator_and_native_map_empty_and_nonempty_inputs_agree() {
    for (source, expected) in [
        (
            "=b(&c,0)\n([i:i],i)=@(0,0;a<b;a+1,b+^a(a);b)\n1=a*a",
            b"0\n".as_slice(),
        ),
        (
            "=b(&c,8)\n([i:i],i)=@(0,0;a<8;a+1,b+^a(a);b)\n1=a*a",
            b"140\n",
        ),
        (
            "=b(&c,[1,2,3])\n([i:i],3):3=[^a(b[0]),^a(b[1]),^a(b[2])]\n1=a*a",
            b"[1,4,9]\n",
        ),
    ] {
        // Array result is consumed through a scalar entry for the common checker.
        if source.contains(":3=") {
            parity(
                "=b(&c,[1,2,3])[2]\n([i:i],3):3=[^a(b[0]),^a(b[1]),^a(b[2])]\n1=a*a",
                b"9\n",
                None,
                256,
            );
        } else {
            parity(source, expected, None, 256);
        }
    }
}
#[test]
fn allocating_hof_callback_preserves_aliases_and_record_layouts() {
    parity(
        "=b(&c,\"old\")\n([s:s],s)=#^a(b)+b[0]\n(s):s=!concat(a,\"new\")",
        b"117\n",
        None,
        256,
    );
    parity(
        "record R(data:s,value:i)\n=b(&c,R(\"x\",7))\n([R:R],R)=^a(b).value+b.value\n(R):R=a{value:a.value+1}",
        b"15\n",
        None,
        256,
    );
}
#[test]
fn zero_slot_record_callbacks_and_record_buffers_are_sound_at_o0_and_o2() {
    for source in [
        "record Z(x:0)\n=b(&c,Z([]))\n([Z:Z],Z)=#!buffer[Z](2,^a(b))\n(Z):Z=a",
        "record Z(x:0)\nrecord N(z:Z,y:0)\n=b(&c,N(Z([]),[]))\n([N:N],N)=#!buffer[N](2,^a(b))\n(N):N=a",
    ] {
        parity(source, b"2\n", None, 256);
    }
}
#[test]
fn effectful_callback_in_unselected_lazy_arm_never_executes() {
    parity(
        "=false?b(&c):42\n([:i])=^a()\n=!out(\"BAD\")",
        b"42\n",
        None,
        256,
    );
    parity("=true?42:b(&c)\n([:i])=^a()\n=1/0", b"42\n", None, 256);
}
#[test]
fn callback_effect_order_stays_left_to_right_across_lazy_arms() {
    parity(
        "=b(&c,!out(\"A\"),true?d():!out(\"BAD\"))+!out(\"D\")\n([i:i],i,i)=^a(b+c)\n1=!out(\"C\")+a\n=!out(\"B\")",
        b"ABCD4\n",
        None,
        256,
    );
}
#[test]
fn callback_allocation_quota_and_trap_order_are_unchanged() {
    parity(
        "=b(&c)\n([:s])=#^a()\n:s=!bytes(67108865,0)",
        b"",
        Some("E013"),
        256,
    );
    parity(
        "=b(&c,!out(\"A\"))\n([i:i],i)=^a(b)\n1=!bytes(-1,256)[0]",
        b"A",
        Some("E013"),
        256,
    );
}
#[test]
fn recursion_through_callback_and_mutual_hof_recursion_preserve_depth_checks() {
    for n in [10, 248, 300] {
        let source = format!("=b(&c,{n})\n([i:i],i)=b==0?0:^a(b)+b(^a,b-1)\n1=a");
        if n == 300 {
            parity(&source, b"", Some("E008"), 256);
        } else {
            parity(
                &source,
                format!("{}\n", n * (n + 1) / 2).as_bytes(),
                None,
                256,
            );
        }
    }
    parity(
        "=b(&d,10)\n([i:i],i)=b==0?0:^a(b)+c(^a,b-1)\n([i:i],i)=b==0?0:^a(b)+b(^a,b-1)\n1=a",
        b"55\n",
        None,
        256,
    );
    parity(
        "=b(&c,10)\n([i:i],i)=^a(b)\n1=a==0?0:c(a-1)+1",
        b"10\n",
        None,
        256,
    );
}
#[test]
fn source_module_exported_hof_accepts_a_caller_local_callback() {
    let folder = std::env::temp_dir().join(format!("nil-hof-module-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let manifest = folder.join("module.manifest");
    std::fs::write(
        &manifest,
        "nil-module 1\nid 13\nsource module.nil\nexport 0 0\nexport 1 1\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("module.nil"),
        "([s:s],s):s=^a(b)\n(s):s=!concat(a,\"!\")",
    )
    .unwrap();
    let p = nil_compiler::compile_with_plugins(
        ":s=!plugin(13,0,&b,\"x\")\n(s):s=!concat(a,\"y\")",
        SourceProfile::ExprV5,
        &[&manifest],
    )
    .unwrap();
    check(&p.hir, b"xy\n", None, 256);
    let p = nil_compiler::compile_with_plugins(
        ":s=b(&!plugin(13,1),\"x\")\n([s:s],s):s=^a(b)",
        SourceProfile::ExprV5,
        &[&manifest],
    )
    .unwrap();
    check(&p.hir, b"x!\n", None, 256);
    std::fs::write(
        &manifest,
        "nil-plugin 1\nid 13\nsource module.nil\neffect borrow\nexport 0 0\n",
    )
    .unwrap();
    let d = nil_compiler::compile_with_plugins(
        "=!plugin(13,0,&b,1)\n1=a",
        SourceProfile::ExprV5,
        &[&manifest],
    )
    .unwrap_err();
    assert_eq!(d.code, "E024");
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn explicit_state_arguments_replace_implicit_closure_capture() {
    parity(
        " :s=b(&c,\"prefix\",\"x\")\n([s,s:s],s,s):s=^a(b,c)\n(s,s):s=!concat(a,b)",
        b"prefixx\n",
        None,
        256,
    );
}
#[test]
fn reference_execution_still_denies_host_effects_in_callbacks() {
    let p =
        compile_with_profile("=b(&c)\n([:i])=^a()\n=!out(\"x\")", SourceProfile::ExprV5).unwrap();
    let d = nil_compiler::evaluator::execute_values(&p.hir, FunctionId(0), &[], Limits::default())
        .unwrap_err();
    assert_eq!(d.code, "E018");
    assert_eq!(d.phase, Phase::Execute);
}
