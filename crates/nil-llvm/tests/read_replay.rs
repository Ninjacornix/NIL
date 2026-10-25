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

fn resource_witness(source: &str) {
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let result = execute_values_with_host(
        &program.hir,
        FunctionId(0),
        &[],
        Limits {
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
            ..Limits::default()
        },
        &mut MemoryHost::default(),
    );
    if let Err(error) = result {
        assert_eq!(error.code, "E013");
    }
    // Allocation failure location is not a semantic contract.
    parity(source);
}

#[test]
fn different_indices_multiple_reads_bindings_and_reordered_operands_are_replayed() {
    for source in [
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[2]]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[0]+a[1]-97]",
        ":s=b(\"abcd\")\n(s):s=idx=2;a[0:90][1:a[idx]]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:1+a[2]-1]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:(true?(false?a[-1]:a[2]):a[-2])]",
    ] {
        parity(source);
    }
    let program = compile_with_profile(
        ":s=b(\"abcd\")\n(s):s=changed=a[0:90];changed[1:a[2]]",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let ir = nil_llvm::emit_llvm(&program.hir);
    assert!(ir.contains("call void @nil_set_replay_check"));
    assert!(ir.contains("call ptr @nil_set_replay"));
    assert!(ir.contains("icmp ult i64"));
}

#[test]
fn readonly_callees_and_recursive_summaries_preserve_original_reads() {
    for source in [
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,2)]\n(s,i)=a[b]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,2)]\n(s,i)=b==-1?0:a[b]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,2)]\n(s,i)=b==0?a[0]:c(a,b-1)",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,2)]\n(s,i)=b==0?a[0]:d(a,b-1)\n(s,i)=b==0?a[0]:c(a,b-1)",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,20)]\n(s,i)=b==0?a[0]:c(a,b-1)",
    ] {
        parity(source);
    }
}

#[test]
fn chain_operand_traps_precede_write_validation_with_simultaneous_updates() {
    for source in [
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[-1]]",
        ":s=b(\"abcd\")\n(s):s=a[-1:90][1:a[9]]",
        ":s=b(\"abcd\")\n(s):s=a[0:256][1:a[-1]]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,9)]\n(s,i)=a[b]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:1/0+a[0]]",
        ":s=b(!bytes(33554400,97))\n(s):s=a[0:90][1:a[-1]]",
    ] {
        parity(source);
    }
}

#[test]
fn effects_and_allocations_remain_conservative_in_b1_replay() {
    for source in [
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a)]\n(s)=ignored=!out(\"middle\");a[0]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:(true?(ignored=!out(\"middle\");a[0]):a[1])]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:(false?(ignored=!out(\"BAD\");a[-1]):a[1])]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:(ignored=#!bytes(67108824,0);a[0])]",
    ] {
        parity(source);
    }
    resource_witness(
        "record Holder(text:s,n:i)\n:s=b(Holder(!bytes(33554392,97),0))\n(Holder):s=changed=a.text[0:90];temp=#!bytes(100,0);n=a.n;changed",
    );
}

#[test]
fn multi_update_snapshot_keeps_values_under_ordinary_memory_semantics() {
    // Multiple updates no longer impose a quota-only replay barrier.
    let source = ":s=b(!bytes(22369590,97))\n(s):s=changed=a[0:90];changed[1:a[2]][2:a[0]]";
    resource_witness(source);
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    assert!(nil_llvm::emit_llvm(&program.hir).contains("guarded replay retires only"));
}

#[test]
fn live_returned_and_record_child_aliases_still_preserve_immutable_values() {
    for source in [
        ":s=old=\"abcd\";new=b(old);!concat(new,old)\n(s):s=a[0:90][1:a[2]]",
        ":s=old=c(\"abcd\");new=b(old);!concat(new,old)\n(s):s=a[0:90][1:a[2]]\n(s):s=a",
        "record R(data:s)\n:s=old=R(\"abcd\");new=b(old.data);!concat(new,old.data)\n(s):s=a[0:90][1:a[2]]",
        "record Z(empty:0)\n:s=b(\"abcd\",Z([]))\n(s,Z):s=a[0:90][1:c(a,b)]\n(s,Z)=a[2]+#b.empty",
    ] {
        parity(source);
    }
}

#[test]
fn loop_state_and_nested_lazy_regions_preserve_snapshot_semantics() {
    for source in [
        ":s=@(\"abcd\",0;b<3;b==1?a[0:90][1:b(a)]:a,b+1;a)\n(s)=a[2]",
        ":s=@(\"abcd\",0;b<3;a[0:90][1:(a[0]==97?a[2]:a[3])],b+1;a)",
        ":s=b(\"\")\n(s):s=a[0:90][1:a[0]]",
    ] {
        parity(source);
    }
}

