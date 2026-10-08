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
        Limits::default(),
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
                let prefix = format!("{} @{}..{} ", error.code, span.start, span.end);
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
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[2]]",
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
fn trapping_reads_fall_back_and_keep_update_failure_before_read_failure() {
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
fn effects_and_quota_observations_are_not_crossed_even_when_result_is_unused() {
    for source in [
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a)]\n(s)=ignored=!out(\"middle\");a[0]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:(true?(ignored=!out(\"middle\");a[0]):a[1])]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:(false?(ignored=!out(\"BAD\");a[-1]):a[1])]",
        ":s=b(\"abcd\")\n(s):s=a[0:90][1:(ignored=#!bytes(67108824,0);a[0])]",
        "record Holder(text:s,n:i)\n:s=b(Holder(!bytes(33554392,97),0))\n(Holder):s=changed=a.text[0:90];temp=#!bytes(100,0);n=a.n;changed",
    ] {
        parity(source);
    }
}

#[test]
fn multi_update_snapshot_does_not_erase_an_intervening_quota_reservation() {
    // Three semantic capacities exceed 64 MiB. Reusing the first write while
    // an old read crosses the second would incorrectly charge only two.
    let source = ":s=b(!bytes(22369590,97))\n(s):s=a[0:90][1:a[2]][2:a[0]]";
    parity(source);
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    assert!(nil_llvm::emit_llvm(&program.hir).contains("call ptr @nil_set("));
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
    ];
    for (index, source) in sources.iter().enumerate() {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        for optimization in [Optimization::O0, Optimization::O2] {
            let options = Options {
                optimization,
                instrumentation: Instrumentation::Unbounded,
                ..Default::default()
            };
            let plain = nil_llvm::run_arguments(&p.hir, options, &[]).unwrap();
            let hints = nil_llvm::run_arguments(
                &p.hir,
                Options {
                    explain_copies: true,
                    ..options
                },
                &[],
            )
            .unwrap();
            let again = nil_llvm::run_arguments(
                &p.hir,
                Options {
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
                assert!(rendered.contains("\"kind\":\"replace\""), "{rendered}");
                assert!(
                    rendered.contains("\"precision\":\"direct-local-read\""),
                    "{rendered}"
                );
                let read = source.find("a[2]").unwrap() + 1;
                assert!(
                    rendered.contains(&format!("\"keeping_read\":[{read},{}]", read + 3)),
                    "{rendered}"
                );
            } else if index == 1 {
                assert!(
                    rendered.is_empty(),
                    "a replayed update should not copy: {rendered}"
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
                ..Default::default()
            };
            let plain = nil_llvm::run_arguments(&program.hir, options, &[]).unwrap();
            let hinted = nil_llvm::run_arguments(
                &program.hir,
                Options {
                    explain_copies: true,
                    ..options
                },
                &[],
            )
            .unwrap();
            let repeated = nil_llvm::run_arguments(
                &program.hir,
                Options {
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
}
