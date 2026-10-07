//! ADR 044's general read/alias matrix; source shapes vary independently of spelling.
pub const FAMILY_COUNT: usize = 29;
pub fn cases(seed: u64) -> Vec<(&'static str, String)> {
    let index = seed % 4;
    vec![
        ("g2_different_index", format!(":s=b(\"abcd\")\n(s):s=a[0:90][1:a[{index}]]")),
        ("g2_multiple_reads", ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[0]+a[1]-97]".into()),
        ("g2_reordered_operands", ":s=b(\"abcd\")\n(s):s=a[0:90][1:1+a[2]-1]".into()),
        ("g2_index_binding", ":s=b(\"abcd\")\n(s):s=idx=2;a[0:90][1:a[idx]]".into()),
        ("g2_read_before_and_after", ":s=b(\"abcd\")\n(s):s=a[0:a[1]][1:a[0]]".into()),
        ("g2_readonly_callee", ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,2)]\n(s,i)=a[b]".into()),
        ("g2_callee_conditional", ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,2)]\n(s,i)=b==-1?0:a[b]".into()),
        ("g2_nested_lazy", ":s=b(\"abcd\")\n(s):s=a[0:90][1:(true?(false?a[-1]:a[2]):a[-2])]".into()),
        ("g2_selected_bad_read", ":s=b(\"abcd\")\n(s):s=a[0:90][1:a[-1]]".into()),
        ("g2_update_trap_first", ":s=b(\"abcd\")\n(s):s=a[-1:90][1:a[9]]".into()),
        ("g2_byte_trap_first", ":s=b(\"abcd\")\n(s):s=a[0:256][1:a[-1]]".into()),
        ("g2_quota_before_read", ":s=b(!bytes(33554400,97))\n(s):s=a[0:90][1:a[-1]]".into()),
        ("g2_division_barrier", ":s=b(\"abcd\")\n(s):s=a[0:90][1:1/0+a[0]]".into()),
        ("g2_effect_barrier", ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a)]\n(s)=ignored=!out(\"middle\");a[0]".into()),
        ("g2_unused_lazy_effect", ":s=b(\"abcd\")\n(s):s=a[0:90][1:(true?(ignored=!out(\"middle\");a[0]):a[1])]".into()),
        ("g2_unselected_lazy_effect", ":s=b(\"abcd\")\n(s):s=a[0:90][1:(false?(ignored=!out(\"BAD\");a[-1]):a[1])]".into()),
        ("g2_allocation_barrier", ":s=b(\"abcd\")\n(s):s=a[0:90][1:(ignored=#!bytes(67108824,0);a[0])]".into()),
        ("g2_live_alias", ":s=old=\"abcd\";new=b(old);!concat(new,old)\n(s):s=a[0:90][1:a[2]]".into()),
        ("g2_returned_alias", ":s=old=c(\"abcd\");new=b(old);!concat(new,old)\n(s):s=a[0:90][1:a[2]]\n(s):s=a".into()),
        ("g2_record_child", ":s=old=Cell(\"abcd\",0);new=b(old.text);!concat(new,old.text)\n(s):s=a[0:90][1:a[2]]".into()),
        ("g2_map_value_alias", ":s=old=!put(!bytemap(),\"x\",\"abcd\");new=b(!get(old,\"x\"));!concat(new,!get(old,\"x\"))\n(s):s=a[0:90][1:a[2]]".into()),
        ("g2_recursive_read", ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,3)]\n(s,i)=b==0?a[0]:c(a,b-1)".into()),
        ("g2_mutual_read", ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,3)]\n(s,i)=b==0?a[0]:d(a,b-1)\n(s,i)=b==0?a[0]:c(a,b-1)".into()),
        ("g2_replay_depth_fallback", ":s=b(\"abcd\")\n(s):s=a[0:90][1:c(a,20)]\n(s,i)=b==0?a[0]:c(a,b-1)".into()),
        ("g2_loop_state", ":s=@(\"abcd\",0;b<3;b==1?a[0:90][1:b(a)]:a,b+1;a)\n(s)=a[2]".into()),
        ("g2_loop_conditional", ":s=@(\"abcd\",0;b<3;a[0:90][1:(a[0]==97?a[2]:a[3])],b+1;a)".into()),
        ("g2_zero_slot_argument", ":s=b(\"abcd\",Zero([]))\n(s,Zero):s=a[0:90][1:c(a,b)]\n(s,Zero)=a[2]+#b.empty".into()),
        ("g2_empty_update", ":s=b(\"\")\n(s):s=a[0:90][1:a[0]]".into()),
        ("g2_multiple_update_quota_barrier", ":s=b(!bytes(22369590,97))\n(s):s=a[0:90][1:a[2]][2:a[0]]".into()),
    ]
}
