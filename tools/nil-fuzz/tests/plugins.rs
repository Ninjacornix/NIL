#[test]
fn every_plugin_family_is_typed_and_replays_deterministically() {
    for seed in [0, 1729, u64::MAX] {
        for mode in 240..264 {
            let case = nil_fuzz::application::Case::new(seed, mode);
            let p = nil_fuzz::application::compile_case(&case.source)
                .unwrap_or_else(|e| panic!("seed {seed},mode {mode}: {e}\n{}", case.source));
            assert_eq!(nil_hir::validate(p.hir.program().clone()).unwrap(), p.hir);
            for (_, source) in nil_fuzz::application::plugin_mutations(&case.source) {
                let a = nil_fuzz::application::compile_case(&source);
                let b = nil_fuzz::application::compile_case(&source);
                match (a, b) {
                    (Ok(a), Ok(b)) => assert_eq!(a.hir, b.hir),
                    (Err(a), Err(b)) => assert_eq!(a, b),
                    _ => panic!("nondeterministic plugin mutation"),
                }
            }
        }
    }
}
