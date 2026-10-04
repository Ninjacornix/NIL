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
fn record_buffer_construction_index_and_field_types() {
    run(
        "record Node(value:i,next:i)
=!buffer[Node](3,Node(42,-1))[2].value",
        b"42\n",
    );
    run(
        "record Node(value:i,next:i)
=#!buffer[Node](0,Node(42,-1))",
        b"0\n",
    );
}
#[test]
fn record_buffer_unique_update_preserves_old_alias() {
    run(
        "record Node(value:i,next:i)
=b(!buffer[Node](1,Node(2,-1)))
(v[Node])=c(a,a[0:Node(7,-1)])
(v[Node],v[Node])=a[0].value*10+b[0].value",
        b"27\n",
    );
}
#[test]
fn nested_rows_hold_bytes_across_allocations() {
    run(
        "record Cell(text:s)
record Row(cells:v[Cell])
:s=b(!buffer[Row](2,Row(!buffer[Cell](2,Cell(\"kept\")))))
(v[Row]):s=c(a,!bytes(1024,0))
(v[Row],s):s=a[0].cells[0].text",
        b"kept\n",
    );
}
#[test]
fn repeated_child_aliases_block_child_reuse() {
    run(
        "record Row(data:v)
=b(!buffer[Row](2,Row(!buffer(1,2))))
(v[Row])=c(a,a[0].data[0:9])
(v[Row],v)=a[1].data[0]*10+b[0]",
        b"29\n",
    );
}
#[test]
fn concat_and_slice_keep_old_nested_aliases() {
    run(
        "record Cell(text:s)
=b(!buffer[Cell](1,Cell(\"A\")))
(v[Cell])=c(a,!concat(a,a))
(v[Cell],v[Cell])=a[0].text[0]+!slice(b,1,1)[0].text[0]+#b",
        b"132\n",
    );
}
#[test]
fn each_record_snapshot_survives_functional_update() {
    run(
        "record Cell(value:i)
record State(rows:v[Cell],total:i)
=b(!buffer[Cell](3,Cell(2)))
(v[Cell])=!each(a,State(a,0);c{rows:c.rows[0:Cell(9)][1:Cell(9)][2:Cell(9)]}{total:c.total+b.value};a).total",
        b"6\n",
    );
}
#[test]
fn growing_record_loop_returns_nested_values() {
    run(
        "record Cell(value:i)
=b(!buffer[Cell](0,Cell(0)),0)[9].value
(v[Cell],i):v[Cell]=@(a,0;b<10;!concat(a,!buffer[Cell](1,Cell(b))),b+1;a)",
        b"9\n",
    );
}
#[test]
fn record_buffer_range_traps_precede_quota() {
    for source in [
        "record Cell(value:i)
=!buffer[Cell](0,Cell(0))[0].value",
        "record Cell(value:i)
=#!slice(!buffer[Cell](1,Cell(0)),2,67108864)",
    ] {
        parity(source, b"", Some("E012"), 1000000, 256);
    }
    parity(
        "record Cell(value:i)
=#!buffer[Cell](-1,Cell(0))",
        b"",
        Some("E013"),
        1000000,
        256,
    );
    parity(
        "record Cell(value:i)
=#!buffer[Cell](8388608,Cell(0))",
        b"",
        Some("E013"),
        1000000,
        256,
    );
}
#[test]
fn record_buffer_sort_has_explicit_unsupported_order() {
    for (mode, code) in [(0, "E018"), (1, "E018"), (2, "E012"), (-1, "E012")] {
        parity(
            &format!("record Cell(value:i)\n=#!sort(!buffer[Cell](0,Cell(0)),{mode})"),
            b"",
            Some(code),
            1000000,
            256,
        );
    }
}
#[test]
fn lazy_record_allocations_keep_traps_and_effects_selected() {
    run(
        "record Cell(text:s)
=1==1?!buffer[Cell](1,Cell(\"A\"))[0].text[0]:!buffer[Cell](-1,Cell(\"trap\"))[0].text[0]",
        b"65\n",
    );
}
#[test]
fn record_buffer_quota_counts_transitive_children() {
    parity(
        "record Cell(text:s)
=b(!buffer[Cell](1,Cell(!bytes(33554432,1))))
(v[Cell])=c(a,!bytes(41943040,0))
(v[Cell],s)=a[0].text[0]+#b",
        b"",
        Some("E013"),
        1000000,
        256,
    );
}
#[test]
fn dead_outer_buffer_releases_transitive_quota() {
    run(
        "record Cell(text:s)
=b(!buffer[Cell](1,Cell(!bytes(33554432,1))))
(v[Cell])=c(#a,!bytes(41943040,0))
(i,s)=a+#b",
        b"41943041\n",
    );
}

#[test]
fn scalar_record_update_loop_uses_unique_path_before_optimization() {
    let source = "record Cell(value:i)\n=b(!buffer[Cell](8,Cell(0)))\n(v[Cell])=@(a,0;b<8;a[b:Cell(b)],b+1;!each(a,0;c+b.value;a))";
    run(source, b"28\n");
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("@nil_record_set"));
    assert!(
        ir.lines()
            .any(|line| line.contains("call ptr @nil_record_set") && line.contains("i1 true"))
    );
    let aliased = "record Cell(value:i)\n=b(!buffer[Cell](8,Cell(0)))\n(v[Cell])=c(a,a[0:Cell(7)])\n(v[Cell],v[Cell])=a[0].value+b[0].value";
    let p = compile_with_profile(aliased, SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(
        ir.lines()
            .any(|line| line.contains("call ptr @nil_record_set") && line.contains("i1 false"))
    );
}
#[test]
fn nested_record_update_loop_retains_replaced_children_and_aliases() {
    run(
        "record Cell(text:s)\n:s=b(!buffer[Cell](2,Cell(\"old\")))\n(v[Cell]):s=c(a,@(a,0;b<2;a[b:Cell(!concat(a[b].text,\"x\"))],b+1;a))\n(v[Cell],v[Cell]):s=!concat(a[0].text,b[1].text)",
        b"oldoldx\n",
    );
}
#[test]
fn records_cross_buffer_boundary_with_all_scalar_bits() {
    run(
        "record Cell(small:u64,wide:u128,float:f64,flag:b,arr:2)\n= b(!buffer[Cell](1,Cell(18446744073709551615u64,340282366920938463463374607431768211455u128,!floatbits(9223372036854775808u64),true,[7,9])))\n(v[Cell])=(!bits(a[0].float)==9223372036854775808u64?1:0)+(a[0].flag?1:0)+(a[0].small==18446744073709551615u64?1:0)+(a[0].wide==340282366920938463463374607431768211455u128?1:0)+a[0].arr[1]",
        b"13\n",
    );
}

#[test]
fn zero_slot_record_rows_are_padded_and_preserve_empty_fields() {
    run("record Zero(empty:0)\n=#!buffer[Zero](3,Zero([]))", b"3\n");
    run(
        "record Zero(empty:0)\n=#!buffer[Zero](3,Zero([]))[2].empty",
        b"0\n",
    );
    run(
        "record Zero(empty:0)\n=#!slice(!concat(!buffer[Zero](3,Zero([]))[1:Zero([])],!buffer[Zero](2,Zero([]))),1,3)",
        b"3\n",
    );
    run(
        "record Zero(empty:0)\nrecord Wrapped(zero:Zero,value:i)\n=!buffer[Wrapped](2,Wrapped(Zero([]),42))[1].value",
        b"42\n",
    );
}
#[test]
fn zero_slot_record_rows_keep_bounds_and_quota_checks() {
    parity(
        "record Zero(empty:0)\n=#!buffer[Zero](0,Zero([]))[0].empty",
        b"",
        Some("E012"),
        1_000_000,
        256,
    );
    parity(
        "record Zero(empty:0)\n=#!buffer[Zero](8388608,Zero([]))",
        b"",
        Some("E013"),
        1_000_000,
        256,
    );
}

#[test]
fn degenerate_nested_map_layout_and_loop_state_preserve_parity() {
    run(
        "record Z(x:0)\nrecord ZZ(a:Z,b:0)\n=b(!map[ZZ]())\n(map[ZZ])=@(a,0;b<4;!put(a,!format(b),ZZ(Z([]),[])),b+1;!size(a)+#!get(a,\"3\").a.x)",
        b"4\n",
    );
    run(
        "record Z(x:0,y:0)\nrecord ZZ(a:Z,b:0)\n=b(!buffer[ZZ](0,ZZ(Z([],[]),[])))\n(v[ZZ])=@(a,0;b<4;!concat(a,!buffer[ZZ](1,ZZ(Z([],[]),[]))),b+1;#a+#a[3].a.y)",
        b"4\n",
    );
}
