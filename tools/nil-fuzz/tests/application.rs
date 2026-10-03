use nil_compiler::{SourceProfile, compile_with_profile};
#[test]
fn application_generator_and_corpus_are_well_typed() {
    for seed in [0, 5130572, u64::MAX] {
        for mode in 0..104 {
            let case = nil_fuzz::application::Case::new(seed, mode);
            compile_with_profile(&case.source, SourceProfile::ExprV5)
                .unwrap_or_else(|e| panic!("seed={seed} mode={mode} {}: {e}", case.source));
        }
    }
    for source in [
        include_str!("../../../fuzz/corpus/expr-v5/generality-parse.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/generality-equal-find.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/sequence-return.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/range-exit.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/call-leaf.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/call-recursive-live.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/call-nested.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/lazy-scalar-loop.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/lazy-effects-order.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/bytes.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/alias.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/loop.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/effects.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/parse.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/byte-priority.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/quota.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/alias-old-read.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/alias-caller.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/alias-loop-state.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/alias-lazy.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/alias-return.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/append-alias.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/append-lazy.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/append-caller.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/append-mixed.nil"),
        include_str!("../../../fuzz/corpus/expr-v5/append-self.nil"),
    ] {
        nil_fuzz::frontend_with_profile(source, SourceProfile::ExprV5);
    }
}
#[test]
fn application_reference_native_and_host_effects_match() {
    let root = std::env::temp_dir().join(format!("nil-v5-fuzz-test-{}", std::process::id()));
    nil_fuzz::application::campaign(5130572, 80, &root).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
