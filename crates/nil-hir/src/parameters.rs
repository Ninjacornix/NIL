//! Per-function, per-parameter ownership summaries over validated HIR.
//!
//! Two facts per function, settled by ADR 049's R2 addendum:
//!
//! * `owns[f][k]` — `f` installs an owning root for parameter `k` that covers
//!   every allocation and collection in its body. This describes what lowering
//!   already does (a live parameter of a non-borrow function is rooted at entry)
//!   rather than predicting it, so a caller may surrender its own root only when
//!   the callee's root demonstrably spans the gap.
//! * `origins[f]` — the parameters whose storage `f`'s result may alias, or
//!   `Unknown`. Returning an alias hands a live obligation back to the caller;
//!   it is never evidence that the parameter was fresh.
//!
//! `origins` is recursive, so it is solved as a least fixed point over finite
//! parameter-index sets, iterating the whole call graph until stable. Direct and
//! mutual recursion converge without any optimistic acyclic shortcut (ADR 049
//! obligation 4). Unknown provenance is never relaxed into "no origin".
//!
//! Stated conservative gaps: every intrinsic is unknown, because none of them
//! has a published result-provenance proof and some (`!slice`) may reuse an
//! argument's storage; a region result naming an enclosing local rather than a
//! parameter is unknown, because this analysis walks one SSA scope at a time and
//! does not thread the enclosing frames needed to resolve it. Both block
//! transfer rather than under-reporting an origin.
use crate::{Operation, Region, ValidatedProgram, ValueId};
use std::collections::BTreeSet;

/// Parameters whose storage a result may alias. `Unknown` is the conservative
/// top of the lattice and is not a superset of `Parameters`: it blocks transfer
/// outright instead of naming candidates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origins {
    Parameters(BTreeSet<usize>),
    Unknown,
}
impl Origins {
    fn empty() -> Self {
        Self::Parameters(BTreeSet::new())
    }
    /// Monotone join. Unknown absorbs, so the iteration only moves upward on a
    /// finite lattice and therefore terminates.
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Parameters(mut a), Self::Parameters(b)) => {
                a.extend(b);
                Self::Parameters(a)
            }
        }
    }
    pub fn is_known(&self) -> bool {
        matches!(self, Self::Parameters(_))
    }
    pub fn contains(&self, parameter: usize) -> bool {
        match self {
            Self::Unknown => false,
            Self::Parameters(set) => set.contains(&parameter),
        }
    }
}

#[derive(Debug)]
pub struct Ownership {
    owns: Vec<Vec<bool>>,
    origins: Vec<Origins>,
}

impl Ownership {
    pub fn analyze(program: &ValidatedProgram, borrowing: &crate::borrowing::Summaries) -> Self {
        let functions = &program.program().functions;
        // A live parameter of a non-borrow function is rooted at entry, ahead of
        // the body, so its root covers the caller's transfer gap. A borrow
        // function installs no parameter root and cannot update a sequence.
        let owns = functions
            .iter()
            .enumerate()
            .map(|(index, function)| {
                let live = crate::liveness::last_uses(
                    &function.instructions,
                    std::slice::from_ref(&function.result),
                    function.parameters.len(),
                );
                let rooted = !borrowing.function(index);
                (0..function.parameters.len())
                    .map(|k| rooted && live[k].is_some())
                    .collect()
            })
            .collect();
        let mut result = Self {
            owns,
            origins: vec![Origins::empty(); functions.len()],
        };
        loop {
            let mut changed = false;
            for (index, function) in functions.iter().enumerate() {
                let parameters = function.parameters.len();
                let found = result.origins_in(
                    &function.instructions,
                    parameters,
                    parameters,
                    function.result,
                    &mut 256,
                );
                if found != result.origins[index] {
                    result.origins[index] = found;
                    changed = true;
                }
            }
            if !changed {
                return result;
            }
        }
    }