#[test]
fn copy_hints_report_actual_copy_and_direct_keeping_read_without_changing_semantics() {
    let sources = [
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[2]][2:a[0]]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[2]]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:(ignored=!out(\"middle\");a[0])]",
        ":s=b(\"abcd\")\n(s):s=changed=a[0:90];changed[1:a[9]]",
    ];
    for (index, source) in sources.iter().enumerate() {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        for optimization in [Optimization::O0, Optimization::O2] {
            let options = Options {
                optimization,
                instrumentation: Instrumentation::Unbounded,
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                ..Default::default()
            };
            let plain = nil_llvm::run_arguments(&p.hir, options, &[]).unwrap();
            let hints = nil_llvm::run_arguments(
                &p.hir,
                Options {
                    memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                    explain_copies: true,
                    ..options
                },
                &[],
            )
            .unwrap();
            let again = nil_llvm::run_arguments(
                &p.hir,
                Options {
                    memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                    explain_copies: true,
                    ..options
                },
                &[],
            )
            .unwrap();
            assert_eq!(plain.status.code(), hints.status.code());
            assert_eq!(plain.stdout, hints.stdout);
            assert_eq!(hints.stderr, again.stderr);
            let rendered = String::from_utf8(hints.stderr).unwrap();
            if index == 0 {
                assert!(rendered.is_empty(), "simultaneous chain copied: {rendered}");
            } else if index == 3 {
                assert!(rendered.contains("\"kind\":\"replace\""), "{rendered}");
                assert!(
                    rendered.contains("\"precision\":\"direct-local-read\""),
                    "{rendered}"
                );
                let read = source.find("a[9]").unwrap() + 1;
                assert!(
                    rendered.contains(&format!("\"keeping_read\":[{read},{}]", read + 3)),
                    "{rendered}"
                );
            } else if index == 1 {
                assert!(
                    rendered.is_empty(),
                    "a single-use chain should not copy: {rendered}"
                );
            }
        }
    }
}

fn hints_match_plain(source: &str, kinds: &[&str]) -> String {
    parity(source);
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let mut last = String::new();
    for optimization in [Optimization::O0, Optimization::O2] {
        for instrumentation in [Instrumentation::Bounded, Instrumentation::Unbounded] {
            let options = Options {
                optimization,
                instrumentation,
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                ..Default::default()
            };
            let plain = nil_llvm::run_arguments(&program.hir, options, &[]).unwrap();
            let hinted = nil_llvm::run_arguments(
                &program.hir,
                Options {
                    memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                    explain_copies: true,
                    ..options
                },
                &[],
            )
            .unwrap();
            let repeated = nil_llvm::run_arguments(
                &program.hir,
                Options {
                    memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                    explain_copies: true,
                    ..options
                },
                &[],
            )
            .unwrap();
            assert_eq!(plain.status.code(), hinted.status.code(), "{source}");
            assert_eq!(plain.stdout, hinted.stdout, "{source}");
            assert_eq!(
                hinted.stderr, repeated.stderr,
                "hints must be deterministic"
            );
            let stderr = String::from_utf8(hinted.stderr).unwrap();
            let non_hints = stderr
                .lines()
                .filter(|line| !line.starts_with("NIL_COPY_HINT "))
                .map(|line| format!("{line}\n"))
                .collect::<String>();
            assert_eq!(plain.stderr, non_hints.as_bytes(), "diagnostics changed");
            for kind in kinds {
                assert!(
                    stderr.contains(&format!("\"kind\":\"{kind}\"")),
                    "missing {kind}: {stderr}"
                );
            }
            assert!(
                !stderr.contains("\"bytes\":0,"),
                "zero-byte work is not a payload copy"
            );
            last = stderr;
        }
    }
    last
}

#[test]
fn copy_hints_report_each_copy_kind_at_a_site_and_skip_zero_byte_work() {
    hints_match_plain(":s=!concat(\"ab\",\"cd\")", &["append_right"]);
    hints_match_plain(
        ":s=b(\"ab\",\"cd\")\n(s,s):s=c(!concat(a,b),a)\n(s,s):s=!concat(a,b)",
        &["concat", "concat_right"],
    );
    let hints = hints_match_plain(":s=!slice(\"abcd\",0,0)", &[]);
    assert!(hints.is_empty(), "empty slice copied no bytes: {hints}");
}

