//! Differential fuzz families for integer-keyed sets (ADR 051 part 2).
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
pub const FAMILY_COUNT: usize = 14;

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
    ]
}