    /// Parameter origins of `value` inside one SSA scope. `inputs` counts the
    /// values entering this scope; `parameters` is the enclosing function's
    /// parameter count, which stays fixed as regions nest.
    fn origins_in(
        &self,
        instructions: &[crate::Instruction],
        inputs: usize,
        parameters: usize,
        value: ValueId,
        budget: &mut usize,
    ) -> Origins {
        if *budget == 0 {
            return Origins::Unknown;
        }
        *budget -= 1;
        if value.0 < inputs {
            // A function parameter is its own origin. Any other value entering
            // this scope is an enclosing local whose producer lives in a frame
            // this walk does not carry, so it stays unknown.
            return if value.0 < parameters {
                Origins::Parameters([value.0].into_iter().collect())
            } else {
                Origins::Unknown
            };
        }
        let instruction = &instructions[value.0 - inputs];
        match &instruction.operation {
            // An update, projection or field write keeps its subject's storage
            // provenance: the result may be that allocation mutated in place.
            Operation::Replace { array, .. } => {
                self.origins_in(instructions, inputs, parameters, *array, budget)
            }
            Operation::Field { record, .. } => {
                self.origins_in(instructions, inputs, parameters, *record, budget)
            }
            Operation::UpdateField { record, value, .. } => self
                .origins_in(instructions, inputs, parameters, *record, budget)
                .join(self.origins_in(instructions, inputs, parameters, *value, budget)),
            Operation::Record { fields, .. } => fields.iter().fold(Origins::empty(), |acc, id| {
                acc.join(self.origins_in(instructions, inputs, parameters, *id, budget))
            }),
            // Both arms are joined: a conditional identity still reports its
            // origin, and laziness never hides one.
            Operation::If {
                then_region,
                else_region,
                ..
            } => self
                .region_origins(then_region, value.0, parameters, budget)
                .join(self.region_origins(else_region, value.0, parameters, budget)),
            // Substitute the callee's result provenance through this call's
            // arguments. Recursion is resolved by the outer fixed point.
            Operation::Call {
                function,
                arguments,
            } => match self.origins.get(function.0) {
                None | Some(Origins::Unknown) => Origins::Unknown,
                Some(Origins::Parameters(set)) => {
                    set.iter()
                        .fold(Origins::empty(), |acc, k| match arguments.get(*k) {
                            None => Origins::Unknown,
                            Some(id) => acc.join(self.origins_in(
                                instructions,
                                inputs,
                                parameters,
                                *id,
                                budget,
                            )),
                        })
                }
            },
            // Fresh storage, or a scalar that cannot carry sequence provenance.
            Operation::Bytes(_)
            | Operation::RecordMap(_)
            | Operation::RecordBuffer { .. }
            | Operation::Array(_)
            | Operation::Repeat { .. }
            | Operation::Constant(_)
            | Operation::Unsigned { .. }
            | Operation::Float(_)
            | Operation::Boolean(_)
            | Operation::Length(_)
            | Operation::Index { .. }
            | Operation::Binary { .. }
            | Operation::Unary { .. }
            | Operation::Compare { .. } => Origins::empty(),
            // Intrinsics, plugin calls, loops and anything added later stay
            // unknown until they have their own provenance proof.
            _ => Origins::Unknown,
        }
    }

    fn region_origins(
        &self,
        region: &Region,
        inputs: usize,
        parameters: usize,
        budget: &mut usize,
    ) -> Origins {
        let Some(result) = region.results.first() else {
            return Origins::Unknown;
        };
        self.origins_in(&region.instructions, inputs, parameters, *result, budget)
    }

