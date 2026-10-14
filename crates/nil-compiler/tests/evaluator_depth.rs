//! The reference evaluator must cost O(1) per call, not O(depth).
//!
//! It used to sweep every value of every live frame on every step, so a
//! depth-d recursion cost O(d^2): a pure recursion with no data structures
//! measured 0.42 s at depth 1,000 and 300 s at 30,000, and the depth-100,000
//! tree fixtures could not finish inside the 120 s qualification budget.
//!
//! This guards the shape of the cost curve, not an absolute time, so it uses a
//! wide margin: quadratic growth would show as 16x for a 4x depth increase,
//! linear as 4x, and the test only fails past 8x.
use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::FunctionId;
use std::time::Instant;

/// Pure self-recursion: one call per level and no dynamic values at all, so the
/// only thing that can scale with depth is per-call bookkeeping.
const RECURSE: &str = "1=a<=0?0:1+a(a-1)\n";

fn seconds_at(depth: i64) -> f64 {
    let program = compile_with_profile(RECURSE, SourceProfile::ExprV5).unwrap();
    let start = Instant::now();
    let result = execute_values(
        &program.hir,
        FunctionId(0),
        &[Value::I64(depth)],
        Limits {
            steps: u64::MAX,
            call_depth: 1_000_000,
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
        },
    )
    .unwrap();
    let elapsed = start.elapsed().as_secs_f64();
    assert_eq!(result, Value::I64(depth), "recursion must still be correct");
    elapsed
}

#[test]
fn per_call_cost_does_not_grow_with_recursion_depth() {
    // Warm up so first-run effects do not land on the shallow measurement.
    seconds_at(2_000);
    let shallow = seconds_at(4_000);
    let deep = seconds_at(16_000);
    // Four times the depth. Linear predicts about 4x, quadratic about 16x.
    let growth = deep / shallow.max(1e-6);
    assert!(
        growth < 8.0,
        "per-call cost grew with depth: {shallow:.4}s at 4,000 and {deep:.4}s at 16,000 \
         is {growth:.2}x for a 4x depth increase, which is closer to quadratic than linear"
    );
}

#[test]
fn deep_recursion_still_reports_the_configured_limit() {
    // The fix must not weaken the guard: E008 still fires exactly at the limit.
    let program = compile_with_profile(RECURSE, SourceProfile::ExprV5).unwrap();
    let error = execute_values(
        &program.hir,
        FunctionId(0),
        &[Value::I64(5_000)],
        Limits {
            steps: u64::MAX,
            call_depth: 256,
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "E008");
    assert!(error.message.contains("call depth"), "{}", error.message);
    // And a depth inside the limit still succeeds.
    assert_eq!(
        execute_values(
            &program.hir,
            FunctionId(0),
            &[Value::I64(200)],
            Limits {
                steps: u64::MAX,
                call_depth: 256,
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
            },
        )
        .unwrap(),
        Value::I64(200)
    );
}

#[test]
fn live_byte_accounting_survives_deep_frames_holding_shared_values() {
    // A shared sequence held by every frame: bytes must be counted once, so the
    // memory guard must not trip, and the incremental accounting must subtract
    // exactly what it added as frames unwind.
    // One self-recursive function; `a(..)` is the call, bare `a` the buffer.
    let source = "(v,i)=b<=0?#a:#a+a(a,b-1)\n";
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let buffer = Value::Buffer((0..512).collect::<Vec<i64>>().into());
    let depth = 2_000;
    let result = execute_values(
        &program.hir,
        FunctionId(0),
        &[buffer, Value::I64(depth)],
        Limits {
            steps: u64::MAX,
            call_depth: 1_000_000,
            // Far below depth * buffer size, so double counting would trip it.
            memory_bytes: 1 << 20,
        },
    )
    .unwrap();
    assert_eq!(result, Value::I64(512 * (depth + 1)));
}
