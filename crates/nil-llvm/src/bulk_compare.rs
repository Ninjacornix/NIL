//! Prove a provider's complete equality scan from its validated HIR, independent
//! of its identity/source. Hash-consing makes matching linear in HIR size rather
//! than expanding shared expression DAGs into exponentially large trees.
use nil_hir::{BinaryOp, CompareOp, Operation, Region, Type, ValueId, plugin::Provider};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Argument(usize),
    State(usize),
    Integer(i64),
    Boolean(bool),
    Length(usize),
    Index(usize, usize),
    Add(usize, usize),
    Equal(usize, usize),
    Less(usize, usize),
    If(usize, usize, usize),
    Loop(Vec<usize>, usize, Vec<usize>, usize),
}
#[derive(Default)]
struct Graph {
    nodes: BTreeMap<Node, usize>,
    used: BTreeSet<usize>,
    loops: usize,
    in_body: bool,
}
impl Graph {
    fn node(&mut self, node: Node) -> usize {
        let node = match node {
            Node::Equal(a, b) => Node::Equal(a.min(b), a.max(b)),
            Node::Add(a, b) => Node::Add(a.min(b), a.max(b)),
            node => node,
        };
        let next = self.nodes.len();
        *self.nodes.entry(node).or_insert(next)
    }
    fn instructions(
        &mut self,
        list: &[nil_hir::Instruction],
        mut values: Vec<usize>,
    ) -> Option<Vec<usize>> {
        for instruction in list {
            let v = |id: ValueId| values[id.0];
            let node = match &instruction.operation {
                Operation::Constant(n) => Node::Integer(*n),
                Operation::Boolean(b) => Node::Boolean(*b),
                Operation::Length(a) => Node::Length(v(*a)),
                Operation::Index { array, index } if self.in_body => {
                    Node::Index(v(*array), v(*index))
                }
                Operation::Binary {
                    op: BinaryOp::Add,
                    lhs,
                    rhs,
                } => Node::Add(v(*lhs), v(*rhs)),
                Operation::Compare {
                    op: CompareOp::Eq,
                    lhs,
                    rhs,
                } => Node::Equal(v(*lhs), v(*rhs)),
                Operation::Compare {
                    op: CompareOp::Lt,
                    lhs,
                    rhs,
                } => Node::Less(v(*lhs), v(*rhs)),
                Operation::If {
                    condition,
                    then_region,
                    else_region,
                } => {
                    let condition = v(*condition);
                    let yes = self.region(then_region, values.clone())?;
                    let no = self.region(else_region, values.clone())?;
                    Node::If(condition, *yes.first()?, *no.first()?)
                }
                Operation::Loop {
                    initial,
                    condition,
                    body,
                    finish,
                } => {
                    self.loops += 1;
                    // Only one scan is proved. Nested/extra loops stay conservative.
                    if self.loops != 1 {
                        return None;
                    }
                    let initial = initial.iter().map(|id| v(*id)).collect::<Vec<_>>();
                    let state = (0..initial.len())
                        .map(|i| self.node(Node::State(i)))
                        .collect::<Vec<_>>();
                    let condition = self.region(condition, state.clone())?;
                    self.in_body = true;
                    let body = self.region(body, state.clone())?;
                    self.in_body = false;
                    let finish = self.region(finish, state)?;
                    Node::Loop(initial, *condition.first()?, body, *finish.first()?)
                }
                _ => return None,
            };
            let id = self.node(node);
            self.used.insert(id);
            values.push(id);
        }
        Some(values)
    }
    fn region(&mut self, region: &Region, values: Vec<usize>) -> Option<Vec<usize>> {
        let values = self.instructions(&region.instructions, values)?;
        Some(region.results.iter().map(|id| values[id.0]).collect())
    }
    fn expected(&mut self) -> usize {
        let a = self.node(Node::Argument(0));
        let b = self.node(Node::Argument(1));
        let alen = self.node(Node::Length(a));
        let blen = self.node(Node::Length(b));
        let lengths = self.node(Node::Equal(alen, blen));
        let zero = self.node(Node::Integer(0));
        let one = self.node(Node::Integer(1));
        let yes = self.node(Node::Boolean(true));
        let no = self.node(Node::Boolean(false));
        let s = (0..4)
            .map(|i| self.node(Node::State(i)))
            .collect::<Vec<_>>();
        let len = self.node(Node::Length(s[0]));
        let bound = self.node(Node::Less(s[2], len));
        let condition = self.node(Node::If(bound, s[3], no));
        let lhs = self.node(Node::Index(s[0], s[2]));
        let rhs = self.node(Node::Index(s[1], s[2]));
        let equal = self.node(Node::Equal(lhs, rhs));
        let step = self.node(Node::Add(s[2], one));
        let scan = self.node(Node::Loop(
            vec![a, b, zero, yes],
            condition,
            vec![s[0], s[1], step, equal],
            s[3],
        ));
        self.node(Node::If(lengths, scan, no))
    }
}

