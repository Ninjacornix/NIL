#[test]
fn full_campaign_rejects_undercoverage_before_creating_artifacts() {
    let dir = std::env::temp_dir().join(format!("nil-std-undercoverage-{}", std::process::id()));
    let e = nil_fuzz::application::campaign(1729, 256, &dir).unwrap_err();
    assert!(e.contains("at least 438"));
    assert!(!dir.exists());
}
#[test]
fn std_and_mirrored_provider_families_are_typed_at_three_seeds() {
    for seed in [5130572, 1729, 4294967295] {
        for i in 418..nil_fuzz::application::FAMILY_COUNT {
            let c = nil_fuzz::application::Case::new(seed, i);
            nil_fuzz::application::compile_case(&c.source)
                .unwrap_or_else(|e| panic!("{i}: {e}: {}", c.source));
        }
    }
}
#[test]
fn corpus_hash_diversity_counts_actual_sources_not_seed_labels() {
    use std::collections::BTreeSet;
    let sets = [5130572, 1729, 4294967295].map(|seed| {
        (0..nil_fuzz::application::FAMILY_COUNT)
            .map(|i| nil_fuzz::application::Case::new(seed, i).source)
            .collect::<BTreeSet<_>>()
    });
    assert!(sets[0].difference(&sets[1]).count() > 0);
    assert!(sets[0].intersection(&sets[1]).count() > 0); // Many fixed edge shapes remain fixed.
}
