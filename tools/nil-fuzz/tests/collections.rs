#[test]
fn collection_families_are_typed_and_replay_deterministically() {
    for seed in [0, 1729, u64::MAX] {
        for mode in 264..308 {
            let case = nil_fuzz::application::Case::new(seed, mode);
            let p = nil_fuzz::application::compile_case(&case.source)
                .unwrap_or_else(|e| panic!("seed {seed}, mode {mode}: {e}\n{}", case.source));
            assert_eq!(nil_hir::validate(p.hir.program().clone()).unwrap(), p.hir);
            for (_, source) in nil_fuzz::application::collection_mutations(&case.source) {
                let a = nil_fuzz::application::compile_case(&source);
                let b = nil_fuzz::application::compile_case(&source);
                match (a, b) {
                    (Ok(a), Ok(b)) => assert_eq!(a.hir, b.hir),
                    (Err(a), Err(b)) => assert_eq!(a, b),
                    _ => panic!("nondeterministic collection mutation"),
                }
            }
        }
    }
}

#[test]
fn degenerate_layout_family_covers_products_maps_and_loop_state() {
    for seed in [1729, 8675309, 5130572] {
        for mode in 288..308 {
            let case = nil_fuzz::application::Case::new(seed, mode);
            let p = nil_fuzz::application::compile_case(&case.source)
                .unwrap_or_else(|e| panic!("seed {seed}, mode {mode}: {e}\n{}", case.source));
            assert_eq!(nil_hir::validate(p.hir.program().clone()).unwrap(), p.hir);
        }
    }
}

#[test]
fn bulk_comparison_families_are_typed_and_replay_deterministically() {
    for seed in [1729, 8675309, 5130572] {
        for mode in 308..320 {
            let case = nil_fuzz::application::Case::new(seed, mode);
            let p = nil_fuzz::application::compile_case(&case.source)
                .unwrap_or_else(|e| panic!("seed {seed}, mode {mode}: {e}\n{}", case.source));
            assert_eq!(nil_hir::validate(p.hir.program().clone()).unwrap(), p.hir);
        }
    }
}

#[test]
fn builder_root_transfer_families_are_typed_and_replay_deterministically() {
    for seed in [1729, 8675309, 5130572] {
        for mode in 320..332 {
            let case = nil_fuzz::application::Case::new(seed, mode);
            let p = nil_fuzz::application::compile_case(&case.source)
                .unwrap_or_else(|e| panic!("seed {seed}, mode {mode}: {e}\n{}", case.source));
            assert_eq!(nil_hir::validate(p.hir.program().clone()).unwrap(), p.hir);
        }
    }
}
