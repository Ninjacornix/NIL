//! Differential fuzz families for integer-keyed sets and maps (ADR 051
//! part 2).
//!
//! The Round 28 audit found the set shipped with no differential families at
//! all, so these cover what its unit tests assert plus the combinations unit
//! tests do not reach: sets held in records and loop state, bulk against
//! incremental construction, and removal interleaved with membership.
//!
//! Each case is a whole program using the application entry signature
//! `(v,s,s,s)`, so the existing reference/native comparison at O0 and O2
//! applies unchanged. Values derive from `#a`, which is total, so no family
//! depends on a non-empty input.
pub const FAMILY_COUNT: usize = 40;

pub fn cases(seed: u64) -> Vec<(&'static str, String)> {
    let k = (seed % 7) as i64;
    // Loop bound, kept small so a family exercises key behaviour rather than
    // the allocation guard.
    let n = 8 + (seed % 24) as i64;
    // A key that must not collide with the small keys above once encoded.
    let far = match seed % 4 {
        0 => "-9223372036854775808".to_string(),
        1 => "9223372036854775807".to_string(),
        2 => format!("-{}", 1 + seed % 97),
        _ => format!("{}", 1_000_000_007u64 + seed),
    };
    vec![
        // Membership over a loop-built set, including a key never inserted.
        (
            "set_membership",
            format!(
                "(v,s,s,s)=@(!set(),0,{n};b<c;!add(a,b*{k}-1),b+1,c;\
                 !size(a)*1000+(!has(a,-1)?100:0)+(!has(a,{far})?10:0)+(!has(a,0)?1:0))\n"
            ),
        ),
        // The extremes of the key range are ordinary keys.
        (
            "set_extreme_keys",
            "(v,s,s,s)=(s=!add(!add(!add(!set(),-9223372036854775808),9223372036854775807),0);\
             !size(s)*100+(!has(s,-9223372036854775808)?10:0)+(!has(s,9223372036854775807)?1:0))\n"
                .to_string(),
        ),
        // Negatives must not alias positives through the key encoding.
        (
            "set_sign_separation",
            format!(
                "(v,s,s,s)=@(!set(),1,{n};b<c;!add(!add(a,b),0-b),b+1,c;\
                 !size(a)*10+(!has(a,0-({n}-1))?1:0))\n"
            ),
        ),
        // Re-adding is idempotent and does not disturb size.
        (
            "set_readd_idempotent",
            format!("(v,s,s,s)=@(!set(),0,{n};b<c;!add(!add(a,b%3),b%3),b+1,c;!size(a))\n"),
        ),
        // Duplicates in a loop: size counts distinct keys only.
        (
            "set_duplicate_keys",
            format!("(v,s,s,s)=@(!set(),0,{n};b<c;!add(a,(b*{k})%5),b+1,c;!size(a))\n"),
        ),
        // Removal of present, absent and repeated keys.
        (
            "set_removal",
            format!(
                "(v,s,s,s)=@(!set(),0,{n};b<c;!add(a,b),b+1,c;\
                 (t=!remove(!remove(!remove(a,0),0),{far});\
                 !size(a)*1000+!size(t)*10+(!has(t,0)?1:0)))\n"
            ),
        ),
        // Removal in a loop, draining the set.
        (
            "set_removal_drain",
            format!(
                "(v,s,s,s)=(s=@(!set(),0,{n};b<c;!add(a,b),b+1,c;a);\
                 @(s,0,{n};b<c;!remove(a,b*2),b+1,c;!size(a)))\n"
            ),
        ),
        // Bulk construction must agree with the incremental build.
        (
            "set_bulk_vs_incremental",
            format!(
                "(v,s,s,s)=(q=@(!buffer(0,0),0,{n};b<c;!concat(a,!buffer(1,(b*{k})%7)),b+1,c;a);\
                 !size(!set(q))*100+!size(@(!set(),0,{n};b<c;!add(a,(b*{k})%7),b+1,c;a)))\n"
            ),
        ),
        // Bulk construction over the program input, whose length is unknown
        // statically, including the empty case.
        (
            "set_bulk_from_input",
            "(v,s,s,s)=!size(!set(a))*10+(#a>0?(!has(!set(a),a[0])?1:0):0)\n".to_string(),
        ),
        // An empty source is an empty set, not a fault.
        (
            "set_bulk_empty",
            format!(
                "(v,s,s,s)=(e=!set(!buffer(0,0));\
                 !size(e)*10+(!has(e,{k})?1:0))\n"
            ),
        ),
        // Iteration order is first occurrence: draining in insertion order
        // must empty the set built from a duplicate-bearing sequence.
        (
            "set_first_occurrence_order",
            format!(
                "(v,s,s,s)=(q=@(!buffer(0,0),0,{n};b<c;!concat(a,!buffer(1,b%4)),b+1,c;a);\
                 s=!set(q);@(s,0,4;b<c;!remove(a,b),b+1,c;!size(a)))\n"
            ),
        ),
        // A set is an immutable value: the original survives add and remove.
        (
            "set_alias_immutable",
            format!(
                "(v,s,s,s)=(s=!add(!add(!set(),{k}),{k}+1);\
                 u=!add(s,{far});w=!remove(s,{k});\
                 !size(s)*100+!size(u)*10+!size(w))\n"
            ),
        ),
        // A set inside loop state, carried across iterations beside other
        // values, so its root is not the only live one.
        (
            "set_in_loop_state",
            format!(
                "(v,s,s,s)=@(!set(),!set(),0;c<{n};!add(a,c),!add(b,c*2),c+1;\
                 !size(a)*100+!size(b)*10+(!has(a,0)?1:0))\n"
            ),
        ),
        // A set stored in a record field, projected back out and updated, so
        // the set is reached through a parent row.
        (
            "set_in_record",
            format!(
                "record Seen(keys:k,count:i)\n\
                 (v,s,s,s)=(x=Seen(!add(!set(),{k}),1);\
                 y=Seen(!add(x.keys,{far}),2);\
                 !size(x.keys)*100+!size(y.keys)*10+(!has(y.keys,{far})?1:0))\n"
            ),
        ),
        // --- text toolkit (ADR 052) ---
        // Empty vector encoding, which failed E016 before step 0.
        (
            "text_parse_empty_vector",
            "(v,s,s,s)=#!parsebuf(\"0 \\n0 \\n\",\" \\n\")*10+#!parsebuf(\"\\n\\n\",\" \\n\")\n".to_string(),
        ),
        // Separator runs, leading and trailing separators.
        (
            "text_parse_separator_runs",
            format!(
                "(v,s,s,s)=#!parsebuf(\" 1  2   3 \",\" \")*100+#!parsebuf(\"{}\",\" \")*10+#!parsebuf(\"   \",\" \")\n",
                1 + seed % 9
            ),
        ),
        // Multi-byte separator sets, including a non-ASCII separator byte.
        (
            "text_parse_multibyte_separators",
            "(v,s,s,s)=#!parsebuf(\"1,2;3\",\",;\")*10+#!parsebuf(\"1\\xFF2\",\"\\xFF\")\n"
                .to_string(),
        ),
        // Row join round-trips through the parser.
        (
            "text_row_roundtrip",
            format!(
                "(v,s,s,s)=#!parsebuf(!format(@(!buffer(0,0),0;b<{n};!concat(a,!buffer(1,b*{k}-1)),b+1;a),\" \",\"\\n\"),\" \\n\")\n"
            ),
        ),
        // Empty row, empty separator and the extremes of the value range.
        (
            "text_row_edges",
            "(v,s,s,s)=#!format(!buffer(0,0),\", \")*100+#!format(!buffer(1,-9223372036854775808),\" \")+#!format(!buffer(2,7),\"\")\n"
                .to_string(),
        ),
        // A very long row: one allocation, so this must not be quadratic.
        (
            "text_row_long",
            format!(
                "(v,s,s,s)=#!format(@(!buffer(0,0),0;b<{};!concat(a,!buffer(1,b)),b+1;a),\" \",\"\\n\")\n",
                4096 + seed % 512
            ),
        ),
        // --- sequence reductions (ADR 055 step 3) ---
        // A reduction must agree with the loop it replaces.
        (
            "reduce_matches_loop",
            format!(
                "(v,s,s,s)=(q=@(!buffer(0,0),0;b<{n};!concat(a,!buffer(1,b*{k}-3)),b+1;a);\
                 (!min(q)-@(q,q[0],1;c<#a;a,a[c]<b?a[c]:b,c+1;b))*100+\
                 (!max(q)-@(q,q[0],1;c<#a;a,a[c]>b?a[c]:b,c+1;b))*10+\
                 (!sum(q)-@(q,0,0;c<#a;a,b+a[c],c+1;b)))\n"
            ),
        ),
        // Empty sums to zero; empty min faults, so both outcomes are compared.
        (
            "reduce_empty_sum",
            "(v,s,s,s)=!sum(!buffer(0,0))+!sum(!bytes(0,0))\n".to_string(),
        ),
        (
            "reduce_empty_min_faults",
            "(v,s,s,s)=!min(!buffer(0,0))\n".to_string(),
        ),
        // The extremes of the value range are ordinary elements.
        (
            "reduce_extremes",
            "(v,s,s,s)=(q=!parsebuf(\"-9223372036854775808 0 9223372036854775807\",\" \");\
             (!min(q)==0-9223372036854775807-1?1:0)*10+(!max(q)==9223372036854775807?1:0))\n"
                .to_string(),
        ),
        // Addition wraps, so the sum of the extremes is specified, not a fault.
        (
            "reduce_sum_wraps",
            "(v,s,s,s)=(!sum(!parsebuf(\"9223372036854775807 1\",\" \"))==0-9223372036854775807-1?1:0)*10+\
             (!sum(!parsebuf(\"-9223372036854775808 -1\",\" \"))==9223372036854775807?1:0)\n"
                .to_string(),
        ),
        // Bytes reduce too, and the sequence survives the reduction unchanged.
        (
            "reduce_bytes_and_alias",
            format!(
                "(v,s,s,s)=(q=!bytes({},7);r=!min(q)*1000+!max(q)*100+!sum(q);\
                 r+#q-{}+!sum(q)-!sum(q))\n",
                1 + seed % 16,
                1 + seed % 16
            ),
        ),
        // --- splitting (ADR 052 part 1a) ---
        // Empty input is one empty field, and all-separator input is n+1.
        (
            "split_empty_and_all_separators",
            "(v,s,s,s)=#!split(\"\",\"\\n\")*100+#!split(\"\\n\\n\",\"\\n\")*10+#!split(\"abc\",\"\\n\")\n"
                .to_string(),
        ),
        // Leading, trailing and interior separator runs are all kept.
        (
            "split_separator_runs",
            "(v,s,s,s)=#!split(\"\\nx\",\"\\n\")*100+#!split(\"x\\n\",\"\\n\")*10+#!split(\"a\\n\\nb\",\"\\n\")\n"
                .to_string(),
        ),
        // A separator set with several bytes, including a high byte.
        (
            "split_multibyte_separators",
            "(v,s,s,s)=#!split(\"a,b;c\",\",;\")*10+#!split(\"1\\xFF2\",\"\\xFF\")\n".to_string(),
        ),
        // Fields recovered by !slice must reconstruct the input exactly.
        (
            "split_slice_roundtrip",
            "(v,s,s,s)=(t=!split(b,\"\\n\");@(t,0,0;c<#a/2;a,b+a[2*c+1],c+1;b)+#t/2-1-#b)\n"
                .to_string(),
        ),
        // Split then join then split again agrees on the field count.
        (
            "split_join_roundtrip",
            format!(
                "(v,s,s,s)=(q=@(!buffer(0,0),0;b<{n};!concat(a,!buffer(1,b*{k})),b+1;a);#!split(!format(q,\" \"),\" \")-#q)\n"
            ),
        ),
        // A long input with many fields: one allocation, never quadratic.
        (
            "split_long_input",
            format!(
                "(v,s,s,s)=#!split(!format(@(!buffer(0,0),0;b<{};!concat(a,!buffer(1,b)),b+1;a),\" \"),\" \")\n",
                2048 + seed % 256
            ),
        ),
        // --- integer-keyed map (!imap) ---
        // Lookup over a loop-built index map.
        (
            "imap_lookup",
            format!(
                "(v,s,s,s)=@(!imap(),0,{n};b<c;!put(a,b*{k}-1,b),b+1,c;\
                 !size(a)*100+!get(a,0-1)*10+(!has(a,{far})?1:0))\n"
            ),
        ),
        // The extremes are ordinary keys and must not collide.
        (
            "imap_extreme_keys",
            "(v,s,s,s)=(m=!put(!put(!put(!imap(),-9223372036854775808,1),9223372036854775807,2),0,3);\
             !size(m)*100+!get(m,-9223372036854775808)*10+!get(m,9223372036854775807))\n"
                .to_string(),
        ),
        // Negatives must not alias positives through the key encoding.
        (
            "imap_sign_separation",
            format!(
                "(v,s,s,s)=@(!imap(),1,{n};b<c;!put(!put(a,b,b),0-b,0-b),b+1,c;\
                 !size(a)*10+(!get(a,0-1)==0-1?1:0))\n"
            ),
        ),
        // A repeated key keeps its position and takes the later value.
        (
            "imap_repeated_key",
            format!(
                "(v,s,s,s)=@(!imap(),0,{n};b<c;!put(a,b%3,b),b+1,c;\
                 !size(a)*100+!get(a,0)*10+!key(a,0))\n"
            ),
        ),
        // Iteration is insertion order, read back through !key.
        (
            "imap_iteration_order",
            format!(
                "(v,s,s,s)=(m=@(!imap(),0,{n};b<c;!put(a,{n}-b,b),b+1,c;a);\
                 @(m,0,0;c<!size(a);a,b*3+!key(a,c)%3,c+1;b))\n"
            ),
        ),
        // Removal of present, absent and repeated keys, order preserved.
        (
            "imap_removal",
            format!(
                "(v,s,s,s)=(m=@(!imap(),0,{n};b<c;!put(a,b,b*2),b+1,c;a);\
                 t=!remove(!remove(!remove(m,0),0),{far});\
                 !size(m)*1000+!size(t)*10+!key(t,0))\n"
            ),
        ),
        // A map is an immutable value: the original survives put and remove.
        (
            "imap_alias_immutable",
            format!(
                "(v,s,s,s)=(m=!put(!put(!imap(),{k},1),{k}+1,2);\
                 u=!put(m,{far},3);w=!remove(m,{k});\
                 !size(m)*100+!size(u)*10+!size(w))\n"
            ),
        ),
        // An integer map inside a record field, reached through a parent row.
        (
            "imap_in_record",
            format!(
                "record Index(byKey:j,count:i)\n\
                 (v,s,s,s)=(x=Index(!put(!imap(),{k},1),1);\
                 y=Index(!put(x.byKey,{far},2),2);\
                 !size(x.byKey)*100+!size(y.byKey)*10+!get(y.byKey,{far}))\n"
            ),
        ),
    ]
}
