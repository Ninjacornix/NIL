use nil_compiler::{SourceProfile, compile_with_profile, hir::Phase};

macro_rules! rejected {
    ($name:ident, $file:literal, $code:literal, $phase:ident, $mismatch:expr) => {
        #[test]
        fn $name() {
            let source = include_str!($file);
            let error = compile_with_profile(source, SourceProfile::LinesV0).unwrap_err();
            assert_eq!(error.code, $code);
            assert_eq!(error.phase, Phase::$phase);
            let span = error.span.expect("source error must have a span");
            assert!(span.start < span.end && span.end <= source.len());
            assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
            assert_eq!(
                error.expected.as_deref().zip(error.actual.as_deref()),
                $mismatch
            );
        }
    };
}

rejected!(
    malformed,
    "fixtures/fail/malformed.nil",
    "E001",
    Parse,
    Some(("2", "1"))
);
rejected!(
    unknown_function,
    "fixtures/fail/unknown-function.nil",
    "E004",
    Check,
    None
);
rejected!(
    unknown_value,
    "fixtures/fail/unknown-value.nil",
    "E005",
    Check,
    None
);
rejected!(
    wrong_arity,
    "fixtures/fail/wrong-arity.nil",
    "E006",
    Check,
    Some(("1", "0"))
);
rejected!(
    unsupported_parameter,
    "fixtures/fail/unsupported-parameter.nil",
    "E002",
    Parse,
    Some(("i64", "str"))
);
rejected!(
    unsupported_return,
    "fixtures/fail/unsupported-return.nil",
    "E002",
    Parse,
    Some(("i64", "bool"))
);
rejected!(
    duplicate_function,
    "fixtures/fail/duplicate-function.nil",
    "E003",
    Check,
    None
);
rejected!(
    invalid_operation,
    "fixtures/fail/invalid-operation.nil",
    "E001",
    Parse,
    None
);
