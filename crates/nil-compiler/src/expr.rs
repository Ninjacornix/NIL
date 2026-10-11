//! Experimental expression source profile. It lowers to the existing lines AST/HIR.
use crate::{parser::MAX_SOURCE_BYTES, syntax};
use nil_hir::{BinaryOp, CompareOp, Diagnostic, Phase, Span, Type, UnaryOp};
use std::collections::{BTreeMap, BTreeSet};

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
    scan_profile(line, offset, false, false)
}
fn scan_profile(
    line: &str,
    offset: usize,
    typed: bool,
    application: bool,
) -> Result<Vec<Token<'_>>, Diagnostic> {
    let mut tokens = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if matches!(bytes[index], b' ' | b'\t') {
            index += 1;
            continue;
        }
        let start = index;
        if application && bytes[index] == b'"' {
            index += 1;
            let mut closed = false;
            while index < bytes.len() {
                if bytes[index] == b'"' {
                    index += 1;
                    closed = true;
                    break;
                }
                if bytes[index] == b'\\' {
                    index += 1;
                }
                index += 1;
            }
            if !closed {
                return Err(error(
                    Span {
                        start: offset + start,
                        end: offset + line.len(),
                    },
                    "unterminated byte literal",
                ));
            }
        } else if application && bytes[index].is_ascii_digit() {
            let radix = bytes
                .get(index..index + 2)
                .is_some_and(|prefix| matches!(prefix, b"0x" | b"0b"));
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric()
                    || bytes[index] == b'.'
                    || (!radix
                        && matches!(bytes[index], b'+' | b'-')
                        && matches!(bytes[index - 1], b'e' | b'E')))
            {
                index += 1;
            }
        } else if bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_' {
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
        } else if bytes.get(index..index + 2).is_some_and(|pair| {
            [b"->".as_slice(), b"<=", b">=", b"==", b"!="].contains(&pair)
                || (application && [b"<<".as_slice(), b">>", b"&&", b"||"].contains(&pair))
        }) {
            index += 2;
        } else if b"(),:=+-*/<>?;@".contains(&bytes[index])
            || (typed && b"[]#".contains(&bytes[index]))
            || (application && b"!.{}&^%|~".contains(&bytes[index]))
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
    records: &'a [nil_hir::RecordDefinition],
    tokens: Vec<Token<'a>>,
    cursor: usize,
    compact: bool,
    positional: bool,
    symbolic_loop: bool,
    typed: bool,
    application: bool,
    arity: Option<usize>,
    base: Option<u32>,
    end: usize,
    parameters: BTreeMap<&'a str, u32>,
    locals: BTreeMap<&'a str, u32>,
    static_parameters: BTreeSet<u32>,
    instructions: Vec<syntax::Instruction>,
    // Fresh update tails can extend through parentheses, but never bindings or
    // region boundaries. These are parser bookkeeping, not semantic HIR values.
    chain_tails: BTreeSet<u32>,
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
        self.chain_tails.remove(&id);
        self.instructions.push(syntax::Instruction { kind, span });
        id
    }

    fn signature_type(&mut self) -> Result<Type, Diagnostic> {
        let token = self
            .next()
            .ok_or_else(|| error(self.here(), "expected type"))?;
        match token.text {
            "map" | "v" if self.application && self.peek().is_some_and(|t| t.text == "[") => {
                self.expect("[")?;
                let name = self
                    .next()
                    .ok_or_else(|| error(self.here(), "expected record type"))?;
                let value = self
                    .records
                    .iter()
                    .enumerate()
                    .find(|(_, r)| r.name == name.text)
                    .map(|(id, r)| Type::Record(id, r.fields.iter().map(|f| f.ty.slots()).sum()))
                    .ok_or_else(|| {
                        nil_hir::records::error(Some(name.span), "unknown record map type")
                    })?;
                self.expect("]")?;
                let Type::Record(id, slots) = value else {
                    return Err(nil_hir::records::error(
                        Some(token.span),
                        "record map type required",
                    ));
                };
                Ok(if token.text == "map" {
                    Type::MapRecord(id, slots)
                } else {
                    Type::RecordBuffer(id, slots)
                })
            }
            "i" => Ok(Type::I64),
            "u64" if self.application => Ok(Type::U64),
            "u128" if self.application => Ok(Type::U128),
            "f64" if self.application => Ok(Type::F64),
            "b" => Ok(Type::Bool),
            "m" if self.application => Ok(Type::MapI64),
            "t" if self.application => Ok(Type::MapBytes),
            "v" if self.application => Ok(Type::Buffer),
            "s" if self.application => Ok(Type::Bytes),
            text if self.application && text.as_bytes()[0].is_ascii_uppercase() => self
                .records
                .iter()
                .enumerate()
                .find(|(_, r)| r.name == text)
                .map(|(id, r)| Type::Record(id, r.fields.iter().map(|f| f.ty.slots()).sum()))
                .ok_or_else(|| nil_hir::records::error(Some(token.span), "unknown record type")),
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

    fn parameter_type(&mut self) -> Result<syntax::Parameter, Diagnostic> {
        if !self.application || !self.peek().is_some_and(|t| t.text == "[") {
            return self.signature_type().map(syntax::Parameter::Value);
        }
        self.expect("[")?;
        let mut parameters = vec![];
        if self.peek().is_some_and(|t| t.text != ":") {
            loop {
                if parameters.len() == 4096 {
                    return Err(error(self.here(), "too many callback parameters"));
                }
                parameters.push(self.signature_type()?);
                if !self.peek().is_some_and(|t| t.text == ",") {
                    break;
                }
                self.next();
            }
        }
        self.expect(":")?;
        let result = self.signature_type()?;
        self.expect("]")?;
        Ok(syntax::Parameter::Function(syntax::FunctionSignature {
            parameters,
            result,
        }))
    }
    fn function_reference(
        &mut self,
        marker: Token<'a>,
    ) -> Result<syntax::FunctionReference, Diagnostic> {
        use syntax::FunctionReference as R;
        if marker.text == "&" && self.peek().is_some_and(|t| t.text == "!") {
            self.next();
            if let Some(token) = self.peek().filter(|t| t.text.contains('.')) {
                let text = token.text;
                let span = token.span;
                self.next();
                let (id, op) = qualified(text, span)?;
                return Ok(R::Export(id, op));
            }
            self.expect("plugin")?;
            self.expect("(")?;
            let id = self
                .next()
                .ok_or_else(|| error(self.here(), "expected module ID"))?;
            let id = canonical_u32(id.text)
                .ok_or_else(|| error(id.span, "canonical module ID required"))?;
            self.expect(",")?;
            let op = self
                .next()
                .ok_or_else(|| error(self.here(), "expected export ID"))?;
            let op = canonical_u32(op.text)
                .ok_or_else(|| error(op.span, "canonical export ID required"))?;
            self.expect(")")?;
            return Ok(R::Export(id, op));
        }
        let token = self
            .next()
            .ok_or_else(|| error(self.here(), "expected function reference"))?;
        let label = compact_label(token)?;
        Ok(if marker.text == "&" {
            R::Local(label)
        } else {
            R::Parameter(label)
        })
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
        let first = self.instructions.len();
        let mut value = self.primary(depth)?;
        if self.application {
            return self.simultaneous_postfix(value, first, depth);
        }
        loop {
            match self.peek().map(|t| t.text) {
                Some("[") if self.typed => value = self.array_postfix(value, depth)?,
                Some("." | "{") if self.application => value = self.record_postfix(value, depth)?,
                _ => break,
            }
        }
        Ok(value)
    }
    fn replacement_ahead(&self) -> bool {
        match self.peek().map(|t| t.text) {
            Some("{") => return true,
            Some("[") => {}
            _ => return false,
        }
        let mut nesting = 0usize;
        let mut questions = 0usize;
        for token in self.tokens.iter().skip(self.cursor + 1) {
            match token.text {
                "(" | "[" | "{" => nesting += 1,
                "]" if nesting == 0 => return false,
                ")" | "]" | "}" => nesting = nesting.saturating_sub(1),
                "?" if nesting == 0 => questions += 1,
                ":" if nesting == 0 && questions == 0 => return true,
                ":" if nesting == 0 => questions -= 1,
                _ => {}
            }
        }
        false
    }
    fn flush_updates(&mut self, mut value: u32, pending: &mut Vec<syntax::Instruction>) -> u32 {
        for mut instruction in pending.drain(..) {
            match &mut instruction.kind {
                syntax::InstructionKind::Replace(receiver, ..)
                | syntax::InstructionKind::UpdateField(receiver, ..) => *receiver = value,
                _ => unreachable!("pending postfix is an update"),
            }
            value = self.emit(instruction.kind, instruction.span);
            self.chain_tails.insert(value);
        }
        value
    }
    fn simultaneous_postfix(
        &mut self,
        mut value: u32,
        first: usize,
        depth: usize,
    ) -> Result<u32, Diagnostic> {
        let mut pending = Vec::new();
        if self.replacement_ahead() {
            // A grouped fresh chain ends in contiguous update instructions.
            // Recover only that receiver chain; leave every operand instruction
            // in place. Bound/local values and captures are never fresh tails.
            let base = self.base.unwrap_or(self.parameter_count() as u32);
            while self.instructions.len() > first
                && value == base + self.instructions.len() as u32 - 1
                && self.chain_tails.remove(&value)
            {
                let instruction = self.instructions.pop().unwrap();
                value = match &instruction.kind {
                    syntax::InstructionKind::Replace(receiver, ..)
                    | syntax::InstructionKind::UpdateField(receiver, ..) => *receiver,
                    _ => unreachable!("marked chain tail is an update"),
                };
                pending.push(instruction);
            }
            pending.reverse();
        }
        loop {
            if self.replacement_ahead() {
                let open = self.next().unwrap();
                let (kind, span) = if open.text == "[" {
                    let index = self.expression(0, depth + 1)?;
                    self.expect(":")?;
                    let replacement = self.expression(0, depth + 1)?;
                    let close = self.expect("]")?;
                    (
                        syntax::InstructionKind::Replace(0, index, replacement),
                        Span {
                            start: open.span.start,
                            end: close.span.end,
                        },
                    )
                } else {
                    let field = self
                        .next()
                        .ok_or_else(|| error(self.here(), "expected field name"))?;
                    if !identifier(field.text) {
                        return Err(error(field.span, "expected field name"));
                    }
                    self.expect(":")?;
                    let replacement = self.expression(0, depth + 1)?;
                    self.expect("}")?;
                    (
                        syntax::InstructionKind::UpdateField(0, field.text.into(), replacement),
                        open.span,
                    )
                };
                pending.push(syntax::Instruction { kind, span });
                continue;
            }
            value = self.flush_updates(value, &mut pending);
            match self.peek().map(|t| t.text) {
                Some("[") => value = self.array_postfix(value, depth)?,
                Some(".") => value = self.record_postfix(value, depth)?,
                _ => break,
            }
        }
        Ok(value)
    }
    fn array_postfix(&mut self, value: u32, depth: usize) -> Result<u32, Diagnostic> {
        let open = self.next().unwrap();
        let index = self.expression(0, depth + 1)?;
        let kind = if self.peek().is_some_and(|t| t.text == ":") {
            self.next();
            syntax::InstructionKind::Replace(value, index, self.expression(0, depth + 1)?)
        } else {
            syntax::InstructionKind::Index(value, index)
        };
        let close = self.expect("]")?;
        Ok(self.emit(
            kind,
            Span {
                start: open.span.start,
                end: close.span.end,
            },
        ))
    }
    fn record_postfix(&mut self, value: u32, depth: usize) -> Result<u32, Diagnostic> {
        let open = self.next().unwrap();
        let field = self
            .next()
            .ok_or_else(|| error(self.here(), "expected field name"))?;
        if !identifier(field.text) {
            return Err(error(field.span, "expected field name"));
        }
        let kind = if open.text == "." {
            syntax::InstructionKind::Field(value, field.text.into())
        } else {
            self.expect(":")?;
            let replacement = self.expression(0, depth + 1)?;
            self.expect("}")?;
            syntax::InstructionKind::UpdateField(value, field.text.into(), replacement)
        };
        Ok(self.emit(kind, open.span))
    }
    fn record_constructor(&mut self, token: Token<'a>, depth: usize) -> Result<u32, Diagnostic> {
        let ty = self
            .records
            .iter()
            .enumerate()
            .find(|(_, r)| r.name == token.text)
            .map(|(id, r)| Type::Record(id, r.fields.iter().map(|f| f.ty.slots()).sum()))
            .ok_or_else(|| {
                nil_hir::records::error(Some(token.span), "unknown record constructor")
            })?;
        self.expect("(")?;
        let fields = self.expression_list(")", depth + 1)?;
        self.expect(")")?;
        Ok(self.emit(syntax::InstructionKind::Record(ty, fields), token.span))
    }
    fn record_map_constructor(&mut self, token: Token<'a>) -> Result<u32, Diagnostic> {
        self.expect("[")?;
        let ty = self.signature_type()?;
        let Type::Record(id, slots) = ty else {
            return Err(nil_hir::records::error(
                Some(token.span),
                "record map requires record type",
            ));
        };
        self.expect("]")?;
        self.expect("(")?;
        self.expect(")")?;
        Ok(self.emit(
            syntax::InstructionKind::RecordMap(Type::MapRecord(id, slots)),
            token.span,
        ))
    }

    fn plugin_call(&mut self, token: Token<'a>, depth: usize) -> Result<u32, Diagnostic> {
        self.expect("(")?;
        let id = self
            .next()
            .ok_or_else(|| error(token.span, "expected plugin id"))?;
        let id =
            canonical_u32(id.text).ok_or_else(|| error(id.span, "canonical plugin id required"))?;
        self.expect(",")?;
        let op = self
            .next()
            .ok_or_else(|| error(token.span, "expected operation id"))?;
        let op = canonical_u32(op.text)
            .ok_or_else(|| error(op.span, "canonical operation id required"))?;
        let arguments = if self.peek().is_some_and(|t| t.text == ",") {
            self.expect(",")?;
            self.expression_list(")", depth + 1)?
        } else {
            vec![]
        };
        self.expect(")")?;
        Ok(self.emit(
            syntax::InstructionKind::Plugin(id, op, arguments),
            token.span,
        ))
    }
    fn scalar_primitive(
        &mut self,
        token: Token<'a>,
        name: Token<'a>,
        depth: usize,
    ) -> Result<u32, Diagnostic> {
        let unary = match name.text {
            "abs" => Some(UnaryOp::Abs),
            "sqrt" => Some(UnaryOp::Sqrt),
            "floor" => Some(UnaryOp::Floor),
            "ceil" => Some(UnaryOp::Ceil),
            "trunc" => Some(UnaryOp::Trunc),
            "round" => Some(UnaryOp::Round),
            _ => None,
        };
        self.expect("(")?;
        let args = self.expression_list(")", depth + 1)?;
        let close = self.expect(")")?;
        let span = Span {
            start: token.span.start,
            end: close.span.end,
        };
        let count = if unary.is_some() { 1 } else { 2 };
        if args.len() != count {
            return Err(Diagnostic::new(
                "E006",
                Phase::Check,
                Some(span),
                "scalar operation arity mismatch",
            )
            .mismatch(count, args.len()));
        }
        let kind = if let Some(op) = unary {
            syntax::InstructionKind::Unary(op, args[0])
        } else {
            syntax::InstructionKind::Binary(
                if name.text == "min" {
                    BinaryOp::Min
                } else {
                    BinaryOp::Max
                },
                args[0],
                args[1],
            )
        };
        Ok(self.emit(kind, span))
    }
    fn intrinsic(&mut self, token: Token<'a>, depth: usize) -> Result<u32, Diagnostic> {
        let name = self
            .next()
            .ok_or_else(|| error(self.here(), "expected operation name"))?;
        if matches!(
            name.text,
            "abs" | "sqrt" | "floor" | "ceil" | "trunc" | "round" | "min" | "max"
        ) {
            return self.scalar_primitive(token, name, depth);
        }
        if name.text == "map" && self.peek().is_some_and(|t| t.text == "[") {
            return self.record_map_constructor(token);
        }
        if name.text == "buffer" && self.peek().is_some_and(|t| t.text == "[") {
            self.expect("[")?;
            let Type::Record(id, slots) = self.signature_type()? else {
                return Err(nil_hir::records::error(
                    Some(token.span),
                    "record buffer requires record type",
                ));
            };
            self.expect("]")?;
            self.expect("(")?;
            let length = self.expression(0, depth + 1)?;
            self.expect(",")?;
            let fill = self.expression(0, depth + 1)?;
            self.expect(")")?;
            return Ok(self.emit(
                syntax::InstructionKind::RecordBuffer(Type::RecordBuffer(id, slots), length, fill),
                token.span,
            ));
        }
        if name.text == "each" {
            self.expect("(")?;
            let input = self.expression(0, depth + 1)?;
            self.expect(",")?;
            let initial = self.expression_list(";", depth + 1)?;
            if initial.is_empty() || initial.len() > 4094 {
                return Err(error(name.span, "each requires 1..4094 state values"));
            }
            self.expect(";")?;
            let body = self.state_region(initial.len() + 2, ";", depth + 1, true)?;
            self.expect(";")?;
            let finish = self.state_region(initial.len(), ")", depth + 1, false)?;
            self.expect(")")?;
            return Ok(self.emit(
                syntax::InstructionKind::Each(input, initial, body, finish),
                token.span,
            ));
        }
        if name.text == "plugin" {
            return self.plugin_call(token, depth);
        }
        if nil_hir::Intrinsic::parse(name.text).is_none()
            && !matches!(name.text, "filter" | "fold" | "push" | "swap" | "pop")
            && !name.text.contains('.')
        {
            return Err(error(name.span, "unknown application operation"));
        }
        self.expect("(")?;
        let arguments = self.expression_list(")", depth + 1)?;
        self.expect(")")?;
        Ok(self.emit(named_kind(name, arguments)?, token.span))
    }
    fn literal(&mut self, token: Token<'a>) -> Result<u32, Diagnostic> {
        let mut bytes = Vec::new();
        let mut chars = token.text[1..token.text.len() - 1].chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                bytes.push(match chars.next() {
                    Some('n') => b'\n',
                    Some('r') => b'\r',
                    Some('t') => b'\t',
                    Some('0') => 0,
                    Some('"') => b'"',
                    Some('\\') => b'\\',
                    Some('x') => {
                        let high = chars.next().and_then(|c| c.to_digit(16));
                        let low = chars.next().and_then(|c| c.to_digit(16));
                        match (high, low) {
                            (Some(high), Some(low)) => (high * 16 + low) as u8,
                            _ => {
                                return Err(error(
                                    token.span,
                                    "expected two hexadecimal byte digits",
                                ));
                            }
                        }
                    }
                    _ => return Err(error(token.span, "invalid byte literal escape")),
                });
            } else {
                bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
            }
        }
        Ok(self.emit(syntax::InstructionKind::Bytes(bytes), token.span))
    }

    fn float_literal(&mut self, token: Token<'_>, negative: bool) -> Result<u32, Diagnostic> {
        let text = if negative {
            format!("-{}", token.text)
        } else {
            token.text.to_string()
        };
        let value = crate::numeric::parse_float(&text)
            .filter(|v| v.is_finite())
            .ok_or_else(|| error(token.span, "invalid finite f64 literal"))?;
        Ok(self.emit(
            syntax::InstructionKind::Float(crate::numeric::canonical_bits(value)),
            token.span,
        ))
    }

    fn unsigned_literal(&mut self, token: Token<'_>) -> Result<u32, Diagnostic> {
        let (digits, ty) = if let Some(v) = token.text.strip_suffix("u64") {
            (v, Type::U64)
        } else {
            (token.text.strip_suffix("u128").unwrap(), Type::U128)
        };
        let value = digits
            .parse::<u128>()
            .ok()
            .filter(|v| canonical_decimal(digits) && (ty == Type::U128 || *v <= u64::MAX as u128))
            .ok_or_else(|| error(token.span, "invalid unsigned literal"))?;
        Ok(self.emit(
            syntax::InstructionKind::Unsigned([value as u64, (value >> 64) as u64], ty),
            token.span,
        ))
    }

    fn radix_literal(&mut self, token: Token<'_>, negative: bool) -> Result<u32, Diagnostic> {
        let (digits, ty) = if let Some(s) = token.text.strip_suffix("u64") {
            (s, Type::U64)
        } else if let Some(s) = token.text.strip_suffix("u128") {
            (s, Type::U128)
        } else {
            (token.text, Type::I64)
        };
        let (digits, radix) = if let Some(s) = digits.strip_prefix("0x") {
            (s, 16)
        } else {
            (digits.strip_prefix("0b").unwrap(), 2)
        };
        let value = u128::from_str_radix(digits, radix)
            .ok()
            .filter(|_| {
                !digits.is_empty()
                    && (digits.len() == 1 || !digits.starts_with('0'))
                    && digits.bytes().all(|b| {
                        if radix == 16 {
                            b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                        } else {
                            matches!(b, b'0' | b'1')
                        }
                    })
            })
            .ok_or_else(|| error(token.span, "invalid canonical radix literal"))?;
        if ty == Type::I64 {
            let limit = i64::MAX as u128 + u128::from(negative);
            if value > limit || (negative && value == 0) {
                return Err(error(token.span, "i64 radix literal out of range"));
            }
            let v = if negative {
                (value as i64).wrapping_neg()
            } else {
                value as i64
            };
            Ok(self.emit(syntax::InstructionKind::Constant(v), token.span))
        } else {
            if negative || (ty == Type::U64 && value > u64::MAX as u128) {
                return Err(error(token.span, "unsigned radix literal out of range"));
            }
            Ok(self.emit(
                syntax::InstructionKind::Unsigned([value as u64, (value >> 64) as u64], ty),
                token.span,
            ))
        }
    }

    fn named_constant(&mut self, token: Token<'_>) -> Option<u32> {
        use syntax::InstructionKind as K;
        let kind = match token.text {
            "i64min" => K::Constant(i64::MIN),
            "i64max" => K::Constant(i64::MAX),
            "u64min" => K::Unsigned([0, 0], Type::U64),
            "u64max" => K::Unsigned([u64::MAX, 0], Type::U64),
            "u128min" => K::Unsigned([0, 0], Type::U128),
            "u128max" => K::Unsigned([u64::MAX, u64::MAX], Type::U128),
            "inf" => K::Float(f64::INFINITY.to_bits()),
            "nan" => K::Float(crate::numeric::NAN_BITS),
            _ => return None,
        };
        Some(self.emit(kind, token.span))
    }

    fn binding_prefix(&mut self, depth: usize) -> Result<(), Diagnostic> {
        while self.application
            && self.peek().is_some_and(|t| identifier(t.text))
            && self
                .tokens
                .get(self.cursor + 1)
                .is_some_and(|t| t.text == "=")
        {
            let name = self.next().unwrap();
            if !matches!(name.text.as_bytes()[0], b'a'..=b'z' | b'_')
                || matches!(
                    name.text,
                    "true"
                        | "false"
                        | "loop"
                        | "record"
                        | "inf"
                        | "nan"
                        | "i64min"
                        | "i64max"
                        | "u64min"
                        | "u64max"
                        | "u128min"
                        | "u128max"
                )
            {
                return Err(error(name.span, "reserved or invalid binding name"));
            }
            self.expect("=")?;
            let value = self.expression(0, depth + 1)?;
            self.expect(";")?;
            self.locals.insert(name.text, value);
            self.chain_tails.remove(&value);
        }
        Ok(())
    }

    fn callback_primary(&mut self, token: Token<'a>, depth: usize) -> Result<u32, Diagnostic> {
        let reference = self.function_reference(token)?;
        if token.text == "^" && self.peek().is_some_and(|t| t.text == "(") {
            self.expect("(")?;
            let arguments = self.expression_list(")", depth + 1)?;
            let close = self.expect(")")?;
            Ok(self.emit(
                syntax::InstructionKind::CallbackCall(reference, arguments),
                Span {
                    start: token.span.start,
                    end: close.span.end,
                },
            ))
        } else {
            let end = self.tokens[self.cursor - 1].span.end;
            Ok(self.emit(
                syntax::InstructionKind::FunctionReference(reference),
                Span {
                    start: token.span.start,
                    end,
                },
            ))
        }
    }
    fn negative_literal(&mut self, token: Token<'a>) -> Result<u32, Diagnostic> {
        if self.application && self.peek().is_some_and(|t| t.text == "inf") {
            let number = self.next().unwrap();
            return Ok(self.emit(
                syntax::InstructionKind::Float(f64::NEG_INFINITY.to_bits()),
                Span {
                    start: token.span.start,
                    end: number.span.end,
                },
            ));
        }
        if self.application
            && self
                .peek()
                .is_some_and(|t| t.text.starts_with("0x") || t.text.starts_with("0b"))
        {
            let mut number = self.next().unwrap();
            number.span.start = token.span.start;
            return self.radix_literal(number, true);
        }
        if self.application
            && self.peek().is_some_and(|t| {
                t.text.contains('.') || t.text.contains('e') || t.text.contains('E')
            })
        {
            let mut number = self.next().unwrap();
            number.span.start = token.span.start;
            return self.float_literal(number, true);
        }
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
    fn direct_call(&mut self, token: Token<'a>, depth: usize) -> Result<u32, Diagnostic> {
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
    fn local_value(&mut self, token: Token<'a>) -> Result<u32, Diagnostic> {
        if self.application {
            if let Some(id) = self.locals.get(token.text) {
                return Ok(*id);
            }
        }
        let parameter = if self.positional {
            compact_label(token)
                .ok()
                .filter(|id| (*id as usize) < self.parameter_count())
        } else {
            self.parameters.get(token.text).copied()
        };
        if parameter.is_some_and(|id| self.static_parameters.contains(&id)) {
            return Err(Diagnostic::new(
                "E007",
                Phase::Check,
                Some(token.span),
                "static function parameters require a caret reference",
            ));
        }
        if let Some(id) = parameter {
            return Ok(id);
        }
        if self.application {
            if let Some(id) = self.named_constant(token) {
                return Ok(id);
            }
        }
        parameter.ok_or_else(|| {
            Diagnostic::new(
                "E005",
                Phase::Check,
                Some(token.span),
                format!("unknown parameter {}", token.text),
            )
        })
    }
    fn primary(&mut self, depth: usize) -> Result<u32, Diagnostic> {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(error(self.here(), "expression nesting limit exceeded"));
        }
        let token = self
            .next()
            .ok_or_else(|| error(self.here(), "expected expression"))?;
        match token.text {
            "&" | "^" if self.application => self.callback_primary(token, depth),
            "!" if self.application => {
                let is_call = self
                    .peek()
                    .is_some_and(|t| t.text.contains('.') || t.text == "each")
                    || self
                        .tokens
                        .get(self.cursor + 1)
                        .is_some_and(|t| t.text == "(")
                    || (self
                        .peek()
                        .is_some_and(|t| matches!(t.text, "map" | "buffer"))
                        && self
                            .tokens
                            .get(self.cursor + 1)
                            .is_some_and(|t| t.text == "["));
                if is_call {
                    self.intrinsic(token, depth)
                } else {
                    let value = self.postfix(depth + 1)?;
                    Ok(self.emit(
                        syntax::InstructionKind::Unary(UnaryOp::Not, value),
                        token.span,
                    ))
                }
            }
            "~" if self.application => {
                let value = self.postfix(depth + 1)?;
                Ok(self.emit(
                    syntax::InstructionKind::Unary(UnaryOp::BitNot, value),
                    token.span,
                ))
            }
            _ if self.application && token.text.starts_with('"') => self.literal(token),
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
            _ if self.application
                && (token.text.starts_with("0x") || token.text.starts_with("0b")) =>
            {
                self.radix_literal(token, false)
            }
            _ if self.application
                && token.text.as_bytes()[0].is_ascii_digit()
                && (token.text.contains('.')
                    || token.text.contains('e')
                    || token.text.contains('E')) =>
            {
                self.float_literal(token, false)
            }
            _ if self.application
                && (token.text.ends_with("u64") || token.text.ends_with("u128")) =>
            {
                self.unsigned_literal(token)
            }
            "-" => self.negative_literal(token),
            _ if token.text.bytes().all(|b| b.is_ascii_digit()) => {
                let value = token
                    .text
                    .parse::<i64>()
                    .ok()
                    .filter(|_| canonical_decimal(token.text))
                    .ok_or_else(|| error(token.span, "expected canonical i64 literal"))?;
                Ok(self.emit(syntax::InstructionKind::Constant(value), token.span))
            }
            _ if self.application && token.text.as_bytes()[0].is_ascii_uppercase() => {
                self.record_constructor(token, depth)
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
                self.direct_call(token, depth)
            }
            _ if identifier(token.text) => self.local_value(token),
            _ => Err(error(token.span, "expected expression")),
        }
    }

    fn expression(&mut self, minimum: u8, depth: usize) -> Result<u32, Diagnostic> {
        let saved = self.locals.clone();
        if minimum == 0 {
            self.binding_prefix(depth)?;
        }
        let result = self.expression_inner(minimum, depth);
        self.locals = saved;
        result
    }

    fn logical_expression(
        &mut self,
        token: Token<'a>,
        lhs: u32,
        precedence: u8,
        depth: usize,
    ) -> Result<u32, Diagnostic> {
        let base =
            self.base.unwrap_or(self.parameter_count() as u32) + self.instructions.len() as u32;
        let rhs = self.branch_at(precedence + 1, depth)?;
        let constant = syntax::Region {
            instructions: vec![syntax::Instruction {
                kind: syntax::InstructionKind::Boolean(token.text == "||"),
                span: token.span,
            }],
            results: vec![base],
        };
        Ok(self.emit(
            if token.text == "&&" {
                syntax::InstructionKind::If(lhs, rhs, constant)
            } else {
                syntax::InstructionKind::If(lhs, constant, rhs)
            },
            token.span,
        ))
    }
    fn expression_inner(&mut self, minimum: u8, depth: usize) -> Result<u32, Diagnostic> {
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
                let precedence = if self.application { 7 } else { 1 };
                if minimum > precedence {
                    break;
                }
                self.next();
                let rhs = self.expression(precedence + 1, depth + 1)?;
                lhs = self.emit(syntax::InstructionKind::Compare(op, lhs, rhs), token.span);
                continue;
            }
            if self.application && matches!(token.text, "&&" | "||") {
                let precedence = if token.text == "&&" { 3 } else { 2 };
                if precedence < minimum {
                    break;
                }
                self.next();
                lhs = self.logical_expression(token, lhs, precedence, depth)?;
                continue;
            }
            let (precedence, op) = match token.text {
                "+" => (if self.application { 9 } else { 2 }, BinaryOp::Add),
                "-" => (if self.application { 9 } else { 2 }, BinaryOp::Sub),
                "*" => (if self.application { 10 } else { 3 }, BinaryOp::Mul),
                "/" => (if self.application { 10 } else { 3 }, BinaryOp::Div),
                "%" if self.application => (10, BinaryOp::Rem),
                "<<" if self.application => (8, BinaryOp::Shl),
                ">>" if self.application => (8, BinaryOp::Shr),
                "&" if self.application => (6, BinaryOp::BitAnd),
                "^" if self.application => (5, BinaryOp::BitXor),
                "|" if self.application => (4, BinaryOp::BitOr),
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
        self.branch_at(0, depth)
    }

    fn branch_at(&mut self, minimum: u8, depth: usize) -> Result<syntax::Region, Diagnostic> {
        let base =
            self.base.unwrap_or(self.parameter_count() as u32) + self.instructions.len() as u32;
        let saved = std::mem::take(&mut self.instructions);
        let saved_tails = std::mem::take(&mut self.chain_tails);
        let saved_base = self.base.replace(base);
        let result = self.expression(minimum, depth + 1)?;
        let instructions = std::mem::replace(&mut self.instructions, saved);
        self.chain_tails = saved_tails;
        self.base = saved_base;
        Ok(syntax::Region {
            instructions,
            results: vec![result],
        })
    }

    fn expression_list(&mut self, terminator: &str, depth: usize) -> Result<Vec<u32>, Diagnostic> {
        let saved = self.locals.clone();
        self.binding_prefix(depth)?;
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
        self.locals = saved;
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
        let saved_tails = std::mem::take(&mut self.chain_tails);
        let parameters = std::mem::take(&mut self.parameters);
        let locals = std::mem::take(&mut self.locals);
        let static_parameters = std::mem::take(&mut self.static_parameters);
        let saved_arity = self.arity.replace(arity);
        let saved_positional = std::mem::replace(&mut self.positional, true);
        let saved_base = self.base.take();
        let results = if list {
            self.expression_list(terminator, depth)?
        } else {
            vec![self.expression(0, depth + 1)?]
        };
        let instructions = std::mem::replace(&mut self.instructions, saved);
        self.chain_tails = saved_tails;
        self.parameters = parameters;
        self.locals = locals;
        self.static_parameters = static_parameters;
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

    fn record(mut self) -> Result<nil_hir::RecordDefinition, Diagnostic> {
        self.expect("record")?;
        let name = self
            .next()
            .ok_or_else(|| error(self.here(), "expected record name"))?;
        if !identifier(name.text) || !name.text.as_bytes()[0].is_ascii_uppercase() {
            return Err(nil_hir::records::error(
                Some(name.span),
                "record names start with an uppercase letter",
            ));
        }
        self.expect("(")?;
        let mut fields = Vec::new();
        if self.peek().is_some_and(|t| t.text != ")") {
            loop {
                let name = self
                    .next()
                    .ok_or_else(|| error(self.here(), "expected field name"))?;
                if !identifier(name.text) {
                    return Err(error(name.span, "expected field name"));
                }
                self.expect(":")?;
                fields.push(nil_hir::RecordField {
                    name: name.text.into(),
                    ty: self.signature_type()?,
                });
                if fields.len() > 64 {
                    return Err(nil_hir::records::error(
                        Some(name.span),
                        "at most 64 fields",
                    ));
                }
                if self.peek().is_some_and(|t| t.text == ",") {
                    self.next();
                } else {
                    break;
                }
            }
        }
        self.expect(")")?;
        if self.peek().is_some() {
            return Err(error(self.here(), "unexpected token after record"));
        }
        Ok(nil_hir::RecordDefinition {
            name: name.text.into(),
            fields,
        })
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
                    types.push(self.parameter_type()?);
                    if self.peek().is_some_and(|t| t.text == ",") {
                        self.next();
                    } else {
                        break;
                    }
                }
            }
            self.expect(")")?;
            if types
                .iter()
                .all(|ty| *ty == syntax::Parameter::Value(Type::I64))
            {
                return Err(error(name.span, "use numeric arity for all-i64 parameters"));
            }
            self.arity = Some(types.len());
            self.static_parameters = types
                .iter()
                .enumerate()
                .filter(|(_, ty)| matches!(ty, syntax::Parameter::Function(_)))
                .map(|(id, _)| id as u32)
                .collect();
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
            parameters: typed_parameters.unwrap_or_else(|| {
                vec![syntax::Parameter::Value(Type::I64); self.parameter_count()]
            }),
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
    parse_profile(source, false, false, false, false)
}

