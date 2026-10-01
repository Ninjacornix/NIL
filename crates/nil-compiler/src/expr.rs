//! Experimental expression source profile. It lowers to the existing lines AST/HIR.
use crate::{parser::MAX_SOURCE_BYTES, syntax};
use nil_hir::{BinaryOp, CompareOp, Diagnostic, Phase, Span, Type};
use std::collections::BTreeMap;

const MAX_EXPRESSION_DEPTH: usize = 128;

#[derive(Clone, Copy, Debug)]
struct Token<'a> {
    text: &'a str,
    span: Span,
}

fn error(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E001", Phase::Parse, Some(span), message)
}

fn scan(line: &str, offset: usize) -> Result<Vec<Token<'_>>, Diagnostic> {
    scan_profile(line, offset, false)
}
fn scan_profile(line: &str, offset: usize, typed: bool) -> Result<Vec<Token<'_>>, Diagnostic> {
    let mut tokens = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if matches!(bytes[index], b' ' | b'\t') {
            index += 1;
            continue;
        }
        let start = index;
        if bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_' {
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
        } else if bytes
            .get(index..index + 2)
            .is_some_and(|pair| [b"->".as_slice(), b"<=", b">=", b"==", b"!="].contains(&pair))
        {
            index += 2;
        } else if b"(),:=+-*/<>?;@".contains(&bytes[index])
            || (typed && b"[]#".contains(&bytes[index]))
        {
            index += 1;
        } else {
            return Err(error(
                Span {
                    start: offset + index,
                    end: offset + index + line[index..].chars().next().unwrap().len_utf8(),
                },
                "invalid character in expression profile",
            ));
        }
        tokens.push(Token {
            text: &line[start..index],
            span: Span {
                start: offset + start,
                end: offset + index,
            },
        });
    }
    Ok(tokens)
}

fn canonical_u32(text: &str) -> Option<u32> {
    if canonical_decimal(text) {
        text.parse().ok()
    } else {
        None
    }
}

fn canonical_decimal(text: &str) -> bool {
    text == "0"
        || (text
            .as_bytes()
            .first()
            .is_some_and(|b| b.is_ascii_digit() && *b != b'0')
            && text.bytes().all(|b| b.is_ascii_digit()))
}

fn function_label(token: Token<'_>) -> Result<u32, Diagnostic> {
    token
        .text
        .strip_prefix('f')
        .and_then(canonical_u32)
        .ok_or_else(|| error(token.span, "expected canonical function name f<ID>"))
}

fn identifier(text: &str) -> bool {
    text.as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
        && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn compact_label(token: Token<'_>) -> Result<u32, Diagnostic> {
    if !token.text.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err(error(token.span, "expected lowercase function reference"));
    }
    token
        .text
        .bytes()
        .try_fold(0u32, |id, b| {
            id.checked_mul(26)
                .and_then(|id| id.checked_add(u32::from(b - b'a') + 1))
        })
        .and_then(|id| id.checked_sub(1))
        .ok_or_else(|| error(token.span, "function reference exceeds u32"))
}

struct ExprParser<'a> {
    tokens: Vec<Token<'a>>,
    cursor: usize,
    compact: bool,
    positional: bool,
    symbolic_loop: bool,
    typed: bool,
    arity: Option<usize>,
    base: Option<u32>,
    end: usize,
    parameters: BTreeMap<&'a str, u32>,
    instructions: Vec<syntax::Instruction>,
}

impl<'a> ExprParser<'a> {
    fn parameter_count(&self) -> usize {
        self.arity.unwrap_or(self.parameters.len())
    }

