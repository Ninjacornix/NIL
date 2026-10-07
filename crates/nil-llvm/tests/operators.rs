use nil_compiler::{
    SourceProfile,
    application::Host,
    compile_with_profile,
    evaluator::{Limits, Value, execute_values_with_host},
    numeric::{canonical_bits, format},
};
use nil_hir::{Diagnostic, FunctionId};
use nil_llvm::{Instrumentation, Optimization, Options};
#[derive(Default)]
struct MemoryHost {
    stdout: Vec<u8>,
}
impl Host for MemoryHost {
    fn out(&mut self, b: &[u8]) -> Result<(), Diagnostic> {
        self.stdout.extend_from_slice(b);
        Ok(())
    }
}
fn text(v: &Value) -> Vec<u8> {
    match v {
        Value::Bool(v) => v.to_string().into_bytes(),
        Value::Bytes(v) => v.as_ref().to_vec(),
        Value::Array(v) => format!(
            "[{}]",
            v.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        )
        .into_bytes(),
        Value::Buffer(v) => format!(
            "[{}]",
            v.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        )
        .into_bytes(),
        _ => format(v).into_bytes(),
    }
}
fn parity(source: &str, expected: Result<Value, &str>, effects: &[u8]) {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let mut host = MemoryHost::default();
    let r = execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), &mut host);
    let diagnostic = r.as_ref().err().cloned();
    assert_eq!(host.stdout, effects, "{source}");
    match &expected {
        Ok(v) => assert_eq!(r.as_ref().unwrap(), v, "{source}"),
        Err(code) => assert_eq!(diagnostic.as_ref().unwrap().code, *code, "{source}"),
    }
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &p.hir,
            Options {
                optimization,
                instrumentation: Instrumentation::Bounded,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        match &expected {
            Ok(v) => {
                assert!(
                    out.status.success(),
                    "{source} {optimization:?}: {:?}",
                    out.stderr
                );
                assert!(out.stderr.is_empty());
                assert_eq!(
                    out.stdout,
                    [effects, text(v).as_slice(), b"\n"].concat(),
                    "{source} {optimization:?}"
                );
            }
            Err(code) => {
                assert!(!out.status.success());
                assert_eq!(out.stdout, effects);
                let d = diagnostic.as_ref().unwrap();
                let prefix = if let Some(s) = d.span {
                    format!("{code} @{}..{} ", s.start, s.end)
                } else {
                    format!("{code} ")
                };
                assert!(
                    out.stderr.starts_with(prefix.as_bytes()),
                    "{source}: {:?}",
                    out.stderr
                );
            }
        }
    }
}
#[test]
fn signed_remainder_sign_and_minimum_edge_match_at_o0_o2() {
    parity(
        ":4=[-7%3,7%-3,-7%-3,i64min%-1]",
        Ok(Value::array(vec![-1, 1, -1, 0])),
        b"",
    );
}
#[test]
fn unsigned_remainders_support_both_widths() {
    parity(":u64=u64max%2u64", Ok(Value::U64(1)), b"");
    parity(":u128=u128max%2u128", Ok(Value::U128(1)), b"");
}
#[test]
fn remainder_zero_preserves_effect_and_first_operand_trap_order() {
    parity("=before=!out(\"A\");(1/0)%(2/0)", Err("E009"), b"A");
    for s in ["=7%0", ":u64=7u64%0u64", ":u128=7u128%0u128"] {
        parity(s, Err("E009"), b"");
    }
}
#[test]
fn signed_bit_operations_and_arithmetic_shift_match() {
    parity(
        ":5=[~0,0b1010&0x3,0b1010|0x3,0b1010^0x3,-8>>2]",
        Ok(Value::array(vec![-1, 2, 11, 9, -2])),
        b"",
    );
}
#[test]
fn unsigned_complement_and_shift_edges_match() {
    parity(":u64=(~0u64)>>63", Ok(Value::U64(1)), b"");
    parity(":u128=1u128<<127", Ok(Value::U128(1 << 127)), b"");
    parity(":u64=1u64<<63<<1", Ok(Value::U64(0)), b"");
}
#[test]
fn invalid_shift_counts_trap_before_llvm_can_create_poison() {
    for s in [
        "=1<<-1",
        "=1>>64",
        ":u64=1u64<<64",
        ":u128=1u128>>128",
        ":u128=1u128<<i64min",
    ] {
        parity(s, Err("E021"), b"");
    }
}
#[test]
fn integer_abs_min_max_and_unsigned_identity_match() {
    parity(
        ":4=[!abs(-7),!min(i64min,i64max),!max(i64min,i64max),!abs(0)]",
        Ok(Value::array(vec![7, i64::MIN, i64::MAX, 0])),
        b"",
    );
    parity(
        ":u128=!min(!abs(u128max),0xffffu128)",
        Ok(Value::U128(65535)),
        b"",
    );
}
#[test]
fn abs_minimum_reports_signed_overflow() {
    parity("=!abs(i64min)", Err("E009"), b"");
}
#[test]
fn lazy_logical_operators_do_not_observe_unselected_effects() {
    parity(
        "=(false&&b())?0:(true||b())?42:0\n:b=!out(\"unexpected\")>0",
        Ok(Value::I64(42)),
        b"",
    );
}
#[test]
fn short_circuit_keeps_selected_effect_order() {
    parity(
        "=first=!out(\"A\")==1;first&&(!out(\"B\")==1)?!out(\"C\"):0",
        Ok(Value::I64(1)),
        b"ABC",
    );
}
#[test]
fn lazy_logical_operators_do_not_observe_unselected_traps_or_quota() {
    for s in [":b=false&&(1/0>0)", ":b=false&&#!bytes(67108864,0)>0"] {
        parity(s, Ok(Value::Bool(false)), b"");
    }
    parity(":b=true||(1<<64>0)", Ok(Value::Bool(true)), b"");
}
#[test]
fn local_bindings_execute_once_in_source_order() {
    parity(
        "=first=!out(\"A\");second=!out(\"B\");first+first+second",
        Ok(Value::I64(3)),
        b"AB",
    );
}
#[test]
fn bindings_inside_unselected_lazy_arms_execute_nothing() {
    parity(
        "=true?(x=7;x):(bad=!out(\"bad\");crash=1/0;bad+crash)",
        Ok(Value::I64(7)),
        b"",
    );
}
#[test]
fn loop_and_each_binding_shadowing_is_local() {
    parity("=x=99;@(x,0;b<3;x=a+1;x,b+1;a)+x", Ok(Value::I64(201)), b"");
    parity("=!each([2,3],0;b=b+1;c+b;a)", Ok(Value::I64(7)), b"");
}
#[test]
fn bindings_preserve_a_live_original_alias() {
    parity(
        "=original=!buffer(2,7);updated=(old=original[0];original[0:old+1]);original[0]*100+updated[0]",
        Ok(Value::I64(708)),
        b"",
    );
    let p = compile_with_profile(
        "=original=!buffer(2,7);updated=original[0:8];original[1]+updated[0]",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert!(nil_llvm::emit_llvm(&p.hir).contains("call ptr @nil_set(ptr"));
}
#[test]
fn binding_liveness_keeps_the_live_capacity_boundary() {
    parity(
        "=old=!bytes(40000000,0);extra=!bytes(40000000,0);#old+#extra",
        Err("E013"),
        b"",
    );
}
#[test]
fn bindings_with_zero_slot_records_keep_physical_layout_sound() {
    parity(
        "record Zero(x:0)\n=empty=Zero([]);seq=!buffer[Zero](2,empty);chosen=seq[0];#!buffer[Zero](3,chosen)",
        Ok(Value::I64(3)),
        b"",
    );
}
#[test]
fn unary_not_can_read_a_record_buffer_field() {
    parity(
        "record Flag(ok:b)\n:b=flags=!buffer[Flag](1,Flag(false));!flags[0].ok",
        Ok(Value::Bool(true)),
        b"",
    );
}
#[test]
fn float_min_max_propagate_nan_and_order_signed_zero() {
    for expr in ["!min(nan,1.0)", "!max(1.0,nan)"] {
        parity(
            &format!(":u64=!bits({expr})"),
            Ok(Value::U64(0x7ff8000000000000)),
            b"",
        );
    }
    for expr in ["!min(0.0,-0.0)", "!min(-0.0,0.0)"] {
        parity(&format!(":u64=!bits({expr})"), Ok(Value::U64(1 << 63)), b"");
    }
    for expr in ["!max(0.0,-0.0)", "!max(-0.0,0.0)"] {
        parity(&format!(":u64=!bits({expr})"), Ok(Value::U64(0)), b"");
    }
}
#[test]
fn float_rounding_and_sqrt_special_patterns_are_bit_exact() {
    let bits = [
        0,
        1 << 63,
        1,
        0x8000000000000001,
        0x0010000000000000,
        f64::MAX.to_bits(),
        (-f64::MAX).to_bits(),
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        0x7ff8000000000000,
        0.5f64.to_bits(),
        (-0.5f64).to_bits(),
        1.5f64.to_bits(),
        2.5f64.to_bits(),
        (-2.5f64).to_bits(),
    ];
    for name in ["abs", "sqrt", "floor", "ceil", "trunc", "round"] {
        let expected = bits
            .iter()
            .map(|b| {
                let f = f64::from_bits(*b);
                canonical_bits(match name {
                    "abs" => f.abs(),
                    "sqrt" => f.sqrt(),
                    "floor" => f.floor(),
                    "ceil" => f.ceil(),
                    "trunc" => f.trunc(),
                    _ => f.round_ties_even(),
                }) as i64
            })
            .collect();
        let expressions = bits
            .iter()
            .map(|b| format!("!trunci64(!bits(!{name}(!floatbits(0x{b:x}u64))))"))
            .collect::<Vec<_>>()
            .join(",");
        parity(
            &format!(":{}=[{expressions}]", bits.len()),
            Ok(Value::array(expected)),
            b"",
        );
    }
}
#[test]
fn float_sqrt_random_finite_patterns_have_bit_exact_parity() {
    let mut seed = 0x2042u64;
    let mut values = vec![];
    for _ in 0..48 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        values.push(seed & 0x7fefffffffffffff);
    }
    let expected = values
        .iter()
        .map(|b| canonical_bits(f64::from_bits(*b).sqrt()) as i64)
        .collect();
    let expressions = values
        .iter()
        .map(|b| format!("!trunci64(!bits(!sqrt(!floatbits(0x{b:x}u64))))"))
        .collect::<Vec<_>>()
        .join(",");
    parity(
        &format!(":{}=[{expressions}]", values.len()),
        Ok(Value::array(expected)),
        b"",
    );
}
#[test]
fn binding_before_heap_swap_enables_the_existing_snapshot_proof() {
    let source = "=(b(!buffer(4,0)[0:2][1:9][2:4][3:1],0,3))[0]\n(v,i,i):v=@(a,b,c;2*b+1<c?(a[b]<a[c(a,b,c)]):false;child=c(a,b,c);a[b:a[child]][child:a[b]],child,c;a)\n(v,i,i)=2*b+2<c?(a[2*b+1]>a[2*b+2]?2*b+1:2*b+2):2*b+1";
    parity(source, Ok(Value::I64(9)), b"");
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("call ptr @nil_set_unique_capture"), "{ir}");
}

#[test]
fn hexadecimal_e_digit_keeps_arithmetic_boundaries_at_o0_o2() {
    parity("=0xe+1", Ok(Value::I64(15)), b"");
    parity("=0xe-1", Ok(Value::I64(13)), b"");
}