#[test]
fn copy_hints_cover_map_key_value_and_record_materialization() {
    hints_match_plain(
        ":s=b(!put(!bytemap(),\"key\",\"value\"))\n(t):s=!concat(!key(a,0),!get(a,\"key\"))",
        &["map_insert_key", "map_update_value", "map_key", "map_value"],
    );
    hints_match_plain(
        "record Pair(x:i,y:i)\n=b(!put(!map[Pair](),\"key\",Pair(20,22)))\n(map[Pair])=!get(a,\"key\").x+!get(a,\"key\").y",
        &["map_record_value"],
    );
    hints_match_plain(
        "record Z(x:0)\n=#!buffer[Z](2,Z([]))[0:Z([])]",
        &["record_update_value"],
    );
}

#[test]
fn copy_hints_distinguish_cross_frame_retention_from_semantic_reads() {
    let source =
        ":s=original=\"abcd\";changed=b(original);!concat(changed,original)\n(s):s=a[0:90]";
    let hints = hints_match_plain(source, &["replace"]);
    let replace = hints
        .lines()
        .find(|line| line.contains("\"kind\":\"replace\""))
        .unwrap();
    assert!(
        replace.contains("\"keeping_read\":null"),
        "no proved later read inside callee: {replace}"
    );
    assert!(
        replace.contains("\"precision\":\"root-retention-use\""),
        "caller root not explained: {replace}"
    );
    assert!(!replace.contains("\"retaining_use\":null"), "{replace}");
    let parent = "record Holder(text:s,n:i)\n:s=b(Holder(\"abcd\",0))\n(Holder):s=changed=a.text[0:90];n=a.n;changed";
    let hints = hints_match_plain(parent, &["replace"]);
    let replace = hints
        .lines()
        .find(|line| line.contains("\"kind\":\"replace\""))
        .unwrap();
    assert!(
        replace.contains("\"keeping_read\":null"),
        "scalar field retention is not a byte read: {replace}"
    );
    assert!(
        replace.contains("\"precision\":\"root-retention-use\""),
        "{replace}"
    );
}

#[test]
fn copy_hint_child_traversal_and_recursive_frames_preserve_effects_and_traps() {
    hints_match_plain(
        "record Cell(text:s)\n:s=b(!buffer[Cell](1,Cell(\"abc\")))\n(v[Cell]):s=c(a,a[0].text)\n(v[Cell],s):s=changed=b[0:90];ignored=!out(a[0].text);changed",
        &["replace"],
    );
    hints_match_plain(
        ":s=b(\"abc\",6)\n(s,i):s=b==0?a[0:90]:(changed=b(a,b-1);ignored=!out(a);changed)",
        &["replace"],
    );
    hints_match_plain(
        ":s=b(\"abc\")\n(s):s=changed=a[0:90];ignored=!out(a);a[4:90]",
        &["replace"],
    );
    hints_match_plain(
        "record Holder(text:s,n:i)\n:s=b(Holder(!bytes(33554392,97),0))\n(Holder):s=changed=a.text[0:90];temp=#!bytes(100,0);n=a.n;changed",
        &["replace"],
    );
}

#[test]
fn copy_hint_child_budget_exhaustion_reports_unknown_without_changing_execution() {
    let hints = hints_match_plain(
        "record Cell(text:s)\n:s=b(!buffer[Cell](5000,Cell(\"x\"))[4999:Cell(\"abc\")])\n(v[Cell]):s=c(a,a[4999].text)\n(v[Cell],s):s=changed=b[0:90];ignored=!out(a[4999].text);changed",
        &["replace"],
    );
    let copy = hints
        .lines()
        .find(|line| line.contains("\"kind\":\"replace\""))
        .unwrap();
    assert!(copy.contains("\"keeping_read\":null"), "{copy}");
    assert!(
        copy.contains("\"retaining_use\":null"),
        "budget exhaustion must not invent provenance: {copy}"
    );
    assert!(copy.contains("\"reason\":\"unresolved-copy-cause\""));
}

#[test]
fn copy_hint_reasons_distinguish_local_reads_retention_and_materialization() {
    let direct = hints_match_plain(
        ":s=b(\"abcd\")\n(s):s=changed=a[0:90];changed[1:a[9]]",
        &["replace"],
    );
    assert!(direct.contains("\"reason\":\"later-local-read\""));
    let parent = hints_match_plain(
        "record Holder(text:s,n:i)\n:s=b(Holder(\"abcd\",0))\n(Holder):s=changed=a.text[0:90];n=a.n;changed",
        &["replace"],
    );
    assert!(parent.contains("\"reason\":\"live-root-retention\""));
    assert!(parent.contains("\"keeping_read\":null"));
    let materialization = hints_match_plain(":s=!concat(\"ab\",\"cd\")", &["append_right"]);
    assert!(materialization.contains("\"reason\":\"required-materialization\""));
}

