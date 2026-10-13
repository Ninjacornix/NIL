//! Native integer-set lowering is refused, not guessed (ADR 051 part 2).
//!
//! The frontend and reference semantics land first; the runtime table needs an
//! integer-hash path before a native result can be claimed. Refusing keeps
//! native answers honest, and reference execution already runs these programs.
use nil_compiler::{SourceProfile, compile_with_profile};

#[test]
fn native_integer_set_lowering_is_refused_on_every_path() {
    let program = compile_with_profile("=(s=!add(!set(),1);!size(s))\n", SourceProfile::ExprV5)
        .expect("a set program must still compile");
    let error = nil_llvm::entry_runtime(&program.hir, nil_llvm::Options::default())
        .expect_err("native lowering must refuse");
    assert_eq!(error.code, "E009");
    assert!(error.message.contains("integer-set"), "{}", error.message);
    // The build path does not reach entry_runtime first, so it refuses too.
    let directory = std::env::temp_dir().join(format!("nil-set-refusal-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("scratch");
    let error = nil_llvm::build(
        &program.hir,
        &directory.join("out"),
        nil_llvm::Options::default(),
    )
    .expect_err("the build path must refuse");
    assert_eq!(error.code, "E009");
    let _ = std::fs::remove_dir_all(&directory);
}