pub fn parse_compact(source: &str) -> Result<syntax::Module, Diagnostic> {
    parse_profile(source, true, false, false, false)
}

pub fn parse_positional(source: &str) -> Result<syntax::Module, Diagnostic> {
    parse_profile(source, true, true, false, false)
}

pub fn parse_typed(source: &str) -> Result<syntax::Module, Diagnostic> {
    parse_profile(source, true, true, true, false)
}

pub fn parse_application(source: &str) -> Result<syntax::Module, Diagnostic> {
    parse_profile(source, true, true, true, true)
}

fn parse_profile(
    source: &str,
    compact: bool,
    positional: bool,
    typed: bool,
    application: bool,
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
    let mut records = Vec::new();
    let mut offset = 0;
    for raw in source.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let tokens = if typed {
            scan_profile(line, offset, true, application)?
        } else {
            scan(line, offset)?
        };
        if !tokens.is_empty() {
            let declaration = application && tokens[0].text == "record";
            let parser = ExprParser {
                records: &records,
                tokens,
                cursor: 0,
                compact,
                positional,
                symbolic_loop: compact && positional,
                typed,
                application,
                arity: None,
                base: None,
                end: offset + line.len(),
                parameters: BTreeMap::new(),
                locals: BTreeMap::new(),
                static_parameters: BTreeSet::new(),
                instructions: Vec::new(),
                chain_tails: BTreeSet::new(),
            };
            if declaration {
                if !functions.is_empty() {
                    return Err(nil_hir::records::error(
                        None,
                        "record declarations precede functions",
                    ));
                }
                records.push(parser.record()?);
                nil_hir::records::validate_definitions(&records)?;
            } else {
                functions.push(parser.function(functions.len() as u32)?);
            }
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
    Ok(syntax::Module { records, functions })
}

fn qualified(text: &str, span: Span) -> Result<(u32, u32), Diagnostic> {
    let (id, op) = text
        .split_once('.')
        .ok_or_else(|| error(span, "expected ID.EXPORT"))?;
    Ok((
        canonical_u32(id).ok_or_else(|| error(span, "canonical module ID required"))?,
        canonical_u32(op).ok_or_else(|| error(span, "canonical export ID required"))?,
    ))
}

fn named_kind(name: Token<'_>, args: Vec<u32>) -> Result<syntax::InstructionKind, Diagnostic> {
    if name.text.contains('.') {
        let (id, op) = qualified(name.text, name.span)?;
        Ok(syntax::InstructionKind::Plugin(id, op, args))
    } else if matches!(name.text, "filter" | "fold" | "push" | "swap" | "pop")
        || (name.text == "map" && args.len() == 2)
    {
        Ok(syntax::InstructionKind::Std(name.text.into(), args))
    } else {
        Ok(syntax::InstructionKind::Intrinsic(
            nil_hir::Intrinsic::parse(name.text)
                .ok_or_else(|| error(name.span, "unknown application operation"))?,
            args,
        ))
    }
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