    /// Does `function` install an owning root for `parameter` that covers every
    /// allocation and collection in its body?
    pub fn owns(&self, function: usize, parameter: usize) -> bool {
        self.owns
            .get(function)
            .and_then(|p| p.get(parameter))
            .copied()
            .unwrap_or(false)
    }
    /// Result provenance of `function`. An out-of-range index is unknown, never
    /// empty, so an unresolved callee cannot be mistaken for a fresh allocation.
    pub fn origins(&self, function: usize) -> Origins {
        self.origins
            .get(function)
            .cloned()
            .unwrap_or(Origins::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Arithmetic, BinaryOp, CompareOp, Function, FunctionId, Instruction, Intrinsic, Program,
        Type, validate,
    };

    fn inst(operation: Operation, ty: Type) -> Instruction {
        Instruction {
            operation,
            ty,
            span: None,
        }
    }
    fn function(parameters: Vec<Type>, result_type: Type, body: Vec<Instruction>) -> Function {
        let result = ValueId(parameters.len() + body.len() - 1);
        Function {
            parameters,
            result_type,
            instructions: body,
            result,
            return_span: None,
        }
    }
    fn analyze(functions: Vec<Function>) -> Ownership {
        let hir = validate(Program {
            records: vec![],
            arithmetic: Arithmetic::Wrapping,
            functions,
        })
        .expect("valid program");
        let borrowing = crate::borrowing::Summaries::analyze(&hir);
        Ownership::analyze(&hir, &borrowing)
    }
    /// `(Buffer) -> I64` entry that calls `target` so multi-function programs
    /// keep a valid first-order shape without constraining the callee.
    fn caller(target: usize, result_type: Type) -> Function {
        let mut body = vec![inst(
            Operation::Call {
                function: FunctionId(target),
                arguments: vec![ValueId(0)],
            },
            result_type,
        )];
        if result_type != Type::I64 {
            body.push(inst(Operation::Length(ValueId(1)), Type::I64));
        }
        function(vec![Type::Buffer], Type::I64, body)
    }

    #[test]
    fn an_updating_callee_owns_its_sequence_parameter_and_returns_its_origin() {
        let updating = function(
            vec![Type::Buffer],
            Type::Buffer,
            vec![
                inst(Operation::Constant(0), Type::I64),
                inst(Operation::Constant(9), Type::I64),
                inst(
                    Operation::Replace {
                        array: ValueId(0),
                        index: ValueId(1),
                        value: ValueId(2),
                    },
                    Type::Buffer,
                ),
            ],
        );
        let proof = analyze(vec![caller(1, Type::Buffer), updating]);
        assert!(proof.owns(1, 0));
        assert!(proof.origins(1).contains(0));
    }

    #[test]
    fn a_read_only_borrow_callee_installs_no_parameter_root() {
        // Scalar reads alone keep the callee allocation free, so it installs no
        // parameter root and must never receive a transferred owner.
        let reading = function(
            vec![Type::Buffer],
            Type::I64,
            vec![
                inst(Operation::Constant(0), Type::I64),
                inst(
                    Operation::Index {
                        array: ValueId(0),
                        index: ValueId(1),
                    },
                    Type::I64,
                ),
            ],
        );
        let proof = analyze(vec![caller(1, Type::I64), reading]);
        assert!(!proof.owns(1, 0));
        assert_eq!(proof.origins(1), Origins::Parameters(BTreeSet::new()));
    }

    #[test]
    fn an_unused_parameter_is_not_owned_even_in_an_allocating_callee() {
        let ignoring = function(
            vec![Type::Buffer],
            Type::Buffer,
            vec![
                inst(Operation::Constant(2), Type::I64),
                inst(Operation::Constant(1), Type::I64),
                inst(
                    Operation::Intrinsic {
                        op: Intrinsic::Buffer,
                        arguments: vec![ValueId(1), ValueId(2)],
                    },
                    Type::Buffer,
                ),
            ],
        );
        let proof = analyze(vec![caller(1, Type::Buffer), ignoring]);
        assert!(!proof.owns(1, 0));
    }

    #[test]
    fn both_lazy_arms_join_so_a_conditional_identity_still_reports_its_origin() {
        // cond ? a : a[0:9] — the identity arm must not be lost.
        let update = Region {
            instructions: vec![
                inst(Operation::Constant(0), Type::I64),
                inst(Operation::Constant(9), Type::I64),
                inst(
                    Operation::Replace {
                        array: ValueId(0),
                        index: ValueId(2),
                        value: ValueId(3),
                    },
                    Type::Buffer,
                ),
            ],
            results: vec![ValueId(4)],
        };
        let conditional = function(
            vec![Type::Buffer, Type::Bool],
            Type::Buffer,
            vec![inst(
                Operation::If {
                    condition: ValueId(1),
                    then_region: Region {
                        instructions: vec![],
                        results: vec![ValueId(0)],
                    },
                    else_region: update,
                },
                Type::Buffer,
            )],
        );
        let entry = function(
            vec![Type::Buffer],
            Type::I64,
            vec![
                inst(Operation::Boolean(true), Type::Bool),
                inst(
                    Operation::Call {
                        function: FunctionId(1),
                        arguments: vec![ValueId(0), ValueId(1)],
                    },
                    Type::Buffer,
                ),
                inst(Operation::Length(ValueId(2)), Type::I64),
            ],
        );
        let proof = analyze(vec![entry, conditional]);
        assert!(proof.origins(1).contains(0));
    }

    #[test]
    fn origins_substitute_through_a_callee_and_pick_the_right_argument() {
        // Function 1 forwards its second parameter to function 2, which returns
        // an update of its own first parameter. Only parameter 1 is an origin.
        let forwarding = function(
            vec![Type::Buffer, Type::Buffer],
            Type::Buffer,
            vec![inst(
                Operation::Call {
                    function: FunctionId(2),
                    arguments: vec![ValueId(1)],
                },
                Type::Buffer,
            )],
        );
        let updating = function(
            vec![Type::Buffer],
            Type::Buffer,
            vec![
                inst(Operation::Constant(0), Type::I64),
                inst(Operation::Constant(9), Type::I64),
                inst(
                    Operation::Replace {
                        array: ValueId(0),
                        index: ValueId(1),
                        value: ValueId(2),
                    },
                    Type::Buffer,
                ),
            ],
        );
        let entry = function(
            vec![Type::Buffer],
            Type::I64,
            vec![
                inst(
                    Operation::Call {
                        function: FunctionId(1),
                        arguments: vec![ValueId(0), ValueId(0)],
                    },
                    Type::Buffer,
                ),
                inst(Operation::Length(ValueId(1)), Type::I64),
            ],
        );
        let proof = analyze(vec![entry, forwarding, updating]);
        assert!(proof.origins(1).contains(1));
        assert!(!proof.origins(1).contains(0));
    }

    #[test]
    fn mutual_recursion_reaches_a_fixed_point_without_an_acyclic_shortcut() {
        // f(a,n) = n<1 ? a[0:9] : g(a,n-1);  g(a,n) = n<1 ? a : f(a,n-1).
        // Both must report parameter 0 after the fixed point converges.
        let step = |other: usize, identity: bool| {
            let base = if identity {
                Region {
                    instructions: vec![],
                    results: vec![ValueId(0)],
                }
            } else {
                // Region inputs are the two parameters plus the two values
                // computed before the If, so region locals start at v4.
                Region {
                    instructions: vec![
                        inst(Operation::Constant(0), Type::I64),
                        inst(Operation::Constant(9), Type::I64),
                        inst(
                            Operation::Replace {
                                array: ValueId(0),
                                index: ValueId(4),
                                value: ValueId(5),
                            },
                            Type::Buffer,
                        ),
                    ],
                    results: vec![ValueId(6)],
                }
            };
            function(
                vec![Type::Buffer, Type::I64],
                Type::Buffer,
                vec![
                    inst(Operation::Constant(1), Type::I64),
                    inst(
                        Operation::Compare {
                            op: CompareOp::Lt,
                            lhs: ValueId(1),
                            rhs: ValueId(2),
                        },
                        Type::Bool,
                    ),
                    inst(
                        Operation::If {
                            condition: ValueId(3),
                            then_region: base,
                            else_region: Region {
                                instructions: vec![
                                    inst(Operation::Constant(1), Type::I64),
                                    inst(
                                        Operation::Binary {
                                            op: BinaryOp::Sub,
                                            lhs: ValueId(1),
                                            rhs: ValueId(4),
                                        },
                                        Type::I64,
                                    ),
                                    inst(
                                        Operation::Call {
                                            function: FunctionId(other),
                                            arguments: vec![ValueId(0), ValueId(5)],
                                        },
                                        Type::Buffer,
                                    ),
                                ],
                                results: vec![ValueId(6)],
                            },
                        },
                        Type::Buffer,
                    ),
                ],
            )
        };
        let entry = function(
            vec![Type::Buffer],
            Type::I64,
            vec![
                inst(Operation::Constant(2), Type::I64),
                inst(
                    Operation::Call {
                        function: FunctionId(1),
                        arguments: vec![ValueId(0), ValueId(1)],
                    },
                    Type::Buffer,
                ),
                inst(Operation::Length(ValueId(2)), Type::I64),
            ],
        );
        let proof = analyze(vec![entry, step(2, false), step(1, true)]);
        assert!(proof.origins(1).contains(0));
        assert!(proof.origins(2).contains(0));
    }

    #[test]
    fn an_unmodelled_intrinsic_producer_stays_unknown_rather_than_fresh() {
        // Concat has no published result-provenance proof, so it is unknown and
        // blocks transfer instead of looking like a fresh allocation.
        let concatenating = function(
            vec![Type::Bytes],
            Type::Bytes,
            vec![
                inst(Operation::Bytes(vec![99]), Type::Bytes),
                inst(
                    Operation::Intrinsic {
                        op: Intrinsic::Concat,
                        arguments: vec![ValueId(0), ValueId(1)],
                    },
                    Type::Bytes,
                ),
            ],
        );
        let entry = function(
            vec![Type::Bytes],
            Type::I64,
            vec![
                inst(
                    Operation::Call {
                        function: FunctionId(1),
                        arguments: vec![ValueId(0)],
                    },
                    Type::Bytes,
                ),
                inst(Operation::Length(ValueId(1)), Type::I64),
            ],
        );
        let proof = analyze(vec![entry, concatenating]);
        assert_eq!(proof.origins(1), Origins::Unknown);
        assert!(!proof.origins(1).is_known());
    }

    #[test]
    fn out_of_range_queries_are_unknown_and_unowned() {
        let proof = analyze(vec![function(
            vec![Type::Buffer],
            Type::I64,
            vec![inst(Operation::Length(ValueId(0)), Type::I64)],
        )]);
        assert_eq!(proof.origins(99), Origins::Unknown);
        assert!(!proof.owns(99, 0));
        assert!(!proof.owns(0, 99));
    }

    #[test]
    fn unknown_absorbs_in_the_join_and_names_no_candidate() {
        let known = Origins::Parameters([0].into_iter().collect());
        assert_eq!(known.clone().join(Origins::Unknown), Origins::Unknown);
        assert_eq!(Origins::Unknown.join(known), Origins::Unknown);
        assert!(!Origins::Unknown.contains(0));
        assert!(Origins::empty().is_known());
    }

    #[test]
    fn an_exhausted_budget_reports_unknown_instead_of_recursing() {
        let proof = Ownership {
            owns: vec![vec![]],
            origins: vec![Origins::empty()],
        };
        let instructions = vec![inst(Operation::Constant(0), Type::I64)];
        assert_eq!(
            proof.origins_in(&instructions, 0, 0, ValueId(0), &mut 0),
            Origins::Unknown
        );
    }
}