    fn peek(&self) -> Option<Token<'a>> {
        self.tokens.get(self.cursor).copied()
    }

    fn next(&mut self) -> Option<Token<'a>> {
        let token = self.peek()?;
        self.cursor += 1;
        Some(token)
    }

    fn here(&self) -> Span {
        self.peek().map_or(
            Span {
                start: self.end,
                end: self.end,
            },
            |token| token.span,
        )
    }

    fn expect(&mut self, expected: &str) -> Result<Token<'a>, Diagnostic> {
        let token = self
            .next()
            .ok_or_else(|| error(self.here(), format!("expected {expected}")))?;
        if token.text != expected {
            return Err(error(token.span, format!("expected {expected}")));
        }
        Ok(token)
    }

    fn check_type(&mut self) -> Result<(), Diagnostic> {
        let token = self
            .next()
            .ok_or_else(|| error(self.here(), "expected i64 type"))?;
        if token.text != "i64" {
            return Err(Diagnostic::new(
                "E002",
                Phase::Parse,
                Some(token.span),
                "unsupported type",
            )
            .mismatch("i64", token.text));
        }
        Ok(())
    }

    fn emit(&mut self, kind: syntax::InstructionKind, span: Span) -> u32 {
        let id =
            self.base.unwrap_or(self.parameter_count() as u32) + self.instructions.len() as u32;
        self.instructions.push(syntax::Instruction { kind, span });
        id
    }

    fn signature_type(&mut self) -> Result<Type, Diagnostic> {
        let token = self
            .next()
            .ok_or_else(|| error(self.here(), "expected type"))?;
        match token.text {
            "i" => Ok(Type::I64),
            "b" => Ok(Type::Bool),
            text => canonical_u32(text)
                .filter(|n| *n as usize <= nil_hir::MAX_ARRAY_LEN)
                .map(|n| Type::Array(n as usize))
                .ok_or_else(|| {
                    Diagnostic::new(
                        "E002",
                        Phase::Parse,
                        Some(token.span),
                        "expected i, b or array length 0..256",
                    )
                }),
        }
    }

    fn array(&mut self, span: Span, depth: usize) -> Result<u32, Diagnostic> {
        if self.peek().is_some_and(|t| t.text == "]") {
            self.next();
            return Ok(self.emit(syntax::InstructionKind::Array(vec![]), span));
        }
        let first = self.expression(0, depth + 1)?;
        if self.peek().is_some_and(|t| t.text == ";") {
            self.next();
            let length = self
                .next()
                .ok_or_else(|| error(self.here(), "expected repeat length"))?;
            let len = canonical_u32(length.text)
                .filter(|n| (2..=nil_hir::MAX_ARRAY_LEN as u32).contains(n))
                .ok_or_else(|| error(length.span, "repeat length must be 2..256"))?;
            self.expect("]")?;
            return Ok(self.emit(syntax::InstructionKind::Repeat(first, len as usize), span));
        }
        let mut values = vec![first];
        while self.peek().is_some_and(|t| t.text == ",") {
            self.next();
            if values.len() == nil_hir::MAX_ARRAY_LEN {
                return Err(error(self.here(), "array length exceeds 256"));
            }
            values.push(self.expression(0, depth + 1)?);
        }
        self.expect("]")?;
        Ok(self.emit(syntax::InstructionKind::Array(values), span))
    }

    fn postfix(&mut self, depth: usize) -> Result<u32, Diagnostic> {
        let mut value = self.primary(depth)?;
        while self.typed && self.peek().is_some_and(|t| t.text == "[") {
            let open = self.next().unwrap();
            let index = self.expression(0, depth + 1)?;
            let kind = if self.peek().is_some_and(|t| t.text == ":") {
                self.next();
                syntax::InstructionKind::Replace(value, index, self.expression(0, depth + 1)?)
            } else {
                syntax::InstructionKind::Index(value, index)
            };
            let close = self.expect("]")?;
            value = self.emit(
                kind,
                Span {
                    start: open.span.start,
                    end: close.span.end,
                },
            );
        }
        Ok(value)
    }

    fn primary(&mut self, depth: usize) -> Result<u32, Diagnostic> {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(error(self.here(), "expression nesting limit exceeded"));
        }
        let token = self
            .next()
            .ok_or_else(|| error(self.here(), "expected expression"))?;
        match token.text {
            "[" if self.typed => self.array(token.span, depth),
            "#" if self.typed => {
                let array = self.postfix(depth + 1)?;
                Ok(self.emit(syntax::InstructionKind::Length(array), token.span))
            }
            "true" | "false" => Ok(self.emit(
                syntax::InstructionKind::Boolean(token.text == "true"),
                token.span,
            )),
            "loop" if !self.symbolic_loop => self.loop_expression(token.span, depth),
            "@" if self.symbolic_loop => self.loop_expression(token.span, depth),
            "loop" | "@" => Err(error(token.span, "wrong loop spelling for source profile")),
            "(" => {
                let value = self.expression(0, depth + 1)?;
                self.expect(")")?;
                Ok(value)
            }
            "-" => {
                let number = self
                    .next()
                    .ok_or_else(|| error(self.here(), "expected integer after -"))?;
                let value = canonical_decimal(number.text)
                    .then(|| format!("-{}", number.text))
                    .filter(|_| number.text != "0")
                    .and_then(|text| text.parse::<i64>().ok())
                    .ok_or_else(|| error(number.span, "expected canonical negative i64 literal"))?;
                Ok(self.emit(
                    syntax::InstructionKind::Constant(value),
                    Span {
                        start: token.span.start,
                        end: number.span.end,
                    },
                ))
            }
            _ if token.text.bytes().all(|b| b.is_ascii_digit()) => {
                let value = token
                    .text
                    .parse::<i64>()
                    .ok()
                    .filter(|_| canonical_decimal(token.text))
                    .ok_or_else(|| error(token.span, "expected canonical i64 literal"))?;
                Ok(self.emit(syntax::InstructionKind::Constant(value), token.span))
            }
            _ if (self.compact
                && identifier(token.text)
                && self.peek().is_some_and(|t| t.text == "("))
                || (!self.compact
                    && token.text.starts_with('f')
                    && token.text.as_bytes()[1..]
                        .first()
                        .is_some_and(u8::is_ascii_digit)) =>
            {
                let label = if self.compact {
                    compact_label(token)?
                } else {
                    function_label(token)?
                };
                self.expect("(")?;
                let mut arguments = Vec::new();
                if self.peek().is_some_and(|token| token.text != ")") {
                    loop {
                        arguments.push(self.expression(0, depth + 1)?);
                        if self.peek().is_some_and(|token| token.text == ",") {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                let close = self.expect(")")?;
                Ok(self.emit(
                    syntax::InstructionKind::Call(label, arguments),
                    Span {
                        start: token.span.start,
                        end: close.span.end,
                    },
                ))
            }
            _ if identifier(token.text) => {
                let parameter = if self.positional {
                    compact_label(token)
                        .ok()
                        .filter(|id| (*id as usize) < self.parameter_count())
                } else {
                    self.parameters.get(token.text).copied()
                };
                parameter.ok_or_else(|| {
                    Diagnostic::new(
                        "E005",
                        Phase::Check,
                        Some(token.span),
                        format!("unknown parameter {}", token.text),
                    )
                })
            }
            _ => Err(error(token.span, "expected expression")),
        }
    }

    fn expression(&mut self, minimum: u8, depth: usize) -> Result<u32, Diagnostic> {
        let mut lhs = self.postfix(depth)?;
        while let Some(token) = self.peek() {
            let comparison = match token.text {
                "==" => Some(CompareOp::Eq),
                "!=" => Some(CompareOp::Ne),
                "<" => Some(CompareOp::Lt),
                "<=" => Some(CompareOp::Le),
                ">" => Some(CompareOp::Gt),
                ">=" => Some(CompareOp::Ge),
                _ => None,
            };
            if let Some(op) = comparison {
                if minimum > 1 {
                    break;
                }
                self.next();
                let rhs = self.expression(2, depth + 1)?;
                lhs = self.emit(syntax::InstructionKind::Compare(op, lhs, rhs), token.span);
                continue;
            }
            let (precedence, op) = match token.text {
                "+" => (2, BinaryOp::Add),
                "-" => (2, BinaryOp::Sub),
                "*" => (3, BinaryOp::Mul),
                "/" => (3, BinaryOp::Div),
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.next();
            let rhs = self.expression(precedence + 1, depth + 1)?;
            lhs = self.emit(syntax::InstructionKind::Binary(op, lhs, rhs), token.span);
        }
        if minimum == 0 && self.peek().is_some_and(|t| t.text == "?") {
            let question = self.next().unwrap();
            let yes = self.branch_region(depth)?;
            self.expect(":")?;
            let no = self.branch_region(depth)?;
            lhs = self.emit(syntax::InstructionKind::If(lhs, yes, no), question.span);
        }
        Ok(lhs)
    }

    fn branch_region(&mut self, depth: usize) -> Result<syntax::Region, Diagnostic> {
        let base =
            self.base.unwrap_or(self.parameter_count() as u32) + self.instructions.len() as u32;
        let saved = std::mem::take(&mut self.instructions);
        let saved_base = self.base.replace(base);
        let result = self.expression(0, depth + 1)?;
        let instructions = std::mem::replace(&mut self.instructions, saved);
        self.base = saved_base;
        Ok(syntax::Region {
            instructions,
            results: vec![result],
        })
    }

    fn expression_list(&mut self, terminator: &str, depth: usize) -> Result<Vec<u32>, Diagnostic> {
        let mut results = Vec::new();
        if self.peek().is_some_and(|t| t.text != terminator) {
            loop {
                results.push(self.expression(0, depth + 1)?);
                if self.peek().is_some_and(|t| t.text == ",") {
                    self.next();
                } else {
                    break;
                }
            }
        }
        Ok(results)
    }

    fn state_region(
        &mut self,
        arity: usize,
        terminator: &str,
        depth: usize,
        list: bool,
    ) -> Result<syntax::Region, Diagnostic> {
        let saved = std::mem::take(&mut self.instructions);
        let parameters = std::mem::take(&mut self.parameters);
        let saved_arity = self.arity.replace(arity);
        let saved_positional = std::mem::replace(&mut self.positional, true);
        let saved_base = self.base.take();
        let results = if list {
            self.expression_list(terminator, depth)?
        } else {
            vec![self.expression(0, depth + 1)?]
        };
        let instructions = std::mem::replace(&mut self.instructions, saved);
        self.parameters = parameters;
        self.arity = saved_arity;
        self.positional = saved_positional;
        self.base = saved_base;
        Ok(syntax::Region {
            instructions,
            results,
        })
    }

    fn loop_expression(&mut self, span: Span, depth: usize) -> Result<u32, Diagnostic> {
        self.expect("(")?;
        let initial = self.expression_list(";", depth)?;
        self.expect(";")?;
        let condition = self.state_region(initial.len(), ";", depth, false)?;
        self.expect(";")?;
        let body = self.state_region(initial.len(), ";", depth, true)?;
        self.expect(";")?;
        let finish = self.state_region(initial.len(), ")", depth, false)?;
        self.expect(")")?;
        Ok(self.emit(
            syntax::InstructionKind::Loop(initial, condition, body, finish),
            span,
        ))
    }

    fn function(mut self, implicit_label: u32) -> Result<syntax::Function, Diagnostic> {
        let name = self
            .next()
            .ok_or_else(|| error(self.here(), "expected function"))?;
        let label = if self.compact {
            self.cursor = 0;
            implicit_label
        } else {
            let label = function_label(name)?;
            self.expect("(")?;
            label
        };
        let mut typed_parameters = None;
        if self.typed && name.text == "(" {
            self.expect("(")?;
            let mut types = vec![];
            if self.peek().is_some_and(|t| t.text != ")") {
                loop {
                    if types.len() == 4096 {
                        return Err(error(self.here(), "too many parameters"));
                    }
                    types.push(self.signature_type()?);
                    if self.peek().is_some_and(|t| t.text == ",") {
                        self.next();
                    } else {
                        break;
                    }
                }
            }
            self.expect(")")?;
            if types.iter().all(|ty| *ty == Type::I64) {
                return Err(error(name.span, "use numeric arity for all-i64 parameters"));
            }
            self.arity = Some(types.len());
            typed_parameters = Some(types);
        } else if self.positional {
            self.arity = Some(if name.text == "=" || (self.typed && name.text == ":") {
                0
            } else {
                let arity = canonical_u32(name.text)
                    .filter(|n| (1..=4096).contains(n))
                    .ok_or_else(|| {
                        error(
                            name.span,
                            "expected parameter count 1..4096 or = for zero parameters",
                        )
                    })?;
                self.next();
                arity as usize
            });
        }
        let terminator = if self.compact { "=" } else { ")" };
        if !self.positional && self.peek().is_some_and(|token| token.text != terminator) {
            loop {
                let parameter = self
                    .next()
                    .ok_or_else(|| error(self.here(), "expected parameter"))?;
                if !identifier(parameter.text)
                    || matches!(parameter.text, "loop" | "true" | "false")
                    || (!self.compact
                        && parameter.text.starts_with('f')
                        && parameter.text.as_bytes()[1..]
                            .first()
                            .is_some_and(u8::is_ascii_digit))
                {
                    return Err(error(parameter.span, "expected parameter name"));
                }
                if self.parameters.contains_key(parameter.text) {
                    return Err(error(parameter.span, "duplicate parameter name"));
                }
                self.parameters
                    .insert(parameter.text, self.parameters.len() as u32);
                if !self.compact && self.peek().is_some_and(|token| token.text == ":") {
                    self.next();
                    self.check_type()?;
                }
                if self.peek().is_some_and(|token| token.text == ",") {
                    self.next();
                } else {
                    break;
                }
            }
        }
        if !self.compact {
            self.expect(")")?;
        }
        if !self.compact && self.peek().is_some_and(|token| token.text == "->") {
            self.next();
            self.check_type()?;
        }
        let mut result_type = Type::I64;
        if self.typed && self.peek().is_some_and(|t| t.text == ":") {
            let colon = self.next().unwrap();
            result_type = self.signature_type()?;
            if result_type == Type::I64 {
                return Err(error(colon.span, "omit the default i64 result type"));
            }
        }
        let equal = self.expect("=")?;
        let result = self.expression(0, 0)?;
        if let Some(token) = self.peek() {
            return Err(error(token.span, "unexpected token after expression"));
        }
        Ok(syntax::Function {
            label,
            span: Span {
                start: name.span.start,
                end: equal.span.end,
            },
            parameters: typed_parameters.unwrap_or_else(|| vec![Type::I64; self.parameter_count()]),
            result_type,
            instructions: self.instructions,
            result,
            return_span: Span {
                start: equal.span.end,
                end: self.end,
            },
        })
    }
}

pub fn parse(source: &str) -> Result<syntax::Module, Diagnostic> {
    parse_profile(source, false, false, false)
}

pub fn parse_compact(source: &str) -> Result<syntax::Module, Diagnostic> {
    parse_profile(source, true, false, false)
}

pub fn parse_positional(source: &str) -> Result<syntax::Module, Diagnostic> {
    parse_profile(source, true, true, false)
}

pub fn parse_typed(source: &str) -> Result<syntax::Module, Diagnostic> {
    parse_profile(source, true, true, true)
}

fn parse_profile(
    source: &str,
    compact: bool,
    positional: bool,
    typed: bool,
) -> Result<syntax::Module, Diagnostic> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(Diagnostic::new(
            "E008",
            Phase::Parse,
            None,
            "source exceeds 1 MiB limit",
        ));
    }
    let mut functions = Vec::new();
    let mut offset = 0;
    for raw in source.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let tokens = if typed {
            scan_profile(line, offset, true)?
        } else {
            scan(line, offset)?
        };
        if !tokens.is_empty() {
            functions.push(
                ExprParser {
                    tokens,
                    cursor: 0,
                    compact,
                    positional,
                    symbolic_loop: compact && positional,
                    typed,
                    arity: None,
                    base: None,
                    end: offset + line.len(),
                    parameters: BTreeMap::new(),
                    instructions: Vec::new(),
                }
                .function(functions.len() as u32)?,
            );
        }
        offset += raw.len();
    }
    if functions.is_empty() {
        return Err(error(
            Span {
                start: source.len(),
                end: source.len(),
            },
            "program requires a function",
        ));
    }
    Ok(syntax::Module { functions })
}

#[cfg(test)]
mod lexer_tests {
    use super::*;

    #[test]
    fn lexer_emits_punctuation_identifiers_and_byte_spans() {
        let line = "f12(x:i64)=(x+3)/2";
        let tokens = scan(line, 7).unwrap();
        assert_eq!(
            tokens.iter().map(|token| token.text).collect::<Vec<_>>(),
            [
                "f12", "(", "x", ":", "i64", ")", "=", "(", "x", "+", "3", ")", "/", "2"
            ]
        );
        assert_eq!(tokens[0].span, Span { start: 7, end: 10 });
        assert_eq!(tokens[9].span, Span { start: 20, end: 21 });
    }

    #[test]
    fn invalid_unicode_reports_the_full_utf8_byte_span() {
        let error = scan("λ", 11).unwrap_err();
        assert_eq!(error.span, Some(Span { start: 11, end: 13 }));
    }

    #[test]
    fn comments_and_bare_carriage_returns_are_rejected() {
        for source in ["f0(x)=x # comment", "f0(x)=x\rf1(y)=y"] {
            assert_eq!(parse(source).unwrap_err().code, "E001");
        }
    }
}
