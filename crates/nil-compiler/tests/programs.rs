use nil_compiler::{
    compile,
    evaluator::{Limits, execute},
};

macro_rules! program_test {
    ($name:ident, $file:literal, $args:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let program = compile(include_str!($file)).unwrap();
            let result = execute(
                &program.hir,
                program.function(0).unwrap(),
                $args,
                Limits::default(),
            )
            .unwrap();
            assert_eq!(result, $expected);
        }
    };
}

program_test!(arithmetic, "fixtures/programs/arithmetic.nil", &[], 32);
program_test!(nested_calls, "fixtures/programs/nested-calls.nil", &[], 682);
program_test!(arguments, "fixtures/programs/arguments.nil", &[50, 8], 42);
