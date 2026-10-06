#[test]
fn host_effect_families_are_deterministic_and_well_typed() {
    for seed in [0, 1729, u64::MAX] {
        for mode in 356..392 {
            let case = nil_fuzz::application::Case::new(seed, mode);
            assert_eq!(
                case.source,
                nil_fuzz::application::Case::new(seed, mode).source
            );
            nil_fuzz::application::compile_case(&case.source)
                .unwrap_or_else(|e| panic!("mode {mode}: {e}: {}", case.source));
        }
    }
}
