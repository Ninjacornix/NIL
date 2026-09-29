use crate::syntax::*;
use nil_hir::{BinaryOp, Diagnostic, Phase, Span, Type};

pub const MAX_SOURCE_BYTES: usize = 1_048_576;

#[derive(Clone, Copy, Debug)]
struct Token<'a> {
    text: &'a str,
    span: Span,
}
struct Line<'a> {
    tokens: Vec<Token<'a>>,
    span: Span,
}

fn error(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E001", Phase::Parse, Some(span), message)
}

// Byte offsets remain valid for UTF-8: only ASCII separators split tokens.
fn lex(source: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut offset = 0;
    for raw in source.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let mut tokens = Vec::new();
        let mut start = 0;
        for (index, byte) in line.bytes().enumerate() {
            if byte == b' ' || byte == b'\t' {
                if start < index {
                    tokens.push(Token {
                        text: &line[start..index],
                        span: Span {
                            start: offset + start,
                            end: offset + index,
                        },
                    });
                }
                start = index + 1;
            }
        }
        if start < line.len() {
            tokens.push(Token {
                text: &line[start..],
                span: Span {
                    start: offset + start,
                    end: offset + line.len(),
                },
            });
        }
        if !tokens.is_empty() {
            lines.push(Line {
                span: Span {
                    start: tokens[0].span.start,
                    end: tokens.last().unwrap().span.end,
                },
                tokens,
            });
        }
        offset += raw.len();
    }
    lines
}

fn decimal(text: &str) -> bool {
    text == "0"
        || (text
            .as_bytes()
            .first()
            .is_some_and(|b| matches!(b, b'1'..=b'9'))
            && text.bytes().all(|b| b.is_ascii_digit()))
}
fn id(token: Token<'_>) -> Result<u32, Diagnostic> {
    if decimal(token.text) {
        if let Ok(value) = token.text.parse() {
            return Ok(value);
        }
    }
    Err(error(token.span, "expected canonical u32 ID"))
}
fn integer(token: Token<'_>) -> Result<i64, Diagnostic> {
    let magnitude = token.text.strip_prefix('-').unwrap_or(token.text);
    if decimal(magnitude) && token.text != "-0" {
        if let Ok(value) = token.text.parse() {
            return Ok(value);
        }
    }
    Err(error(token.span, "expected canonical i64 constant"))
}
fn ty(token: Token<'_>) -> Result<Type, Diagnostic> {
    if token.text == "i64" {
        Ok(Type::I64)
    } else {
        Err(
            Diagnostic::new("E002", Phase::Parse, Some(token.span), "unsupported type")
                .mismatch("i64", token.text),
        )
    }
}
fn arity(line: &Line<'_>, count: usize) -> Result<(), Diagnostic> {
    if line.tokens.len() == count {
        Ok(())
    } else {
        Err(error(line.span, "wrong directive token count").mismatch(count, line.tokens.len()))
    }
}

pub fn parse_lines(source: &str) -> Result<Module, Diagnostic> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(Diagnostic::new(
            "E008",
            Phase::Parse,
            None,
            "source exceeds 1 MiB limit",
        ));
    }
    let lines = lex(source);
    let eof = Span {
        start: source.len(),
        end: source.len(),
    };
    let mut cursor = 0;
    let mut functions = Vec::new();
    while cursor < lines.len() {
        let header = &lines[cursor];
        let t = &header.tokens;
        if t[0].text != "fn" || t.len() < 4 {
            return Err(error(header.span, "expected function signature"));
        }
        let arrow = t
            .iter()
            .position(|t| t.text == "->")
            .ok_or_else(|| error(header.span, "missing -> in signature"))?;
        if arrow < 2 || arrow + 2 != t.len() {
            return Err(error(
                header.span,
                "expected fn ID parameter-types -> result-type",
            ));
        }
        let label = id(t[1])?;
        let parameters = t[2..arrow]
            .iter()
            .map(|t| ty(*t))
            .collect::<Result<_, _>>()?;
        let result_type = ty(t[arrow + 1])?;
        cursor += 1;
        let mut instructions = Vec::new();
        let mut result = None;
        loop {
            let line = lines.get(cursor).ok_or_else(|| error(eof, "missing end"))?;
            let t = &line.tokens;
            cursor += 1;
            if t[0].text == "end" {
                arity(line, 1)?;
                break;
            }
            if result.is_some() {
                return Err(error(line.span, "only end may follow ret"));
            }
            let kind = match t[0].text {
                "ret" => {
                    arity(line, 2)?;
                    result = Some((id(t[1])?, line.span));
                    continue;
                }
                "const" => {
                    arity(line, 2)?;
                    InstructionKind::Constant(integer(t[1])?)
                }
                "add" | "sub" | "mul" | "div" => {
                    arity(line, 3)?;
                    let op = match t[0].text {
                        "add" => BinaryOp::Add,
                        "sub" => BinaryOp::Sub,
                        "mul" => BinaryOp::Mul,
                        _ => BinaryOp::Div,
                    };
                    InstructionKind::Binary(op, id(t[1])?, id(t[2])?)
                }
                "call" => {
                    if t.len() < 2 {
                        return Err(error(line.span, "call requires function ID"));
                    }
                    InstructionKind::Call(
                        id(t[1])?,
                        t[2..].iter().map(|t| id(*t)).collect::<Result<_, _>>()?,
                    )
                }
                _ => return Err(error(t[0].span, "unknown instruction")),
            };
            instructions.push(Instruction {
                kind,
                span: line.span,
            });
        }
        let (result, return_span) =
            result.ok_or_else(|| error(header.span, "function requires ret"))?;
        functions.push(Function {
            label,
            span: header.span,
            parameters,
            result_type,
            instructions,
            result,
            return_span,
        });
    }
    if functions.is_empty() {
        return Err(error(eof, "program requires a function"));
    }
    Ok(Module { functions })
}

/// Parse the default source profile (`expr-v0`).
pub fn parse(source: &str) -> Result<Module, Diagnostic> {
    crate::expr::parse(source)
}

/// Parse a source file using the selected, versioned syntax profile.
pub fn parse_with_profile(
    source: &str,
    profile: crate::SourceProfile,
) -> Result<Module, Diagnostic> {
    match profile {
        crate::SourceProfile::ExprV0 => parse(source),
        crate::SourceProfile::LinesV0 => parse_lines(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lexer_preserves_utf8_byte_offsets_and_crlf() {
        let lines = lex("\tfn 0 -> i64\r\n  λ\n");
        assert_eq!(lines[0].tokens[0].span, Span { start: 1, end: 3 });
        assert_eq!(lines[1].tokens[0].text, "λ");
        assert_eq!(lines[1].tokens[0].span, Span { start: 16, end: 18 });
    }
}
