//! Differential fuzz families for sum types, recursive values and matching
//! (ADR 050). Each case is a whole program, because sums need a declaration
//! group, and uses the application entry signature `(v,s,s,s)` so the existing
//! reference/native comparison applies unchanged.
//!
//! Values derive from `#a` and `#b`, which are total, so no family depends on
//! a non-empty input.
pub const FAMILY_COUNT: usize = 22;

pub fn cases(seed: u64) -> Vec<(&'static str, String)> {
    let k = seed % 7;
    // Loop-built list length. Kept well under the 256-frame call depth so the
    // deep family exercises node volume, not the depth guard.
    let deep = 200 + seed % 64;
    let rows = vec![
        // Every variant shape: nullary only, one slot, several slots, a
        // sum-typed slot, and a wide tag range.
        (
            "sum_nullary_only",
            format!(
                "sum S(A,B,C)\nsum T(Z)\n(v,s,s,s)=b(#a%3==0?A:#a%3==1?B:C)+{k}\n(S)=?a(A:1,B:2,C:3)"
            ),
        ),
        (
            "sum_one_slot",
            format!("sum S(P(i),Q)\n(v,s,s,s)=b(P(#a+{k}))\n(S)=?a(P(v):v,Q:-1)"),
        ),
        (
            "sum_many_slots",
            format!(
                "sum S(P(i,i,i,i),Q)\n(v,s,s,s)=b(P(#a,#b,#c,{k}))\n(S)=?a(P(w,x,y,z):w*1000+x*100+y*10+z,Q:-1)"
            ),
        ),
        (
            "sum_sum_slot",
            format!(
                "sum S(W(S),P(i),Q)\n(v,s,s,s)=b(W(W(P(#a+{k}))))\n(S)=?a(W(x):?x(W(y):?y(W(_):1,P(v):v,Q:2),P(v):v*10,Q:3),P(v):v*100,Q:4)"
            ),
        ),
        (
            "sum_wide_tags",
            format!(
                "sum S(V0,V1,V2,V3,V4,V5,V6,V7,V8,V9)\n(v,s,s,s)=b(c(#a+{k}))\n(S)=?a(V0:0,V1:1,V2:2,V3:3,V4:4,V5:5,V6:6,V7:7,V8:8,V9:9)\n1:S=a%10==0?V0:a%10==1?V1:a%10==2?V2:a%10==3?V3:a%10==4?V4:a%10==5?V5:a%10==6?V6:a%10==7?V7:a%10==8?V8:V9"
            ),
        ),
        // Recursive values.
        (
            "sum_recursive_list_fold",
            format!(
                "sum L(C(i,L),E)\n(v,s,s,s)=c(b(#a+{k}))\n1:L=a<=0?E:C(a,b(a-1))\n(L)=?a(C(h,t):h+c(t),E:0)"
            ),
        ),
        (
            "sum_recursive_tree_height",
            format!(
                "sum T(N(i,T,T),L)\n(v,s,s,s)=c(b(#a%6+{k}%4))\n1:T=a<=0?L:N(a,b(a-1),b(a-2))\n(T)=?a(N(_,l,r):1+!max(c(l),c(r)),L:0)"
            ),
        ),
        (
            "sum_recursive_mutual",
            format!(
                "sum A(MkA(B),EndA)\nsum B(MkB(A),EndB)\n(v,s,s,s)=c(b(#a%8+{k}))\n1:A=a<=0?EndA:MkA(MkB(b(a-1)))\n(A)=?a(MkA(x):1+?x(MkB(y):c(y),EndB:0),EndA:0)"
            ),
        ),
        (
            "sum_deep_list_loop",
            format!(
                "sum L(C(i,L),E)\n(v,s,s,s)=c(b({deep}+#a))\n1:L=@(E,a;b>0;C(b,a),b-1;a)\n(L)=@(a,0;d(a);e(a),b+f(a);b)\n(L):b=?a(C(_,_):true,E:false)\n(L):L=?a(C(_,t):t,E:E)\n(L)=?a(C(h,_):h,E:0)"
            ),
        ),
        // Aliasing: an older value must never be observed to change when a new
        // parent is built over the same child.
        (
            "sum_alias_old_value",
            format!(
                "sum L(C(i,L),E)\n(v,s,s,s)=d(C(#a+{k},E))\n(L)=?a(C(h,_):h,E:-1)\n(L)=?a(C(_,t):?t(C(h,_):h,E:0),E:-1)\n(L)=b(a)*1000+c(C(9,a))*100+b(C(7,a))*10+b(a)"
            ),
        ),
        (
            "sum_alias_rebuild_chain",
            format!(
                "sum L(C(i,L),E)\n(v,s,s,s)=e(d(c(b(#a+{k}))))\n1:L=C(a,E)\n(L):L=C(100,a)\n(L):L=C(200,a)\n(L)=?a(C(h,t):h+?t(C(x,y):x+?y(C(z,_):z,E:0),E:0),E:-1)"
            ),
        ),
        (
            "sum_reuse_rebuild_loop",
            format!(
                "sum S(P(i),Q)\n(v,s,s,s)=@(Q,0,#a%16+{k};b<c;b(a),b+1,c;c(a))\n(S):S=?a(P(v):P(v+1),Q:P(0))\n(S)=?a(P(v):v,Q:-1)"
            ),
        ),
        // Lazy arms: an unselected arm must execute nothing.
        (
            "sum_lazy_unselected_trap",
            format!("sum S(P(i),Q)\n(v,s,s,s)=b(Q)+{k}\n(S)=?a(P(v):v/0,Q:5)"),
        ),
        (
            "sum_lazy_selected_trap",
            format!("sum S(P(i),Q)\n(v,s,s,s)=b(P(#a+{k}))\n(S)=?a(P(v):v/(#\"\"),Q:5)"),
        ),
        (
            "sum_lazy_unselected_effect",
            format!(
                "sum S(P(i),Q)\n(v,s,s,s)=b(#a%2==0?Q:P({k}))\n(S)=?a(P(v):!out(\"ARM\")+v,Q:5)"
            ),
        ),
        (
            "sum_lazy_unselected_allocation",
            format!("sum S(P(i),Q)\n(v,s,s,s)=b(Q)+{k}\n(S)=?a(P(v):#!buffer(v,0),Q:5)"),
        ),
        // Option-shaped chains without sentinels.
        (
            "sum_option_chain",
            format!(
                "sum O(None,Some(i))\n(v,s,s,s)=d(c(b(#a+{k})))\n1:O=a%3==0?None:Some(a)\n(O):O=?a(Some(v):Some(v*2),None:None)\n(O)=?a(Some(v):v,None:-1)"
            ),
        ),
        (
            "sum_option_nested",
            format!(
                "sum O(None,Some(i))\n(v,s,s,s)=c(b(#a+{k}))+c(b(#a+{k}+1))*10\n1:O=a%4==0?None:Some(a)\n(O)=?a(Some(v):v,None:-1)"
            ),
        ),
        // Sums inside records, and nested updates through an alias.
        (
            "sum_record_field",
            format!(
                "sum O(None,Some(i))\nrecord W(tag:O,n:i)\n(v,s,s,s)=c(W(Some(#a+{k}),#b))\n(W)=(x=a.tag;?x(Some(v):v,None:-1))\n(W)=b(a)*100+b(a{{tag:None}})*10+b(a)"
            ),
        ),
        (
            "sum_record_nested_alias",
            format!(
                "sum O(None,Some(i))\nrecord W(tag:O,n:i)\nrecord V(inner:W,m:i)\n(v,s,s,s)=d(V(W(Some(#a+{k}),1),2))\n(W)=(x=a.tag;?x(Some(v):v,None:-1))\n(V)=b(a.inner)\n(V)=c(a)*1000+c(a{{inner:a.inner{{tag:None}}}})*10+c(a)"
            ),
        ),
        // A sum as loop state, and matching in a loop condition.
        (
            "sum_loop_state",
            format!(
                "sum S(P(i),Q)\n(v,s,s,s)=@(Q,0,#a%12+{k}%3+1;b<c;b(a),b+1,c;c(a))\n(S):S=?a(P(v):P(v+2),Q:P(1))\n(S)=?a(P(v):v,Q:-1)"
            ),
        ),
        (
            "sum_match_condition",
            format!(
                "sum L(C(i,L),E)\n(v,s,s,s)=@(c(#a%20+{k}),0;b(a);d(a),b+1;b)\n(L):b=?a(C(_,_):true,E:false)\n1:L=a<=0?E:C(a,c(a-1))\n(L):L=?a(C(_,t):t,E:E)"
            ),
        ),
    ];
    assert_eq!(rows.len(), FAMILY_COUNT, "sum family count mismatch");
    rows
}
