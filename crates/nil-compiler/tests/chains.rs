use nil_compiler::{
    SourceProfile,
    application::Host,
    compile_with_profile,
    evaluator::{Limits, Value, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId, Operation};
#[derive(Default)]
struct MemoryHost(Vec<u8>);
impl Host for MemoryHost {
    fn out(&mut self, bytes: &[u8]) -> Result<(), Diagnostic> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}
fn run(source: &str, profile: SourceProfile) -> (Result<Value, Diagnostic>, Vec<u8>) {
    let p = compile_with_profile(source, profile).unwrap();
    let mut host = MemoryHost::default();
    let result = execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), &mut host);
    (result, host.0)
}
#[test]
fn chained_operands_lower_before_writes_without_an_index_shape_proof() {
    let p = compile_with_profile(
        "(v):v=a[b(a,1):a[1]][0:b(a,0)]\n(v,i)=a[b]",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let instructions = &p.hir.program().functions[0].instructions;
    let first_write = instructions
        .iter()
        .position(|i| matches!(i.operation, Operation::Replace { .. }))
        .unwrap();
    assert!(
        instructions[..first_write]
            .iter()
            .any(|i| matches!(i.operation, Operation::Call { .. }))
    );
    assert!(
        instructions[first_write..]
            .iter()
            .all(|i| matches!(i.operation, Operation::Replace { .. }))
    );
}
#[test]
fn chain_operands_and_effects_precede_replacement_validation() {
    for source in [
        "=#!bytes(1,0)[3:!out(\"A\")][0:!out(\"B\")]",
        "=#(!bytes(1,0)[3:!out(\"A\")])[0:!out(\"B\")]",
        "=#((!bytes(1,0)[3:!out(\"A\")]))[0:!out(\"B\")]",
    ] {
        let (r, out) = run(source, SourceProfile::ExprV5);
        assert_eq!(out, b"AB", "{source}");
        assert_eq!(r.unwrap_err().code, "E012");
    }
    let (r, out) = run(
        "=#!bytes(1,0)[3:!out(\"A\")][0:!parse(\"bad\")]",
        SourceProfile::ExprV5,
    );
    assert_eq!(out, b"A");
    assert_eq!(r.unwrap_err().code, "E016");
}
#[test]
fn explicit_bindings_and_read_suffixes_are_chain_boundaries() {
    for source in [
        "=x=!bytes(1,0)[3:!out(\"A\")];#x[0:!out(\"B\")]",
        "=#(x=!bytes(1,0)[3:!out(\"A\")];x)[0:!out(\"B\")]",
        "=(!bytes(1,0)[3:!out(\"A\")][0])+!out(\"B\")",
    ] {
        let (r, out) = run(source, SourceProfile::ExprV5);
        assert_eq!(out, b"A", "{source}");
        assert_eq!(r.unwrap_err().code, "E012");
    }
}
#[test]
fn chained_updates_preserve_aliases_duplicates_record_children_and_zero_slots() {
    for (source, expected) in [
        ("=a=[2,3];b=a[0:a[1]][1:a[0]];b[0]*10+b[1]+a[0]", 34),
        (
            "=a=!buffer(2,3);b=a[0:a[1]+1][1:a[0]];b[0]*10+b[1]+a[0]",
            46,
        ),
        (
            "=a=!bytes(2,3);b=(a[0:a[1]+1])[1:a[0]];b[0]*10+b[1]+a[0]",
            46,
        ),
        ("=a=[1,2];b=a[0:7][0:8];b[0]+a[0]", 9),
        (
            "record R(x:v,y:i)\n=orig=R(!buffer(2,1),2);new=orig{x:orig.x[0:7][1:orig.x[0]]}{y:orig.x[0]};new.x[1]+new.y+orig.x[0]",
            3,
        ),
        (
            "record Z(x:0)\nrecord R(z:Z,y:i)\n=r=R(Z([]),1);s=r{z:r.z{x:[]}}{y:r.y+1};s.y",
            2,
        ),
        ("=a=[2,3];b=a[true?0:1:a[1]][1:a[0]];b[0]*10+b[1]", 32),
    ] {
        assert_eq!(
            run(source, SourceProfile::ExprV5).0.unwrap(),
            Value::I64(expected),
            "{source}"
        );
    }
}
#[test]
fn chain_operands_in_unselected_lazy_regions_do_nothing() {
    let (r, out) = run(
        "=false?(#!bytes(1,0)[9:!out(\"BAD\")][0:1/0]):42",
        SourceProfile::ExprV5,
    );
    assert_eq!(r.unwrap(), Value::I64(42));
    assert!(out.is_empty());
}
#[test]
fn earlier_profiles_keep_eager_update_trap_order() {
    let source = "=([1,2][9:1])[0:1/0][0]";
    assert_eq!(
        run(source, SourceProfile::ExprV4).0.unwrap_err().code,
        "E012"
    );
    assert_eq!(
        run(source, SourceProfile::ExprV5).0.unwrap_err().code,
        "E009"
    );
}
#[test]
fn malformed_chains_have_bounded_source_spans() {
    for source in [
        "=[1][0:][0:2]",
        "=[1][0:1][",
        "record R(x:i)\n=R(1){x:}",
        "=[1][true?0:0:]",
    ] {
        let d = compile_with_profile(source, SourceProfile::ExprV5).unwrap_err();
        assert_eq!(d.code, "E001", "{source}");
        let span = d.span.unwrap();
        assert!(span.start <= span.end && span.end <= source.len());
    }
}