/// This is an optimization proof, never a plugin declaration or borrowing proof.
/// Every instruction, including dead instructions, must be part of the safe scan.
pub(crate) fn proved(provider: &Provider) -> bool {
    let f = provider.signature();
    if f.parameters.len() != 2
        || f.parameters[0] != f.parameters[1]
        || !matches!(f.parameters[0], Type::Bytes | Type::Buffer)
        || f.result_type != Type::Bool
    {
        return false;
    }
    let mut graph = Graph::default();
    let expected = graph.expected();
    let expected_nodes = graph.nodes.values().copied().collect::<BTreeSet<_>>();
    let inputs = (0..2).map(|i| graph.node(Node::Argument(i))).collect();
    let Some(values) = graph.instructions(&f.instructions, inputs) else {
        return false;
    };
    values[f.result.0] == expected && graph.loops == 1 && graph.used.is_subset(&expected_nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mirrored_equality_and_commuted_increment_keep_complete_scan_proof() {
        for source in [
            "(s,s):b=#b==#a?@(a,b,0,true;c<#a?d:false;a,b,c+1,b[c]==a[c];d):false",
            "(s,s):b=#a==#b?@(a,b,0,true;c<#a?d:false;a,b,1+c,b[c]==a[c];d):false",
            "(v,v):b=#b==#a?@(a,b,0,true;c<#a?d:false;a,b,1+c,b[c]==a[c];d):false",
        ] {
            let p = nil_compiler::compile_with_profile(source, nil_compiler::SourceProfile::ExprV5)
                .unwrap();
            let provider = Provider::new(8, 0, p.hir, nil_hir::FunctionId(0)).unwrap();
            assert!(proved(&provider));
        }
    }
    #[test]
    fn changed_scan_increment_or_asymmetric_predicate_is_not_equivalent() {
        for source in [
            "(s,s):b=#b==#a?@(a,b,0,true;c<#a?d:false;a,b,c+2,b[c]==a[c];d):false",
            "(s,s):b=#b==#a?@(a,b,0,true;c<#a?d:false;a,b,c+1,b[c]<a[c];d):false",
        ] {
            let p = nil_compiler::compile_with_profile(source, nil_compiler::SourceProfile::ExprV5)
                .unwrap();
            let provider = Provider::new(8, 0, p.hir, nil_hir::FunctionId(0)).unwrap();
            assert!(!proved(&provider));
        }
    }
    #[test]
    fn dead_index_before_scan_guard_is_not_a_bulk_proof() {
        let p = nil_compiler::compile_with_profile(
            ":b=!equal(\"a\",\"a\")",
            nil_compiler::SourceProfile::ExprV5,
        )
        .unwrap();
        let providers = nil_hir::plugin::providers(&p.hir);
        assert!(proved(providers[0]));
        let mut program = providers[0].program().program().clone();
        let Operation::If { then_region, .. } = &mut program.functions[0].instructions[5].operation
        else {
            panic!("equality gate")
        };
        let Operation::Loop { condition, .. } = &mut then_region.instructions[0].operation else {
            panic!("scan")
        };
        condition.instructions.push(nil_hir::Instruction {
            ty: Type::I64,
            span: None,
            operation: Operation::Index {
                array: ValueId(0),
                index: ValueId(2),
            },
        });
        let provider = Provider::new(
            29,
            8,
            nil_hir::validate(program).unwrap(),
            nil_hir::FunctionId(0),
        )
        .unwrap();
        // This extra dead read traps at the final condition evaluation. Matching
        // only the return expression, ignoring executed dead instructions, is unsound.
        assert!(!proved(&provider));
    }
}
