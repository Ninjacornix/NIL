//! Experimental expression source profile. It lowers to the existing lines AST/HIR.
use crate::{parser::MAX_SOURCE_BYTES, syntax};
use nil_hir::{BinaryOp, Diagnostic, Phase, Span, Type};
use std::collections::BTreeMap;

const MAX_EXPRESSION_DEPTH: usize = 128;

#[derive(Clone, Copy)]
struct Token<'a> {
    text: &'a str,
    span: Span,
}

fn error(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E001", Phase::Parse, Some(span), message)
}

fn scan(line: &str, offset: usize) -> Result<Vec<Token<'_>>, Diagnostic> {
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
        } else if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'>') {
            index += 2;
        } else if b"(),:=+-*/".contains(&bytes[index]) {
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

struct ExprParser<'a> {
    tokens: Vec<Token<'a>>,
    cursor: usize,
    end: usize,
    parameters: BTreeMap<&'a str, u32>,
    instructions: Vec<syntax::Instruction>,
}

impl<'a> ExprParser<'a> {
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
        let id = self.parameters.len() as u32 + self.instructions.len() as u32;
        self.instructions.push(syntax::Instruction { kind, span });
        id
    }

    fn primary(&mut self, depth: usize) -> Result<u32, Diagnostic> {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(error(self.here(), "expression nesting limit exceeded"));
        }
        let token = self
            .next()
            .ok_or_else(|| error(self.here(), "expected expression"))?;
        match token.text {
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
            _ if token.text.starts_with('f')
                && token.text.as_bytes()[1..]
                    .first()
                    .is_some_and(u8::is_ascii_digit) =>
            {
                let label = function_label(token)?;
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
                self.parameters.get(token.text).copied().ok_or_else(|| {
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
        let mut lhs = self.primary(depth)?;
        while let Some(token) = self.peek() {
            let (precedence, op) = match token.text {
                "+" => (1, BinaryOp::Add),
                "-" => (1, BinaryOp::Sub),
                "*" => (2, BinaryOp::Mul),
                "/" => (2, BinaryOp::Div),
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.next();
            let rhs = self.expression(precedence + 1, depth + 1)?;
            lhs = self.emit(syntax::InstructionKind::Binary(op, lhs, rhs), token.span);
        }
        Ok(lhs)
    }

    fn function(mut self) -> Result<syntax::Function, Diagnostic> {
        let name = self
            .next()
            .ok_or_else(|| error(self.here(), "expected function"))?;
        let label = function_label(name)?;
        self.expect("(")?;
        if self.peek().is_some_and(|token| token.text != ")") {
            loop {
                let parameter = self
                    .next()
                    .ok_or_else(|| error(self.here(), "expected parameter"))?;
                if !identifier(parameter.text)
                    || (parameter.text.starts_with('f')
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
                if self.peek().is_some_and(|token| token.text == ":") {
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
        self.expect(")")?;
        if self.peek().is_some_and(|token| token.text == "->") {
            self.next();
            self.check_type()?;
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
            parameters: vec![Type::I64; self.parameters.len()],
            result_type: Type::I64,
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
        let tokens = scan(line, offset)?;
        if !tokens.is_empty() {
            functions.push(
                ExprParser {
                    tokens,
                    cursor: 0,
                    end: offset + line.len(),
                    parameters: BTreeMap::new(),
                    instructions: Vec::new(),
                }
                .function()?,
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
