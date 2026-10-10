//! ADR 048's general contiguous-chain and evaluation-boundary matrix.
pub const FAMILY_COUNT: usize = 32;
pub fn cases(seed: u64) -> Vec<(&'static str, String)> {
    let fill = seed % 200 + 1;
    let prefix = "(v,s,s,s)";
    let rows = [
        (
            "chain_buffer_swap",
            format!("seq=!buffer(2,{fill});next=seq[0:seq[1]+1][1:seq[0]];next[0]+next[1]+seq[0]"),
        ),
        (
            "chain_bytes_swap",
            format!("seq=!bytes(2,{fill});next=seq[0:seq[1]+1][1:seq[0]];next[0]+next[1]+seq[0]"),
        ),
        (
            "chain_duplicate_index",
            "seq=[1,2];next=seq[0:7][0:8];next[0]+seq[0]".into(),
        ),
        (
            "chain_reordered",
            "seq=[1,2];next=seq[1:seq[0]][0:1+seq[1]-1];next[0]+next[1]+seq[1]".into(),
        ),
        (
            "chain_operand_binding",
            "seq=[1,2];next=seq[0:(idx=1;seq[idx])][1:(idx=0;seq[idx])];next[0]+next[1]".into(),
        ),
        (
            "chain_grouped",
            "seq=[1,2];next=(seq[0:seq[1]])[1:seq[0]];next[0]+next[1]+seq[0]".into(),
        ),
        (
            "chain_double_grouped",
            "seq=[1,2];next=((seq[0:seq[1]]))[1:seq[0]];next[0]+next[1]".into(),
        ),
        (
            "chain_binding_boundary",
            "seq=!bytes(1,0);next=seq[9:!out(\"A\")];#next[0:!out(\"BAD\")]".into(),
        ),
        (
            "chain_read_boundary",
            "seq=!bytes(1,0);seq[9:!out(\"A\")][0]+!out(\"BAD\")".into(),
        ),
        (
            "chain_helper",
            "seq=!bytes(2,3);next=seq[0:b(seq,1)][1:b(seq,0)];next[0]+next[1]\n(s,i)=a[b]".into(),
        ),
        (
            "chain_selected_lazy",
            "seq=[1,2];next=seq[0:(true?seq[1]:1/0)][1:seq[0]];next[0]+next[1]".into(),
        ),
        (
            "chain_unselected_lazy",
            "false?(#!bytes(1,0)[9:!out(\"BAD\")][0:1/0]):42".into(),
        ),
        (
            "chain_loop",
            "seq=@(!buffer(2,1),0;b<8;a[0:a[1]+1][1:a[0]],b+1;a);seq[0]+seq[1]".into(),
        ),
        (
            "chain_caller_alias",
            "next=b(a);#next+#a+(#a>0?a[0]:0)\n(v):v=#a<2?a:a[0:a[1]][1:a[0]]".into(),
        ),
        (
            "chain_index_ternary",
            "seq=[1,2];next=seq[true?0:1:seq[1]][1:seq[0]];next[0]+next[1]".into(),
        ),
        (
            "chain_nested_operand",
            "seq=!buffer(2,1);next=seq[0:(seq[1:9])[0]][1:seq[0]];next[0]+next[1]+seq[1]".into(),
        ),
        (
            "chain_index_first_trap",
            "#!bytes(2,0)[1/0:!out(\"BAD\")][0:1]".into(),
        ),
        (
            "chain_index_second_trap",
            "#!bytes(2,0)[0:!out(\"A\")][1/0:!out(\"BAD\")]".into(),
        ),
        (
            "chain_index_third_trap",
            "#!bytes(2,0)[0:!out(\"A\")][1:!out(\"B\")][1/0:0]".into(),
        ),
        (
            "chain_value_first_trap",
            "#!bytes(2,0)[0:1/0][1:!out(\"BAD\")]".into(),
        ),
        (
            "chain_value_second_trap",
            "#!bytes(2,0)[0:!out(\"A\")][1:!parse(\"bad\")]".into(),
        ),
        (
            "chain_value_third_trap",
            "#!bytes(2,0)[0:!out(\"A\")][1:!out(\"B\")][0:1/0]".into(),
        ),
        (
            "chain_effect_order",
            "#!bytes(2,0)[!out(\"I\")-1:!out(\"V\")][!out(\"J\")-1:!out(\"W\")]".into(),
        ),
        (
            "chain_write_bounds_after_effects",
            "#!bytes(1,0)[9:!out(\"A\")][0:!out(\"B\")]".into(),
        ),
        (
            "chain_write_byte_after_effects",
            "#!bytes(1,0)[0:256][0:!out(\"A\")]".into(),
        ),
        ("chain_empty_buffer", "#!buffer(0,0)[0:1][0:1/0]".into()),
        (
            "chain_empty_bytes",
            "#!bytes(0,0)[0:1][0:!out(\"A\")]".into(),
        ),
        (
            "chain_nested_loop_lazy",
            "seq=@(!bytes(2,1),0;b<8;(true?(a[0:a[1]+1])[1:a[0]]:a),b+1;a);seq[0]+seq[1]".into(),
        ),
    ];
    let mut result = rows
        .into_iter()
        .map(|(name, body)| (name, format!("{prefix}={body}")))
        .collect::<Vec<_>>();
    result.extend([
        ("chain_record_child", "record Pair(seq:v,n:i)\n(v,s,s,s)=old=Pair(!buffer(2,1),2);new=(old{seq:old.seq[0:7][1:old.seq[0]]}){n:old.seq[0]};new.seq[1]+new.n+old.seq[0]".into()),
        ("chain_zero_slot", "record Z(x:0)\nrecord Pair(z:Z,n:i)\n(v,s,s,s)=old=Pair(Z([]),1);new=old{z:old.z{x:[]}}{n:old.n+1};new.n".into()),
        ("chain_record_buffer_child", "record Pair(seq:v,n:i)\n(v,s,s,s)=old=!buffer[Pair](2,Pair(!buffer(2,1),3));new=old[0:old[0]{seq:old[0].seq[0:7][1:old[0].seq[0]]}{n:old[0].n+1}];new[0].seq[1]+old[0].seq[0]+new[0].n".into()),
        ("chain_record_effect_order", "record Pair(x:i,y:i)\n(v,s,s,s)=old=Pair(1,2);new=(old{x:!out(\"A\")}){y:!out(\"B\")};new.x+new.y+old.x".into()),
    ]);
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn registered_chain_families_all_have_valid_first_order_hir() {
        let cases = super::cases(24301);
        assert_eq!(cases.len(), super::FAMILY_COUNT);
        for (name, source) in cases {
            nil_compiler::compile_with_profile(&source, nil_compiler::SourceProfile::ExprV5)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
}
