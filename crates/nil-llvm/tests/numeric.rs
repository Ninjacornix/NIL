use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
    numeric::{NAN_BITS, canonical_bits, format},
};
use nil_hir::FunctionId;
use nil_llvm::{Optimization, Options};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Executable(PathBuf);
impl Drop for Executable {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn build(source: &str, opt: Optimization) -> (nil_hir::ValidatedProgram, Executable) {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let exe = Executable(std::env::temp_dir().join(format!(
        "nil-numeric-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    nil_llvm::build(
        &p.hir,
        &exe.0,
        Options {
            optimization: opt,
            ..Default::default()
        },
    )
    .unwrap();
    (p.hir, exe)
}
fn parity(source: &str, args: &[&str], values: &[Value], expected: Result<Value, &str>) {
    for opt in [Optimization::O0, Optimization::O2] {
        let (p, exe) = build(source, opt);
        let r = execute_values(&p, FunctionId(0), values, Limits::default());
        let out = Command::new(&exe.0).args(args).output().unwrap();
        match &expected {
            Ok(v) => {
                assert_eq!(r.as_ref().unwrap(), v, "{source}");
                assert!(
                    out.status.success(),
                    "{}",
                    String::from_utf8_lossy(&out.stderr)
                );
                let text = if let Value::Bool(b) = v {
                    b.to_string()
                } else {
                    format(v)
                };
                assert_eq!(
                    out.stdout,
                    format!("{text}\n").as_bytes(),
                    "{source} {opt:?}"
                );
            }
            Err(code) => {
                assert_eq!(r.unwrap_err().code, *code);
                assert!(!out.status.success());
                assert!(
                    String::from_utf8_lossy(&out.stderr).starts_with(code),
                    "{:?}",
                    out.stderr
                );
            }
        }
    }
}
#[test]
fn unsigned_widths_wrap_without_signed_overflow() {
    parity(
        ":u64=18446744073709551615u64+1u64",
        &[],
        &[],
        Ok(Value::U64(0)),
    );
    parity(":u128=0u128-1u128", &[], &[], Ok(Value::U128(u128::MAX)));
    parity(
        ":u64=9223372036854775808u64*2u64",
        &[],
        &[],
        Ok(Value::U64(0)),
    );
    parity(
        ":u128=340282366920938463463374607431768211455u128/2u128",
        &[],
        &[],
        Ok(Value::U128(u128::MAX / 2)),
    );
}
#[test]
fn unsigned_division_zero_retains_integer_trap() {
    parity(":u128=1u128/0u128", &[], &[], Err("E009"));
}
#[test]
fn wide_parameters_cross_calls_and_loop_state() {
    parity(
        "(u128,i):u128=b(a,b)\n(u128,i):u128=@(a,0;b<3;a+1u128,b+1;a)+!u128(b)",
        &["18446744073709551616", "4"],
        &[Value::U128(1u128 << 64), Value::I64(4)],
        Ok(Value::U128((1u128 << 64) + 7)),
    );
}
#[test]
fn unsigned_order_is_not_signed_order() {
    parity(
        ":b=18446744073709551615u64>0u64",
        &[],
        &[],
        Ok(Value::Bool(true)),
    );
}
#[test]
fn truncation_is_explicit_low_bits() {
    parity(
        "=!trunci64(18446744073709551615u128)",
        &[],
        &[],
        Ok(Value::I64(-1)),
    );
    parity(":u64=!truncu64(-1)", &[], &[], Ok(Value::U64(u64::MAX)));
}
#[test]
fn checked_conversions_guard_before_float_casts() {
    for s in [
        "=!i64(9223372036854775808.0)",
        ":u64=!u64(-1)",
        ":u64=!u64(18446744073709551616.0)",
        ":u128=!u128(!parsef64(\"nan\"))",
        "=!i64(!parsef64(\"inf\"))",
        ":u128=!u128(3.402823669209385e38)",
    ] {
        parity(s, &[], &[], Err("E021"));
    }
    parity("=!i64(-3.75)", &[], &[], Ok(Value::I64(-3)));
    parity(":u64=!u64(3.75)", &[], &[], Ok(Value::U64(3)));
    parity(":u128=!u128(-0.0)", &[], &[], Ok(Value::U128(0)));
    for (bits, ty, expected) in [
        (0xc3e0000000000000u64, "i64", Value::I64(i64::MIN)),
        (
            0x43efffffffffffff,
            "u64",
            Value::U64(f64::from_bits(0x43efffffffffffff) as u64),
        ),
        (
            0x47efffffffffffff,
            "u128",
            Value::U128(f64::from_bits(0x47efffffffffffff) as u128),
        ),
    ] {
        let result = match ty {
            "i64" => "",
            "u64" => ":u64",
            _ => ":u128",
        };
        parity(
            &format!("{result}=!{ty}(!floatbits({bits}u64))"),
            &[],
            &[],
            Ok(expected),
        );
    }
}
#[test]
fn float_division_produces_infinity_and_canonical_nan() {
    parity(
        ":f64=1.0/0.0",
        &[],
        &[],
        Ok(Value::F64(f64::INFINITY.to_bits())),
    );
    parity(
        ":f64=-1.0/0.0",
        &[],
        &[],
        Ok(Value::F64(f64::NEG_INFINITY.to_bits())),
    );
    parity(":u64=!bits(0.0/0.0)", &[], &[], Ok(Value::U64(NAN_BITS)));
}
#[test]
fn nan_comparisons_follow_ieee_ordering() {
    parity(
        ":b=!parsef64(\"nan\")==!parsef64(\"nan\")",
        &[],
        &[],
        Ok(Value::Bool(false)),
    );
    parity(
        ":b=!parsef64(\"nan\")!=1.0",
        &[],
        &[],
        Ok(Value::Bool(true)),
    );
    parity(
        ":b=!parsef64(\"nan\")<1.0",
        &[],
        &[],
        Ok(Value::Bool(false)),
    );
}
#[test]
fn signed_zero_is_equal_but_bits_and_division_distinguish_it() {
    parity(":b=-0.0==0.0", &[], &[], Ok(Value::Bool(true)));
    parity(":u64=!bits(-0.0)", &[], &[], Ok(Value::U64(1 << 63)));
    parity(
        ":f64=1.0/-0.0",
        &[],
        &[],
        Ok(Value::F64(f64::NEG_INFINITY.to_bits())),
    );
}
#[test]
fn contraction_is_disabled_for_multiply_then_add() {
    let a = 1.0 + 2f64.powi(-27);
    let b = 1.0 - 2f64.powi(-27);
    assert_ne!(a.mul_add(b, -1.0).to_bits(), 0);
    parity(
        "(f64,f64,f64):u64=!bits(a*b+c)",
        &["1.0000000074505806", "0.9999999925494194", "-1.0"],
        &[
            Value::F64(a.to_bits()),
            Value::F64(b.to_bits()),
            Value::F64((-1.0f64).to_bits()),
        ],
        Ok(Value::U64(0)),
    );
    parity(
        ":u64=!bits(1.0000000074505806*0.9999999925494194-1.0)",
        &[],
        &[],
        Ok(Value::U64(0)),
    );
    let ir = nil_llvm::emit_llvm(
        &compile_with_profile(":f64=1.1*2.2+3.3", SourceProfile::ExprV5)
            .unwrap()
            .hir,
    );
    assert!(ir.contains("fmul double"));
    assert!(ir.contains("fadd double"));
    for flag in [" fast ", " contract ", " reassoc ", " arcp ", "llvm.fma"] {
        assert!(!ir.contains(flag));
    }
}
#[test]
fn numeric_parse_rejects_incomplete_and_noncanonical_text() {
    for s in [
        ":f64=!parsef64(\"1.\")",
        ":f64=!parsef64(\" 1.0\")",
        ":f64=!parsef64(\"1e\")",
        ":f64=!parsef64(\"0x1p0\")",
        ":u64=!parseu64(\"01\")",
        ":u128=!parseu128(\"340282366920938463463374607431768211456\")",
    ] {
        parity(s, &[], &[], Err("E022"));
    }
}
#[test]
fn unsigned_decimal_format_and_parse_round_trip() {
    parity(
        ":u128=!parseu128(!format(340282366920938463463374607431768211455u128))",
        &[],
        &[],
        Ok(Value::U128(u128::MAX)),
    );
    parity(
        ":u64=!parseu64(!format(18446744073709551615u64))",
        &[],
        &[],
        Ok(Value::U64(u64::MAX)),
    );
}
#[test]
fn float_special_and_random_patterns_format_and_round_trip_exactly() {
    let mut bits = vec![
        0,
        1 << 63,
        1,
        0x8000000000000001,
        0x000fffffffffffff,
        0x0010000000000000,
        0x7fefffffffffffff,
        0xffefffffffffffff,
        0x7ff0000000000000,
        0xfff0000000000000,
        NAN_BITS,
        0x7ff0000000000001,
        0xfff8000000001234,
    ];
    let mut state = 5130572u64;
    for _ in 0..256 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        bits.push(state);
    }
    for opt in [Optimization::O0, Optimization::O2] {
        let (_, fmt) = build("(u64):s=!format(!floatbits(a))", opt);
        let (_, round) = build("(u64):u64=!bits(!parsef64(!format(!floatbits(a))))", opt);
        for b in &bits {
            let canonical = canonical_bits(f64::from_bits(*b));
            let out = Command::new(&fmt.0).arg(b.to_string()).output().unwrap();
            assert!(out.status.success());
            assert_eq!(
                out.stdout,
                format!("{}\n", format(&Value::F64(canonical))).as_bytes(),
                "bits={b:016x} {opt:?}"
            );
            let out = Command::new(&round.0).arg(b.to_string()).output().unwrap();
            assert!(out.status.success());
            assert_eq!(
                String::from_utf8(out.stdout).unwrap().trim(),
                canonical.to_string(),
                "bits={b:016x}"
            );
        }
    }
}
#[test]
fn float_conversion_and_lazy_failures_preserve_order() {
    parity(
        ":f64=false?!f64(!i64(!parsef64(\"nan\"))):2.0",
        &[],
        &[],
        Ok(Value::F64(2f64.to_bits())),
    );
    parity("=!i64(!parsef64(\"bad\"))/0", &[], &[], Err("E022"));
}

#[test]
fn decimal_halfway_rounding_and_extreme_exponents_match_bits() {
    for text in [
        "1.00000000000000011102230246251565404236316680908203125",
        "1.00000000000000033306690738754696212708950042724609375",
        "2.2250738585072014e-308",
        "4.9406564584124654e-324",
        "1.7976931348623157e308",
        "-1e-9999",
        "1e9999",
    ] {
        let bits = canonical_bits(nil_compiler::numeric::parse_float(text).unwrap());
        parity(
            &format!(":u64=!bits(!parsef64(\"{text}\"))"),
            &[],
            &[],
            Ok(Value::U64(bits)),
        );
    }
}
#[test]
fn float_cli_parameters_preserve_bits_and_wide_following_arguments() {
    parity(
        "(f64,u128,i):u64=!bits(a+!f64(b)+!f64(c))",
        &["-0.0", "0", "0"],
        &[Value::F64(1 << 63), Value::U128(0), Value::I64(0)],
        Ok(Value::U64(0)),
    );
    parity(
        "(f64):u64=!bits(a)",
        &["nan"],
        &[Value::F64(NAN_BITS)],
        Ok(Value::U64(NAN_BITS)),
    );
}
#[test]
fn numeric_formatting_obeys_live_quota() {
    parity(
        "=b(!bytes(67108864,0))\n(s)=#!format(1.0)+#a",
        &[],
        &[],
        Err("E013"),
    );
}
#[test]
fn numeric_conversion_failures_do_not_expose_unselected_host_effects() {
    parity(
        ":u64=!bits(false?b():1.0)\n:f64=!f64(!out(\"BAD\"))",
        &[],
        &[],
        Ok(Value::U64(1f64.to_bits())),
    );
}

#[test]
fn random_float_arithmetic_matches_canonical_reference_bits() {
    let mut state = 8675309u64;
    let mut pairs = Vec::new();
    for _ in 0..128 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let a = state;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        pairs.push((a, state));
    }
    for opt in [Optimization::O0, Optimization::O2] {
        for op in ["+", "-", "*", "/"] {
            let source = format!("(u64,u64):u64=!bits(!floatbits(a){op}!floatbits(b))");
            let (p, exe) = build(&source, opt);
            for (a, b) in &pairs {
                let expected = execute_values(
                    &p,
                    FunctionId(0),
                    &[Value::U64(*a), Value::U64(*b)],
                    Limits::default(),
                )
                .unwrap();
                let out = Command::new(&exe.0)
                    .args([a.to_string(), b.to_string()])
                    .output()
                    .unwrap();
                assert!(out.status.success());
                assert_eq!(
                    out.stdout,
                    format!("{}\n", format(&expected)).as_bytes(),
                    "{op} {a:x} {b:x} {opt:?}"
                );
            }
        }
    }
}
