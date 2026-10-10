//! ADR 048's general contiguous-chain and evaluation-boundary matrix.
pub const FAMILY_COUNT: usize = 88;
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
    result.extend([
        ("inline_checked_reordered", "(v,s,s,s)=seq=!bytes(2,3);next=b(seq,0,1);next[0]+next[1]+seq[0]\n(s,i,i):s=(a[c:a[b]])[b:a[c]]".into()),
        ("inline_checked_bound_reads", "(v,s,s,s)=seq=!bytes(2,3);next=b(seq,0,1);next[0]+next[1]\n(s,i,i):s=right=a[c];left=a[b];a[b:right][c:left]".into()),
        ("inline_checked_helper_operands", "(v,s,s,s)=seq=!bytes(2,3);next=b(seq,c(),1);next[0]+seq[0]\n(s,i,i):s=a[b:a[c]+0][c:a[b]+0]\n=calls=!out(\"I\");0".into()),
        ("inline_checked_argument_effects", "(v,s,s,s)=next=b((ignored=!out(\"A\");\"ab\"),(ignored=!out(\"B\");0),(ignored=!out(\"C\");1));next[0]\n(s,i,i):s=a[b:a[c]][c:a[b]]".into()),
        ("inline_checked_unused_argument", "(v,s,s,s)=#b(\"ab\",!out(\"A\"))\n(s,i):s=!slice(a,0,#a-1)".into()),
        ("inline_checked_lazy_trap", "(v,s,s,s)=#(false?b(\"\",!out(\"BAD\"),9):\"ok\")\n(s,i,i):s=a[b:a[c]][c:a[b]]".into()),
        ("inline_checked_bounds_order", "(v,s,s,s)=#b(\"ab\",-1,9)\n(s,i,i):s=right=a[c];left=a[b];a[b:right][c:left]".into()),
        ("inline_checked_loop_alias", "(v,s,s,s)=seq=\"ab\";next=@(seq,0;b<8;b(a,0,1),b+1;a);next[0]+seq[0]\n(s,i,i):s=a[b:a[c]][c:a[b]]".into()),
    ]);
    result.extend([
        ("g1_selected_loop_scalar", format!("(v,s,s,s)=@(!buffer(4,{fill}),0,0;b<4;b<2?a[b:b+10]:a,b+1,c+a[b];c+#a)")),
        ("g1_selected_computed_index", "(v,s,s,s)=@(!buffer(4,7),0,0;b<4;b<2?a[0+b:10+b]:a,b+1,c+(b<3?a[b]:a[0]);c+#a)".into()),
        ("g1_selected_bindings", "(v,s,s,s)=@(!buffer(4,7),0,0;b<4;old=a;index=b+0;new=b<2?old[index:b+10]:old;new,b+1,c+old[index];c+#a)".into()),
        ("g1_selected_nested", "(v,s,s,s)=@(!buffer(4,7),0,0;b<4;b<2?(b<1?a[b:10]:a[b:11]):a,b+1,c+a[b];c+#a)".into()),
        ("g1_selected_bytes", "(v,s,s,s)=@(\"abcd\",0,0;b<4;b<2?a[b:b+10]:a,b+1,c+a[b];c+#a)".into()),
        ("g1_selected_empty", "(v,s,s,s)=@(!buffer(0,7),0,0;b<#a;b<2?a[b:b+10]:a,b+1,c+a[b];c+#a)".into()),
        ("g1_selected_live_alias", "(v,s,s,s)=seq=!buffer(4,7);@ (seq,seq,0,0;c<4;c<2?a[c:c+10]:a,b,c+1,d+b[c];d+#a)".into()),
        ("g1_selected_unselected_trap", "(v,s,s,s)=@(!buffer(4,7),0,0;b<4;false?a[!out(\"BAD\")-99:0]:a,b+1,c+a[b];c+#a)".into()),
        ("g1_selected_future_bounds", "(v,s,s,s)=@(!buffer(4,7),0,0;b<4;b<2?a[b:b+10]:a,b+1,c+a[99];c+#a)".into()),
        ("g1_selected_update_bounds", "(v,s,s,s)=@(!buffer(4,7),0,0;b<4;b<2?a[99:b+10]:a,b+1,c+a[-1];c+#a)".into()),
        ("g1_selected_future_effect", "(v,s,s,s)=@(!buffer(4,7),0,0;b<4;b<2?a[b:b+10]:a,b+1,c+!out(\"after\")+a[b];c+#a)".into()),
        ("g1_selected_byte_range", "(v,s,s,s)=@(\"abcd\",0,0;b<4;b<2?a[b:256]:a,b+1,c+a[-1];c+#a)".into()),
        ("g1_selected_scalar_helper", "(v,s,s,s)=@(!buffer(4,7),0,0;b<4;b<2?a[b:b+10]:a,b+1,c+b(a,b);c+#a)\n(v,i)=a[b]".into()),
        ("g1_selected_returned_alias", "(v,s,s,s)=seq=!buffer(4,7);next=@(seq,0,0;b<4;b<2?a[b:b+10]:a,b+1,c+a[b];a);seq[0]+next[0]".into()),
        ("g1_selected_record_parent", "record R(data:v)\n(v,s,s,s)=old=R(!buffer(4,7));next=@(old.data,0,0;b<4;b<2?a[b:b+10]:a,b+1,c+a[b];a);old.data[0]+next[0]".into()),
        ("g1_selected_local_old_read", "(v,s,s,s)=@(!buffer(4,7),0,0;b<4;b<2?(next=a[b:b+10];next[0:a[b]]):a,b+1,c+a[b];c+#a)".into()),
    ]);
    result.extend([
        ("g1_selected_mutual_scalar", "(v,s,s,s)=@(!buffer(4,7),0,0;b<4;b<2?a[b:b+10]:a,b+1,c+b(a,b,2);c+#a)\n(v,i,i)=c<1?a[b]:c(a,b,c-1)\n(v,i,i)=c<1?a[b]:b(a,b,c-1)".into()),
        ("g1_selected_state_order", "(v,s,s,s)=@(0,!buffer(4,7),0;a<4;a+1,a<2?b[a:a+10]:b,c+b[a];c+#b)".into()),
        ("g1_selected_collection_after_handoff", "(v,s,s,s)=@(!buffer(4,7),0,0;b<500;next=b%2==0?a[b%4:b%100]:a;ignored=!bytes(1024,1);next,b+1,c+a[b%4];c+#a)".into()),
    ]);
    // ADR 049 R2: caller/callee root split. Positive shapes vary argument
    // order, arity, bindings, index spelling, recursion and loop siblings;
    // negative shapes keep an independent owner counted.
    result.extend([
        ("g1_transfer_basic", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,0);next[0]+old[0]\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_argument_order", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(0,old);next[0]+old[0]\n(i,v):v=a<9?b[a:a+10]:b")),
        ("g1_transfer_extra_parameters", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,0,1,2);next[0]+old[0]\n(v,i,i,i):v=d<9?a[b:b+10+c]:a")),
        ("g1_transfer_bindings", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,0);idx=0;n=next[idx];o=old[idx];n+o\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_computed_index", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,0);next[0]+old[1+1-2]\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_two_sequences", format!("(v,s,s,s)=old=!buffer(4,{fill});key=!buffer(2,1);next=b(old,key);next[0]+old[0]\n(v,v):v=a[b[0]:10]")),
        ("g1_transfer_bytes", "(v,s,s,s)=old=\"abcd\";next=b(old,0);next[0]+old[0]\n(s,i):s=b<9?a[b:b+10]:a".into()),
        ("g1_transfer_unrelated_read", format!("(v,s,s,s)=old=!buffer(4,{fill});other=!buffer(2,3);next=b(old,0);next[0]+old[0]+other[1]\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_loop_sibling", format!("(v,s,s,s)=@(!buffer(4,{fill}),0,0;b<4;d(a,b),b+1,c+a[b];c+#a)\n(v):v=a\n(v):v=a\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_mutual_recursion", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,2);next[0]+old[0]\n(v,i):v=b<1?a[0:10]:c(a,b-1)\n(v,i):v=b<1?a[1:11]:b(a,b-1)")),
        ("g1_transfer_recursive_ancestor", format!("(v,s,s,s)=o=b(!buffer(4,{fill}),2);o[0]+o[1]+o[2]\n(v,i):v=b<1?a[1:99]:(n=b(a,b-1);n[0:a[1]])")),
        ("g1_transfer_specialized_callback", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,&d);next[0]+old[0]\n(v,[i:i]):v=^b(a[0])<99?a[0:10]:a\n(v,i):v=b<9?a[b:b+10]:a\n1=a+1")),
        ("g1_transfer_future_effect", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,0);next[0]+!out(\"E\")+old[0]\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_future_bounds", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,0);next[0]+old[99]\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_update_bounds", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,99);next[0]+old[0]\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_collection_after", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,0);big=!bytes(4096,3);next[0]+old[0]+#big-4096\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_empty", format!("(v,s,s,s)=old=!buffer(0,{fill});next=b(old,0);#next+#old\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_byte_range", "(v,s,s,s)=old=\"abcd\";next=b(old,0);next[0]+old[0]\n(s,i):s=b<9?a[b:300]:a".into()),
        ("g1_transfer_unselected_arm", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,0);next[0]+old[0]\n(v,i):v=b<9?a[b:b+10]:a[!out(\"BAD\")-99:0]")),
        ("g1_transfer_zero_slot", format!("record Z(x:0)\n(v,s,s,s)=old=!buffer(4,{fill});z=Z([]);next=b(old,0);next[0]+old[0]+#z.x\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_repeated_argument", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,old);next[0]+old[0]\n(v,v):v=a[0:b[0]]")),
        ("g1_transfer_sequence_use_after", "(v,s,s,s)=old=\"abcd\";next=b(old,0);#!concat(next,old)\n(s,i):s=b<9?a[b:b+10]:a".into()),
        ("g1_transfer_live_record_parent", format!("record R(data:v)\n(v,s,s,s)=held=R(!buffer(4,{fill}));d=held.data;next=b(d,0);next[0]+held.data[0]\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_returned_alias", format!("(v,s,s,s)=old=!buffer(4,{fill});mid=c(old,0);next=b(mid,0);next[0]+old[0]+mid[1]\n(v,i):v=b<9?a[b:b+10]:a\n(v,i):v=b<9?a:a")),
        ("g1_transfer_loop_two_slots", format!("(v,s,s,s)=seq=!buffer(4,{fill});@(seq,seq,0,0;c<4;d(a,c),b,c+1,d+b[c];d+#a)\n(v):v=a\n(v):v=a\n(v,i):v=b<9?a[b:b+10]:a")),
        ("g1_transfer_callee_stores", format!("record R(data:v,other:v)\n(v,s,s,s)=old=!buffer(4,{fill});next=b(old,0);next.data[0]+next.other[0]+old[0]\n(v,i):R=R(b<9?a[b:b+10]:a,a)")),
        ("g1_transfer_borrow_callee", format!("(v,s,s,s)=old=!buffer(4,{fill});next=b(old,0);next+old[0]\n(v,i)=b<9?a[b]+a[0]:a[1]")),
        ("g1_transfer_unknown_origin", "(v,s,s,s)=old=\"abcd\";next=b(old,0);next[0]+old[0]\n(s,i):s=b<9?!concat(a,\"z\"):a".into()),
        ("g1_transfer_std_hof", format!("(v,s,s,s)=old=!buffer(4,{fill});next=!map(&c,old);next[0]+old[0]\n(v,i):v=b<9?a[b:b+10]:a\n1=a+1")),
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
