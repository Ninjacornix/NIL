use nil_compiler::{
    SourceProfile,
    application::{Host, SeededHost},
    compile_with_profile,
    evaluator::{Limits, Value, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId};
#[test]
fn seeded_entropy_matches_published_splitmix_vectors() {
    let mut state = 0;
    assert_eq!(
        nil_compiler::application::next_random(&mut state),
        0xe220a8397b1dcdaf
    );
    assert_eq!(
        nil_compiler::application::next_random(&mut state),
        0x6e789e6aa1b965f4
    );
    let mut host = SeededHost::new(nil_compiler::application::DeniedHost, 0);
    assert_eq!(host.random().unwrap(), 0xe220a8397b1dcdaf);
    assert_eq!(host.read(b"anything").unwrap_err().code, "E018");
}
#[test]
fn injected_directory_names_preserve_unsigned_os_bytes_and_order() {
    struct Names;
    impl Host for Names {
        fn directory(&mut self, _: &[u8]) -> Result<Vec<Vec<u8>>, Diagnostic> {
            Ok(vec![vec![255], b"z".to_vec(), b"a".to_vec(), vec![128]])
        }
    }
    let p = compile_with_profile(
        ":s=!each(!directory(\"fixture\"),\"\";!concat(c,a);a)",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert_eq!(
        execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), &mut Names)
            .unwrap(),
        Value::Bytes(vec![b'a', b'z', 128, 255].into())
    );
    assert!(!nil_hir::borrowing::Summaries::analyze(&p.hir).function(0));
}
#[test]
fn effects_keep_order_and_do_not_run_in_unselected_arms() {
    #[derive(Default)]
    struct Events(Vec<Vec<u8>>);
    impl Host for Events {
        fn env(&mut self, name: &[u8]) -> Result<Vec<u8>, Diagnostic> {
            self.0.push(name.to_vec());
            Ok(name.to_vec())
        }
    }
    let p = compile_with_profile(
        ":s=!concat(!env(\"first\"),true?!env(\"second\"):!env(\"wrong\"))",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let mut host = Events::default();
    execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), &mut host).unwrap();
    assert_eq!(host.0, vec![b"first".to_vec(), b"second".to_vec()]);
}
#[test]
fn new_effect_signatures_reject_wrong_types_and_arity() {
    for source in ["=!random(1)", ":s=!env(1)", "=!size(!directory(1))"] {
        assert!(
            compile_with_profile(source, SourceProfile::ExprV5).is_err(),
            "{source}"
        );
    }
}
