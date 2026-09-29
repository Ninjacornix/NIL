//! Test the parameterized solution functions, not only their demonstration wrappers.
use nil_compiler::{
    CompiledProgram, compile,
    evaluator::{Limits, execute},
};

fn solution(program: &CompiledProgram, args: &[i64]) -> i64 {
    execute(
        &program.hir,
        program.function(1).unwrap(),
        args,
        Limits::default(),
    )
    .unwrap()
}

#[test]
fn count_odds_matches_enumeration_for_all_small_intervals() {
    let program = compile(include_str!("../../../examples/count_odds.nil")).unwrap();
    for low in 0..=100 {
        for high in low..=100 {
            let expected = (low..=high).filter(|n| n % 2 != 0).count() as i64;
            assert_eq!(
                solution(&program, &[low, high]),
                expected,
                "[{low}, {high}]"
            );
        }
    }
}

#[test]
fn count_odds_examples_and_problem_boundaries() {
    let program = compile(include_str!("../../../examples/count_odds.nil")).unwrap();
    for (low, high, expected) in [
        (3, 7, 3),
        (8, 10, 1),
        (0, 0, 0),
        (1, 1, 1),
        (0, 1_000_000_000, 500_000_000),
        (1_000_000_000, 1_000_000_000, 0),
        (999_999_999, 1_000_000_000, 1),
    ] {
        assert_eq!(solution(&program, &[low, high]), expected);
    }
}

#[test]
fn smallest_even_multiple_exhausts_the_problem_domain() {
    let program = compile(include_str!("../../../examples/smallest_even_multiple.nil")).unwrap();
    for n in 1..=150 {
        let expected = (1..=2 * n)
            .find(|value| value % 2 == 0 && value % n == 0)
            .unwrap();
        assert_eq!(solution(&program, &[n]), expected, "n={n}");
    }
}

#[test]
fn leetcode_bank_exhausts_the_problem_domain_against_daily_simulation() {
    let program = compile(include_str!("../../../examples/leetcode_bank.nil")).unwrap();
    let mut total = 0;
    let mut monday_deposit = 1;
    let mut weekday = 0;
    for n in 1..=1000 {
        total += monday_deposit + weekday;
        assert_eq!(solution(&program, &[n]), total, "day={n}");
        weekday += 1;
        if weekday == 7 {
            weekday = 0;
            monday_deposit += 1;
        }
    }
}

#[test]
fn leetcode_bank_published_examples() {
    let program = compile(include_str!("../../../examples/leetcode_bank.nil")).unwrap();
    for (n, expected) in [(4, 10), (10, 37), (20, 96)] {
        assert_eq!(solution(&program, &[n]), expected);
    }
}

#[test]
fn polynomial_matches_horner_evaluation() {
    let program = compile(include_str!("../../../examples/polynomial.nil")).unwrap();
    for a in -2..=2 {
        for b in -2..=2 {
            for c in -2..=2 {
                for x in -2..=2 {
                    assert_eq!(solution(&program, &[a, b, c, x]), (a * x + b) * x + c);
                }
            }
        }
    }
}

#[test]
fn sum_of_squares_handles_signed_and_zero_arguments() {
    let program = compile(include_str!("../../../examples/sum_of_squares.nil")).unwrap();
    for a in -10..=10 {
        for b in -10..=10 {
            assert_eq!(solution(&program, &[a, b]), a.pow(2) + b.pow(2));
        }
    }
}