#[test]
fn sliced_replay_preserves_allocations_effects_aliases_and_multi_update_traps() {
    for source in [
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[2]][2:a[0]][3:a[1]]",
        ":s=b(\"abcd\")\n(s):s=changed=a[0:90];temp=!bytes(4096,3);changed[1:a[2]+#temp-4096]",
        ":s=b(\"abcd\")\n(s):s=changed=a[0:90];ignored=!out(\"middle\");changed[1:a[2]]",
        ":s=b(\"abcd\")\n(s):s=changed=a[0:90];ignored=!out(\"middle\");changed[1:a[9]]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[2]][9:a[0]]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[2]][2:a[0]+256]",
        ":s=old=\"abcd\";changed=b(old);!concat(changed,old)\n(s):s=a[0:90][1:a[2]][2:a[0]]",
        "record Holder(text:s)\n:s=old=Holder(\"abcd\");changed=b(old.text);!concat(changed,old.text)\n(s):s=a[0:90][1:a[2]][2:a[0]]",
        ":s=b(\"abcd\")\n(s):s=changed=a[0:90];changed[1:a[#changed-2]][2:a[0]]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,3)][2:a[1]]\n(s,i)=b==0?a[0]:d(a,b-1)\n(s,i)=b==0?a[0]:c(a,b-1)",
        ":s=@(\"abcd\",0;b<4;a[0:90][1:a[2]][2:a[0]],b+1;a)",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:(true?(false?a[-1]:a[2]):a[-2])][2:a[0]]",
        ":s=b(\"abcd\")\n(s):s=changed=a[0:90];!concat(changed,a)",
        ":v=b(!buffer(4,97))\n(v):v=a[0:90][1:a[2]][2:a[0]][3:a[1]]",
    ] {
        parity(source);
    }
}

#[test]
fn successful_replay_retires_local_root_but_unknown_sequence_dependencies_copy() {
    let source = ":s=b(\"abcd\")\n(s):s=changed=a[0:90];changed[1:a[2]][2:a[0]]";
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&program.hir);
    assert!(ir.contains("guarded replay retires only the original local root"));
    assert!(ir.contains("select i1"));
    let copying = ":s=b(\"abcd\")\n(s):s=changed=a[0:90];changed[1:a[#changed-2]][2:a[0]]";
    let program = compile_with_profile(copying, SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&program.hir);
    assert!(ir.contains("call ptr @nil_set("));
    assert!(!ir.contains("guarded replay retires only the original local root"));
}

#[test]
fn selected_sequence_regions_snapshot_scalar_continuations() {
    for source in [
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;b<2?a[b:b+10]:a,b+1,c+a[b];c+#a)",
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;b<2?a[0+b:10+b]:a,b+1,c+(b<3?a[b]:a[0]);c+#a)",
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;old=a;index=b+0;new=b<2?old[index:b+10]:old;new,b+1,c+old[index];c+#a)",
        "=b(!buffer(4,7),0)\n(v,i)=@(a,b,0;b<4;b<2?(b<1?a[b:10]:a[b:11]):a,b+1,c+a[b];c+#a)",
        "=b(!buffer(0,7))\n(v)=@(a,0,0;b<#a;b<2?a[b:b+10]:a,b+1,c+a[b];c+#a)",
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;b<2?a[b:b+10]:a,b+1,c+c(a,b,2);c+#a)\n(v,i,i)=c<1?a[b]:d(a,b,c-1)\n(v,i,i)=c<1?a[b]:c(a,b,c-1)",
        "=b(!buffer(4,7))\n(v)=@(0,a,0;a<4;a+1,a<2?b[a:a+10]:b,c+b[a];c+#b)",
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<500;next=b%2==0?a[b%4:b%100]:a;ignored=!bytes(1024,1);next,b+1,c+a[b%4];c+#a)",
    ] {
        let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let ir = nil_llvm::emit_llvm(&program.hir);
        assert!(
            ir.contains("selected continuation retires the outer owner"),
            "{source}"
        );
        parity(source);
    }
}

