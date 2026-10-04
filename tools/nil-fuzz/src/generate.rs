//! A typed tree/oracle independent of the compiler AST, lowering and evaluator.
use nil_hir::Type;
#[derive(Clone)]
pub struct Random(pub u64);
impl Random {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    pub fn pick(&mut self, n: usize) -> usize {
        self.next_u64() as usize % n
    }
    pub fn integer(&mut self) -> i64 {
        match self.pick(8) {
            0 => i64::MIN,
            1 => i64::MAX,
            2 => -1,
            3 => 0,
            4 => 1,
            _ => self.next_u64() as i64,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bool(bool),
}
impl Value {
    fn integer(self) -> i64 {
        match self {
            Self::Int(v) => v,
            _ => panic!("oracle type bug"),
        }
    }
    fn boolean(self) -> bool {
        match self {
            Self::Bool(v) => v,
            _ => panic!("oracle type bug"),
        }
    }
}
#[derive(Clone, Debug)]
pub enum Expr {
    Int(i64),
    Bool(bool),
    Param(usize),
    Binary(usize, Box<Expr>, Box<Expr>),
    Compare(usize, Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(usize, Vec<Expr>),
    Loop(Vec<Expr>, Box<Expr>, Vec<Expr>, Box<Expr>),
}
const BIN: [&str; 4] = ["+", "-", "*", "/"];
const CMP: [&str; 6] = ["==", "!=", "<", "<=", ">", ">="];
impl Expr {
    pub fn source(&self) -> String {
        match self {
            Self::Int(v) => v.to_string(),
            Self::Bool(v) => v.to_string(),
            Self::Param(v) => char::from(b'a' + *v as u8).to_string(),
            Self::Binary(op, a, b) => format!("({}{}{})", a.source(), BIN[*op], b.source()),
            Self::Compare(op, a, b) => format!("({}{}{})", a.source(), CMP[*op], b.source()),
            Self::If(c, a, b) => format!("({}?{}:{})", c.source(), a.source(), b.source()),
            Self::Call(id, args) => format!("{}({})", char::from(b'a' + *id as u8), join(args)),
            Self::Loop(init, c, updates, result) => format!(
                "@({};{};{};{})",
                join(init),
                c.source(),
                join(updates),
                result.source()
            ),
        }
    }
    fn eval(
        &self,
        values: &[Value],
        program: &[Expr],
        fuel: &mut usize,
    ) -> Result<Value, &'static str> {
        *fuel = fuel.checked_sub(1).ok_or("oracle budget exhausted")?;
        Ok(match self {
            Self::Int(v) => Value::Int(*v),
            Self::Bool(v) => Value::Bool(*v),
            Self::Param(id) => values[*id],
            Self::Binary(op, a, b) => {
                let a = a.eval(values, program, fuel)?.integer();
                let b = b.eval(values, program, fuel)?.integer();
                Value::Int(match op {
                    0 => (a as u64).wrapping_add(b as u64) as i64,
                    1 => (a as u64).wrapping_sub(b as u64) as i64,
                    2 => (a as u64).wrapping_mul(b as u64) as i64,
                    3 if b == 0 => return Err("division by zero"),
                    3 => ((a as i128) / (b as i128)) as i64,
                    _ => unreachable!(),
                })
            }
            Self::Compare(op, a, b) => {
                let a = a.eval(values, program, fuel)?.integer();
                let b = b.eval(values, program, fuel)?.integer();
                Value::Bool(match op {
                    0 => a == b,
                    1 => a != b,
                    2 => a < b,
                    3 => a <= b,
                    4 => a > b,
                    5 => a >= b,
                    _ => unreachable!(),
                })
            }
            Self::If(c, a, b) => {
                if c.eval(values, program, fuel)?.boolean() {
                    a.eval(values, program, fuel)?
                } else {
                    b.eval(values, program, fuel)?
                }
            }
            Self::Call(id, args) => {
                let args = args
                    .iter()
                    .map(|a| a.eval(values, program, fuel))
                    .collect::<Result<Vec<_>, _>>()?;
                program[*id].eval(&args, program, fuel)?
            }
            Self::Loop(init, c, updates, result) => {
                let mut state = init
                    .iter()
                    .map(|a| a.eval(values, program, fuel))
                    .collect::<Result<Vec<_>, _>>()?;
                while c.eval(&state, program, fuel)?.boolean() {
                    state = updates
                        .iter()
                        .map(|a| a.eval(&state, program, fuel))
                        .collect::<Result<Vec<_>, _>>()?;
                }
                result.eval(&state, program, fuel)?
            }
        })
    }
}
fn join(args: &[Expr]) -> String {
    args.iter().map(Expr::source).collect::<Vec<_>>().join(",")
}
fn boxed(e: Expr) -> Box<Expr> {
    Box::new(e)
}
fn expression(r: &mut Random, ty: Type, parameters: &[Type], depth: usize, helper: bool) -> Expr {
    if depth == 0 || r.pick(5) == 0 {
        let positions = parameters
            .iter()
            .enumerate()
            .filter_map(|(i, t)| (*t == ty).then_some(i))
            .collect::<Vec<_>>();
        if !positions.is_empty() && r.pick(2) == 0 {
            return Expr::Param(positions[r.pick(positions.len())]);
        }
        return match ty {
            Type::I64 => Expr::Int(r.integer()),
            Type::Bool => Expr::Bool(r.pick(2) == 0),
            Type::U64
            | Type::U128
            | Type::F64
            | Type::Array(_)
            | Type::Buffer
            | Type::Bytes
            | Type::MapI64
            | Type::MapBytes
            | Type::Record(..)
            | Type::MapRecord(..) => {
                unreachable!("expr-v3 generator only requests scalar types")
            }
        };
    }
    match r.pick(5) {
        0 if ty == Type::I64 => Expr::Binary(
            r.pick(4),
            boxed(expression(r, ty, parameters, depth - 1, helper)),
            boxed(expression(r, ty, parameters, depth - 1, helper)),
        ),
        0 | 1 if ty == Type::Bool => Expr::Compare(
            r.pick(6),
            boxed(expression(r, Type::I64, parameters, depth - 1, helper)),
            boxed(expression(r, Type::I64, parameters, depth - 1, helper)),
        ),
        1 if ty == Type::I64 => {
            if helper && r.pick(2) == 0 {
                Expr::Call(
                    1,
                    vec![
                        expression(r, ty, parameters, depth - 1, helper),
                        expression(r, ty, parameters, depth - 1, helper),
                    ],
                )
            } else {
                Expr::Call(2, vec![Expr::Int(r.pick(6) as i64)])
            }
        }
        2 => {
            // Counter state strictly decreases; other state may wrap and contains a bool.
            let init = vec![
                Expr::Int(r.pick(4) as i64),
                expression(r, Type::I64, parameters, depth - 1, helper),
                expression(r, Type::Bool, parameters, depth - 1, helper),
            ];
            let state = [Type::I64, Type::I64, Type::Bool];
            Expr::Loop(
                init,
                boxed(Expr::Compare(4, boxed(Expr::Param(0)), boxed(Expr::Int(0)))),
                vec![
                    Expr::Binary(1, boxed(Expr::Param(0)), boxed(Expr::Int(1))),
                    expression(r, Type::I64, &state, depth - 1, helper),
                    expression(r, Type::Bool, &state, depth - 1, helper),
                ],
                boxed(expression(r, ty, &state, depth - 1, helper)),
            )
        }
        _ => Expr::If(
            boxed(expression(r, Type::Bool, parameters, depth - 1, helper)),
            boxed(expression(r, ty, parameters, depth - 1, helper)),
            boxed(expression(r, ty, parameters, depth - 1, helper)),
        ),
    }
}
pub struct Case {
    pub seed: u64,
    pub source: String,
    functions: Vec<Expr>,
    pub arguments: Vec<Vec<i64>>,
}
impl Case {
    pub fn new(seed: u64) -> Self {
        let mut r = Random(seed);
        let root = expression(&mut r, Type::I64, &[Type::I64, Type::I64], 3, true);
        let helper = expression(&mut r, Type::I64, &[Type::I64, Type::I64], 3, false);
        let recursive = Expr::If(
            boxed(Expr::Compare(3, boxed(Expr::Param(0)), boxed(Expr::Int(0)))),
            boxed(Expr::Int(r.integer())),
            boxed(Expr::Binary(
                r.pick(3),
                boxed(Expr::Param(0)),
                boxed(Expr::Call(
                    2,
                    vec![Expr::Binary(1, boxed(Expr::Param(0)), boxed(Expr::Int(1)))],
                )),
            )),
        );
        let functions = vec![root, helper, recursive];
        let source = functions
            .iter()
            .zip([2, 2, 1])
            .map(|(f, n)| format!("{n}={}", f.source()))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        Self {
            seed,
            source,
            functions,
            arguments: vec![
                vec![0, 0],
                vec![i64::MIN, -1],
                vec![i64::MAX, 1],
                vec![r.integer(), r.integer()],
            ],
        }
    }
    pub fn expected(&self, args: &[i64]) -> Result<i64, &'static str> {
        self.functions[0]
            .eval(
                &args.iter().copied().map(Value::Int).collect::<Vec<_>>(),
                &self.functions,
                &mut 50000,
            )
            .map(Value::integer)
    }
    pub fn expected_function(&self, index: usize, args: &[i64]) -> Result<i64, &'static str> {
        self.functions[index]
            .eval(
                &args.iter().copied().map(Value::Int).collect::<Vec<_>>(),
                &self.functions,
                &mut 50000,
            )
            .map(Value::integer)
    }
    pub fn trees(&self) -> &[Expr] {
        &self.functions
    }
}
