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
    output: Vec<u8>,
}
impl Host for MemoryHost {
    fn out(&mut self, value: &[u8]) -> Result<(), Diagnostic> {
        self.output.extend_from_slice(value);
        Ok(())
    }
}
fn parity(source: &str) {
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let mut host = MemoryHost::default();
    let result = execute_values_with_host(
        &program.hir,
        FunctionId(0),
        &[],
        Limits {
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
            ..Limits::default()
        },
        &mut host,
    );
    let mut expected = host.output;
    let diagnostic = match result {
        Ok(Value::I64(value)) => {
            expected.extend(format!("{value}\n").bytes());
            None
        }
        Ok(Value::Bytes(value)) => {
            expected.extend(value.as_ref());
            expected.push(b'\n');
            None
        }
        Ok(Value::Buffer(value)) => {
            expected.extend(
                format!(
                    "[{}]\n",
                    value
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                )
                .bytes(),
            );
            None
        }
        Ok(other) => panic!("unexpected result {other:?}"),
        Err(error) => Some(error),
    };
    for optimization in [Optimization::O0, Optimization::O2] {
        for instrumentation in [Instrumentation::Bounded, Instrumentation::Unbounded] {
            let actual = nil_llvm::run_arguments(
                &program.hir,
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
                "{optimization:?} {instrumentation:?} {source}"
            );
            if let Some(error) = &diagnostic {
                let span = error.span.unwrap();
                assert!(!actual.status.success());
                let prefix = if error.code == "E013" {
                    "E013 ".to_owned()
                } else {
                    format!("{} @{}..{} ", error.code, span.start, span.end)
                };
                assert!(
                    actual.stderr.starts_with(prefix.as_bytes()),
                    "{optimization:?} {instrumentation:?}: {:?}",
                    actual.stderr
                );
            } else {
                assert!(
                    actual.status.success(),
                    "{optimization:?} {instrumentation:?}: {:?}",
                    actual.stderr
                );
                assert!(actual.stderr.is_empty());
            }
        }
    }
}

#[test]
fn simultaneous_chains_match_reference_for_all_operand_and_write_traps() {
    for source in [
        "=#!bytes(1,0)[3:!out(\"A\")][0:!out(\"B\")]",
        "=#(!bytes(1,0)[3:!out(\"A\")])[0:!parse(\"bad\")]",
        "=#!bytes(1,0)[0:256][9:1/0]",
        "=#!bytes(1,0)[9:256][0:1]",
        "=#!bytes(1,0)[1/0:1][0:!out(\"BAD\")]",
        "=#!bytes(1,0)[0:!out(\"A\")][1/0:!out(\"BAD\")]",
        "=x=!bytes(1,0)[3:!out(\"A\")];#x[0:!out(\"BAD\")]",
        "=#(x=!bytes(1,0)[3:!out(\"A\")];x)[0:!out(\"BAD\")]",
    ] {
        parity(source);
    }
}
#[test]
fn grouped_and_nested_chains_keep_original_aliases_and_selected_effects() {
    for source in [
        "=a=!buffer(2,3);b=(a[0:a[1]+1])[1:a[0]];b[0]*10+b[1]+a[0]",
        "=a=!bytes(2,3);b=((a[0:a[1]+1]))[1:a[0]];b[0]*10+b[1]+a[0]",
        "=a=[2,3];b=a[true?0:1:a[1]][1:a[0]];b[0]*10+b[1]+a[0]",
        "=false?(#!bytes(1,0)[9:!out(\"BAD\")][0:1/0]):42",
        "=a=[1,2];b=a[0:7][0:8];b[0]+a[0]",
        "=a=!buffer(2,1);b=a[0:(a[1:9])[0]][1:a[0]];b[0]+b[1]+a[1]",
        "=a=!bytes(2,3);b=a[0:b(a,1)][1:b(a,0)];b[0]+b[1]+a[0]\n(s,i)=a[b]",
    ] {
        parity(source);
    }
}
#[test]
fn record_chains_and_zero_slot_children_keep_layout_and_aliases() {
    for source in [
        "record R(x:v,y:i)\n=orig=R(!buffer(2,1),2);new=orig{x:orig.x[0:7][1:orig.x[0]]}{y:orig.x[0]};new.x[1]+new.y+orig.x[0]",
        "record Z(x:0)\nrecord R(z:Z,y:i)\n=r=R(Z([]),1);s=r{z:r.z{x:[]}}{y:r.y+1};s.y",
        "record R(x:i,y:i)\n=r=R(1,2);s=(r{x:!out(\"A\")}){y:!out(\"B\")};s.x+s.y+r.x",
        "record R(x:s,y:i)\n=r=R(\"x\",1);s=r{x:(r.x[9:!out(\"A\")])}{y:!out(\"BAD\")};s.y",
    ] {
        parity(source);
    }
}
#[test]
fn chain_loop_and_lazy_variants_preserve_single_use_lowering() {
    for source in [
        "=a=@(!buffer(2,1),0;b<16;a[0:a[1]+1][1:a[0]],b+1;a);a[0]+a[1]",
        "=a=@(!bytes(2,1),0;b<16;(a[0:a[1]+1])[1:a[0]],b+1;a);a[0]+a[1]",
        "=a=@(!buffer(2,1),0;b<16;true?a[0:a[1]+1][1:a[0]]:a,b+1;a);a[0]+a[1]",
    ] {
        parity(source);
    }
    let p = compile_with_profile(
        "=a=@(!buffer(2,1),0;b<16;a[0:a[1]+1][1:a[0]],b+1;a);a[0]+a[1]",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("call ptr @nil_set_unique"));
    assert!(!ir.contains("call ptr @nil_set_replay"));
}