#[test]
fn selected_sequence_regions_preserve_traps_effects_and_aliases() {
    for source in [
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;b<2?a[b:b+10]:a,b+1,c+a[99];c+#a)",
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;b<2?a[99:b+10]:a,b+1,c+a[-1];c+#a)",
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;b<2?a[b:b+10]:a,b+1,c+!out(\"after\")+a[b];c+#a)",
        "=b(!buffer(4,7))\n(v)=@(a,a,0,0;c<4;c<2?a[c:c+10]:a,b,c+1,d+b[c];d+#a)",
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;b<2?a[b:b+10]:a,b+1,c+(b<4?a[b]:a[-1]);c+#a)",
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;false?a[!out(\"bad\")-99:0]:a,b+1,c+a[b];c+#a)",
        "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;b<2?(x=ignored=!out(\"before\");a[b:b+10];x):a,b+1,c+a[b];c+#a)",
    ] {
        parity(source);
    }
}

#[test]
fn selected_continuations_release_only_the_proved_owner_edge() {
    for (source, copies) in [
        (
            "=b(!buffer(4,7))\n(v)=@(a,0,0;b<4;b<2?a[b:b+10]:a,b+1,c+a[b];c+#a)",
            false,
        ),
        (
            "=old=!buffer(4,7);next=b(old);next+old[0]\n(v)=@(a,0,0;b<4;b<2?a[b:b+10]:a,b+1,c+a[b];c+#a)",
            true,
        ),
        (
            "record R(data:v)\n=old=R(!buffer(4,7));next=b(old.data);next+old.data[0]\n(v)=@(a,0,0;b<4;b<2?a[b:b+10]:a,b+1,c+a[b];c+#a)",
            true,
        ),
    ] {
        parity(source);
        let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        for optimization in [Optimization::O0, Optimization::O2] {
            let actual = nil_llvm::run_arguments(
                &program.hir,
                Options {
                    optimization,
                    instrumentation: Instrumentation::Unbounded,
                    explain_copies: true,
                    ..Default::default()
                },
                &[],
            )
            .unwrap();
            assert!(actual.status.success());
            let hints = String::from_utf8(actual.stderr).unwrap();
            assert_eq!(
                hints.contains("\"kind\":\"replace\""),
                copies,
                "{source}: {hints}"
            );
        }
    }
}

/// R2's transfer marker. Asserting on it proves the interprocedural mechanism
/// ran, rather than inferring a win from LLVM inlining the callee away.
const R2: &str = "R2 surrenders the caller owner only on valid snapshots";

/// ADR 049 R2 variant matrix: argument order, extra parameters, bindings,
/// computed indices, two sequence parameters, bytes, unrelated reads, loop
/// siblings, mutual recursion and a recursive ancestor re-read. None of these
/// is keyed to a program, function name, arity or literal.
#[test]
fn caller_callee_transfer_snapshots_scalar_continuations() {
    for source in [
        // The argument is read again after the call at the same index the
        // callee writes, so a missing snapshot would change the value.
        "=old=!buffer(4,7);next=b(old,0);next[0]+old[0]\n(v,i):v=b<9?a[b:b+10]:a",
        // Sequence in the second argument slot.
        "=old=!buffer(4,7);next=b(0,old);next[0]+old[0]\n(i,v):v=a<9?b[a:a+10]:b",
        "=old=!buffer(4,7);next=b(old,0,1,2);next[0]+old[0]\n(v,i,i,i):v=d<9?a[b:b+10+c]:a",
        "=old=!buffer(4,7);next=b(old,0);idx=0;n=next[idx];o=old[idx];n+o\n(v,i):v=b<9?a[b:b+10]:a",
        "=old=!buffer(4,7);next=b(old,0);next[0]+old[1+1-2]\n(v,i):v=b<9?a[b:b+10]:a",
        "=old=!buffer(4,7);key=!buffer(2,1);next=b(old,key);next[0]+old[0]\n(v,v):v=a[b[0]:10]",
        "=old=\"abcd\";next=b(old,0);next[0]+old[0]\n(s,i):s=b<9?a[b:b+10]:a",
        "=old=!buffer(4,7);other=!buffer(2,3);next=b(old,0);next[0]+old[0]+other[1]\n(v,i):v=b<9?a[b:b+10]:a",
        // Loop state passed to a call whose sibling state expression reads the
        // original afterwards: the shape the natural witness uses.
        "=@(!buffer(4,7),0,0;b<4;d(a,b),b+1,c+a[b];c+#a)\n(v):v=a\n(v):v=a\n(v,i):v=b<9?a[b:b+10]:a",
        "=old=!buffer(4,7);next=b(old,2);next[0]+old[0]\n(v,i):v=b<1?a[0:10]:c(a,b-1)\n(v,i):v=b<1?a[1:11]:b(a,b-1)",
        // A recursive ancestor re-reads its own buffer after the recursive call
        // mutated that storage. The pre-call snapshot is what keeps this sound.
        "=o=b(!buffer(4,7),2);o[0]+o[1]+o[2]\n(v,i):v=b<1?a[1:99]:(n=b(a,b-1);n[0:a[1]])",
        // A specialized static callback leaves an ordinary resolved call.
        "=old=!buffer(4,7);next=b(old,&d);next[0]+old[0]\n(v,[i:i]):v=^b(a[0])<99?a[0:10]:a\n(v,i):v=b<9?a[b:b+10]:a\n1=a+1",
    ] {
        let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let ir = nil_llvm::emit_llvm(&program.hir);
        assert!(ir.contains(R2), "{source}");
        parity(source);
    }
}

/// Traps, effects, laziness, collection after the handoff and degenerate
/// layouts keep their original meaning whether or not the snapshot is valid.
#[test]
fn caller_callee_transfer_preserves_traps_effects_and_laziness() {
    for source in [
        // Effect after the call stays at its own site; the probe is pure.
        "=old=!buffer(4,7);next=b(old,0);next[0]+!out(\"E\")+old[0]\n(v,i):v=b<9?a[b:b+10]:a",
        // Invalid future probe: the owner is retained and the original read
        // traps at its own span.
        "=old=!buffer(4,7);next=b(old,0);next[0]+old[99]\n(v,i):v=b<9?a[b:b+10]:a",
        "=old=!buffer(4,7);next=b(old,99);next[0]+old[0]\n(v,i):v=b<9?a[b:b+10]:a",
        // Allocation large enough to collect after an accepted handoff.
        "=old=!buffer(4,7);next=b(old,0);big=!bytes(4096,3);next[0]+old[0]+#big-4096\n(v,i):v=b<9?a[b:b+10]:a",
        "=old=!buffer(0,7);next=b(old,0);#next+#old\n(v,i):v=b<9?a[b:b+10]:a",
        "=old=\"abcd\";next=b(old,0);next[0]+old[0]\n(s,i):s=b<9?a[b:300]:a",
        // The unselected arm's effect and trap must not run.
        "=old=!buffer(4,7);next=b(old,0);next[0]+old[0]\n(v,i):v=b<9?a[b:b+10]:a[!out(\"BAD\")-99:0]",
        "record Z(x:0)\n=old=!buffer(4,7);z=Z([]);next=b(old,0);next[0]+old[0]+#z.x\n(v,i):v=b<9?a[b:b+10]:a",
    ] {
        parity(source);
    }
}

/// Transfer is refused where the proof is missing, and where it is granted the
/// runtime root count still protects every independent owner. `copies` is the
/// observed runtime copy, read from hints rather than inferred from the IR.
#[test]
fn caller_callee_transfer_keeps_independent_owners_counted() {
    for (source, transfers, copies) in [
        // Baseline: the only owner is surrendered and no copy remains.
        (
            "=old=!buffer(4,7);next=b(old,0);next[0]+old[0]\n(v,i):v=b<9?a[b:b+10]:a",
            true,
            false,
        ),
        // Repeated argument: two parameter slots, so no root is surrendered.
        (
            "=old=!buffer(4,7);next=b(old,old);next[0]+old[0]\n(v,v):v=a[0:b[0]]",
            false,
            true,
        ),
        // The caller observes the original as a sequence after the call.
        (
            "=old=\"abcd\";next=b(old,0);#!concat(next,old)\n(s,i):s=b<9?a[b:b+10]:a",
            false,
            true,
        ),
        // A live record parent keeps its child edge counted.
        (
            "record R(data:v)\n=held=R(!buffer(4,7));d=held.data;next=b(d,0);next[0]+held.data[0]\n(v,i):v=b<9?a[b:b+10]:a",
            false,
            true,
        ),
        // An independently retained returned alias: the transfer is admitted
        // for the argument, and the other owner still forces the copy.
        (
            "=old=!buffer(4,7);mid=c(old,0);next=b(mid,0);next[0]+old[0]+mid[1]\n(v,i):v=b<9?a[b:b+10]:a\n(v,i):v=b<9?a:a",
            true,
            true,
        ),
        // Two loop state slots holding one allocation.
        (
            "=seq=!buffer(4,7);@(seq,seq,0,0;c<4;d(a,c),b,c+1,d+b[c];d+#a)\n(v):v=a\n(v):v=a\n(v,i):v=b<9?a[b:b+10]:a",
            false,
            true,
        ),
        // The callee stores the parameter in a record and returns it, so its
        // own update cannot reuse the storage.
        (
            "record R(data:v,other:v)\n=old=!buffer(4,7);next=b(old,0);next.data[0]+next.other[0]+old[0]\n(v,i):R=R(b<9?a[b:b+10]:a,a)",
            true,
            true,
        ),
        // Read-only borrow callee: no parameter root, so nothing is handed over.
        (
            "=old=!buffer(4,7);next=b(old,0);next+old[0]\n(v,i)=b<9?a[b]+a[0]:a[1]",
            false,
            false,
        ),
        // Unknown return provenance blocks transfer even though the parameter
        // looks dead after the call.
        (
            "=old=\"abcd\";next=b(old,0);next[0]+old[0]\n(s,i):s=b<9?!concat(a,\"z\"):a",
            false,
            false,
        ),
        (
            "=old=\"abcd\";next=b(old,0);next[0]+old[0]\n(s,i):s=b<9?!slice(a,0,#a-1):a",
            false,
            false,
        ),
        // Sequence-taking std higher-order intrinsics are never transfer
        // anchors; a callback itself cannot receive a sequence in this profile.
        (
            "=old=!buffer(4,7);next=!map(&c,old);next[0]+old[0]\n(v,i):v=b<9?a[b:b+10]:a\n1=a+1",
            false,
            false,
        ),
    ] {
        parity(source);
        let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        let ir = nil_llvm::emit_llvm(&program.hir);
        assert_eq!(ir.contains(R2), transfers, "transfer: {source}");
        for optimization in [Optimization::O0, Optimization::O2] {
            let actual = nil_llvm::run_arguments(
                &program.hir,
                Options {
                    optimization,
                    instrumentation: Instrumentation::Unbounded,
                    explain_copies: true,
                    ..Default::default()
                },
                &[],
            )
            .unwrap();
            let hints = String::from_utf8(actual.stderr).unwrap();
            assert_eq!(
                hints.contains("\"kind\":\"replace\""),
                copies,
                "copies: {source}: {hints}"
            );
        }
    }
}

/// Bounded builds keep their instruction and depth observations, so no call
/// transfer is admitted there even when the scalar proof would succeed.
#[test]
fn bounded_builds_never_transfer_a_caller_owner() {
    let source = "=old=!buffer(4,7);next=b(old,0);next[0]+old[0]\n(v,i):v=b<9?a[b:b+10]:a";
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    assert!(nil_llvm::emit_llvm(&program.hir).contains(R2));
    assert!(
        !nil_llvm::emit_llvm_with_instrumentation(&program.hir, Instrumentation::Bounded)
            .contains(R2)
    );
}

/// ADR 053 step 1. A `roots == 2` update copy must report its site, its reason
/// and the root that retains the buffer.
///
/// This exists because ADR 051 addendum A claimed the instrument could not see
/// this copy at all, and withdrew that claim in addendum C: the hints were
/// always emitted and the flag had been passed after the subcommand, so the
/// CLI exited with E010 without running anything. The test locks the behaviour
/// so the claim cannot be made a fourth time.
#[test]
fn a_roots_two_update_copy_reports_site_reason_and_retaining_root() {
    // The buffer is updated in a loop while an earlier loop-state slot still
    // holds it, so the update has two roots and cannot borrow.
    let source = "=#b(!buffer(4096,0))\n\
                  (v):v=@(a,a,0;c<64;a,b[c:c],c+1;b)\n";
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &program.hir,
            Options {
                optimization,
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                explain_copies: true,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert!(
            out.status.success(),
            "{optimization:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let rendered = String::from_utf8(out.stderr).unwrap();
        assert!(
            rendered.contains("NIL_COPY_HINT"),
            "{optimization:?}: a two-root update must report a copy, got {rendered:?}"
        );
        assert!(
            rendered.contains("\"kind\":\"replace\""),
            "{optimization:?}: expected a replace copy, got {rendered:?}"
        );
        // The site, the reason and the retaining root are all required: a bare
        // "something copied" line is what made the earlier misreading possible.
        assert!(
            rendered.contains("\"span\":["),
            "{optimization:?}: missing copy site, got {rendered:?}"
        );
        assert!(
            rendered.contains("\"reason\":\"live-root-retention\""),
            "{optimization:?}: expected live-root-retention, got {rendered:?}"
        );
        assert!(
            rendered.contains("\"retaining_use\":["),
            "{optimization:?}: missing retaining root, got {rendered:?}"
        );
    }
}

/// ADR 054 addendum A. Pins the live-root-retention copy at its minimal
/// reproducer: one loop slot updates a buffer in place while a sibling slot
/// passes the same buffer to a non-leaf call, so the update sees two roots.
///
/// With deferred slot writes disabled, the copy **is** reported. It
/// is the before-picture for the fix, and it records that a *leaf* callee does
/// not reproduce it, because leaf calls are source-expanded and never root.
#[test]
fn eager_sibling_calls_retain_the_original_copying_behavior() {
    let copying = "=#b(!buffer(256,0))\n\
                   (v):v=@(a,0,0;b<64;a[b:b],b+1,c+c(a,b);a)\n\
                   (v,i)=@(a,b;a[b]!=b;a,a[b];b)\n";
    // Same shape, but the sibling calls a leaf, which is expanded and so takes
    // no root: this one must not copy.
    let borrowing = "=#b(!buffer(256,0))\n\
                     (v):v=@(a,0,0;b<64;a[b:b],b+1,c+c(a,b);a)\n\
                     (v,i)=a[b]\n";
    for (source, expect_copy) in [(copying, true), (borrowing, false)] {
        let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        for optimization in [Optimization::O0, Optimization::O2] {
            let out = nil_llvm::run_arguments(
                &program.hir,
                Options {
                    defer_slot_writes: false,
                    optimization,
                    memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                    explain_copies: true,
                    ..Default::default()
                },
                &[],
            )
            .unwrap();
            assert!(out.status.success(), "{optimization:?} must run");
            let rendered = String::from_utf8(out.stderr).unwrap();
            let copied = rendered.contains("\"kind\":\"replace\"");
            assert_eq!(
                copied, expect_copy,
                "{optimization:?}: expected replace copy = {expect_copy}, got {rendered:?}"
            );
            if expect_copy {
                assert!(
                    rendered.contains("\"reason\":\"live-root-retention\""),
                    "{optimization:?}: {rendered:?}"
                );
                assert!(
                    rendered.contains("\"retaining_use\":["),
                    "{optimization:?}: the retaining call must be named: {rendered:?}"
                );
            }
        }
    }
}

/// ADR 055 addendum C. Proves the evaluation order for **sibling loop-state
/// slots**, which ADR 048 does not govern — it covers contiguous update chains.
///
/// In both slot orders a borrowing non-leaf call observes the **pre-update**
/// element, which is what parallel loop-state semantics require. The orders
/// now both avoid a copy under ADR 055 D: writes commit after the readers.
/// The eager-semantics test above preserves the original before-picture.
#[test]
fn a_sibling_call_observes_the_pre_update_element_in_both_slot_orders() {
    // Callee is non-leaf (it contains a loop) and returns the element being
    // updated. 0 means it saw the old element; anything else means it saw the
    // new one.
    let callee = "(v,i)=@(a,b,0;c<1;a,b,c+1;a[b])\n";
    // Update in slot 1, borrowing call in slot 3: the call is ordered after
    // the write.
    let read_after =
        format!("=b(!buffer(8,0))\n(v)=@(a,0,0;b<4;a[b:b+100],b+1,c*1000+c(a,b);c)\n{callee}");
    // Borrowing call in slot 1, update in slot 2: the call is ordered before
    // the write.
    let read_before =
        format!("=b(!buffer(8,0))\n(v)=@(0,a,0;c<4;a*1000+c(b,c),b[c:c+100],c+1;a)\n{callee}");
    for (source, expect_copy) in [(&read_after, false), (&read_before, false)] {
        let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        for optimization in [Optimization::O0, Optimization::O2] {
            let out = nil_llvm::run_arguments(
                &program.hir,
                Options {
                    optimization,
                    memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                    explain_copies: true,
                    ..Default::default()
                },
                &[],
            )
            .unwrap();
            assert!(out.status.success(), "{optimization:?} must run");
            // The pre-update element is what both orders must observe.
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).trim(),
                "0",
                "{optimization:?}: a sibling call must see the pre-update element"
            );
            let rendered = String::from_utf8(out.stderr).unwrap();
            assert_eq!(
                rendered.contains("\"kind\":\"replace\""),
                expect_copy,
                "{optimization:?}: expected replace copy = {expect_copy}, got {rendered:?}"
            );
        }
    }
}

/// ADR 055 addendum C, the GC half: a sibling slot that allocates between the
/// update and the borrowing call's read must not let the pre-update buffer be
/// collected. Run under `scripts/sanitize.sh` this also checks for use after
/// free.
#[test]
fn allocation_between_the_update_and_the_sibling_read_keeps_the_old_buffer_alive() {
    let source = "=b(!buffer(8,0))\n\
                  (v)=@(a,0,0;b<4;a[b:b+100],b+1,c*1000+c(a,b)+#!buffer(4096,b);c)\n\
                  (v,i)=@(a,b,0;c<1;a,b,c+1;a[b])\n";
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &program.hir,
            Options {
                optimization,
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert!(
            out.status.success(),
            "{optimization:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        // Each iteration adds 4096 from the allocation and 0 from the
        // pre-update read, so the accumulator is a fixed, checkable value.
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "4100100100096",
            "{optimization:?}: pre-update read or allocation accounting changed"
        );
    }
}
