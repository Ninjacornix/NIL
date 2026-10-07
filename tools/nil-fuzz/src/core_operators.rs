//! Full boundary matrix for ADR 042, including observable host effects.
use crate::generate::Random;
pub const FAMILY_COUNT: usize = 167;
pub fn cases(seed: u64) -> Vec<(&'static str, String)> {
    let mut r = Random(seed);
    let value = r.integer();
    let x = r.next_u64();
    let mut out = vec![
        ("operator_rem_i64", format!("={value}%3")),
        ("operator_rem_min", "=i64min%-1".into()),
        ("operator_rem_zero_i64", "=i64max%0".into()),
        ("operator_rem_u64", ":u64=u64max%2u64".into()),
        ("operator_rem_zero_u64", ":u64=1u64%0u64".into()),
        ("operator_rem_u128", ":u128=u128max%2u128".into()),
        ("operator_rem_zero_u128", ":u128=1u128%0u128".into()),
        (
            "operator_bits_i64",
            format!("=((~i64min)&i64max)^({value}|-1)"),
        ),
        (
            "operator_bits_u64",
            format!(":u64=((~0u64)&u64max)^({x}u64|1u64)"),
        ),
        (
            "operator_bits_u128",
            format!(":u128=((~0u128)&u128max)^({x}u128|1u128)"),
        ),
    ];
    for (name, prefix, lhs, width) in [
        ("operator_shift_i64", "=", "i64min", 64),
        ("operator_shift_u64", ":u64=", "u64max", 64),
        ("operator_shift_u128", ":u128=", "u128max", 128),
    ] {
        for op in ["<<", ">>"] {
            for n in [0, 1, width - 1, -1, width, width + 1, i64::MAX, i64::MIN] {
                out.push((name, format!("{prefix}{lhs}{op}{n}")));
            }
        }
    }
    for n in [i64::MIN, i64::MIN + 1, -1, 0, i64::MAX] {
        out.push(("operator_abs_i64", format!("=!abs({n})")));
    }
    out.extend([
        ("operator_abs_u64", ":u64=!abs(u64max)".into()),
        ("operator_abs_u128", ":u128=!abs(u128max)".into()),
    ]);
    let special = [
        0,
        1 << 63,
        1,
        0x8000000000000001,
        0x0010000000000000,
        0x7fefffffffffffff,
        0x7ff0000000000000,
        0xfff0000000000000,
        0x7ff8000000000000,
        0xfff8000000000000,
        0x3fe0000000000000,
        0xbfe0000000000000,
        r.next_u64(),
    ];
    for (name, op) in [
        ("operator_abs_bits", "abs"),
        ("operator_sqrt_bits", "sqrt"),
        ("operator_floor_bits", "floor"),
        ("operator_ceil_bits", "ceil"),
        ("operator_trunc_bits", "trunc"),
        ("operator_round_bits", "round"),
    ] {
        for b in special {
            out.push((name, format!(":u64=!bits(!{op}(!floatbits(0x{b:x}u64)))")));
        }
    }
    for (name, op) in [("operator_min_bits", "min"), ("operator_max_bits", "max")] {
        for (a, b) in [
            ("nan", "1.0"),
            ("1.0", "nan"),
            ("0.0", "-0.0"),
            ("-0.0", "0.0"),
        ] {
            out.push((name, format!(":u64=!bits(!{op}({a},{b}))")));
        }
    }
    out.extend([
  ("operator_short_trap",":b=false&&(1/0>0)".into()),
  ("operator_short_shift",":b=true||(1<<64>0)".into()),
  ("operator_short_quota",":b=false&&#!bytes(67108864,0)>0".into()),
  ("operator_short_quota_or",":b=true||#!bytes(67108864,0)>0".into()),
  ("operator_short_effect","=!out(\"L\")+(false&&(!out(\"bad\")>0)?1:0)".into()),
  ("operator_selected_effect","=!out(\"L\")+(true&&(!out(\"R\")>0)?1:0)".into()),
  ("binding_effect_order","=first=!out(\"A\");second=!out(\"B\");first+first+second".into()),
  ("binding_lazy",format!("=true?(x={value};x):(bad=!out(\"bad\");crash=1/0;bad+crash)")),
  ("binding_loop_shadow",format!("=x={value};@(x,0;b<7;x=a+1;x,b+1;a)+x")),
  ("binding_each_shadow","=!each(a,0;b=b+1;c+b;a)".into()),
  ("binding_live_alias",format!("=original=a;updated=(old=original[0];original[0:old+{value}]);original[1]+updated[0]")),
  ("binding_call_alias",format!("=original=a;updated=b(a);original[0]+updated[0]\n(v):v=old=a[0];a[0:old+{value}]")),
  ("binding_lazy_alias",":s=original=b;updated=true?(old=original[0];original[0:old^1]):original;!concat(original,updated)".into()),
  ("binding_live_quota","=old=!bytes(40000000,0);extra=!bytes(40000000,0);#old+#extra".into()),
  ("binding_zero_record","=zero=Zero([]);entries=!buffer[Zero](2,zero);keep=entries[0];#!buffer[Zero](3,keep)".into()),
  ("binding_heap_swap","=(b(!buffer(4,0)[0:2][1:9][2:4][3:1],0,3))[0]\n(v,i,i):v=@(a,b,c;2*b+1<c?(a[b]<a[c(a,b,c)]):false;child=c(a,b,c);a[b:a[child]][child:a[b]],child,c;a)\n(v,i,i)=2*b+2<c?(a[2*b+1]>a[2*b+2]?2*b+1:2*b+2):2*b+1".into()),
 ]);
    assert_eq!(out.len(), FAMILY_COUNT);
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundary_matrix_count_and_every_generated_program_typecheck() {
        for (name, suffix) in cases(2042) {
            let source = format!("record Zero(empty:0)\n(v,s,s,s){suffix}");
            nil_compiler::compile_with_profile(&source, nil_compiler::SourceProfile::ExprV5)
                .unwrap_or_else(|d| panic!("{name}: {d}: {source}"));
        }
    }
}
