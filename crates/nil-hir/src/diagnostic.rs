use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Parse,
    Check,
    Execute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub phase: Phase,
    pub span: Option<Span>,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

impl Diagnostic {
    pub fn new(
        code: &'static str,
        phase: Phase,
        span: Option<Span>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            phase,
            span,
            message: message.into(),
            expected: None,
            actual: None,
        }
    }

    pub fn mismatch(mut self, expected: impl ToString, actual: impl ToString) -> Self {
        self.expected = Some(expected.to_string());
        self.actual = Some(actual.to_string());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code)?;
        if let Some(span) = self.span {
            write!(f, " @{}..{}", span.start, span.end)?;
        }
        write!(f, " {}", self.message)?;
        if let (Some(expected), Some(actual)) = (&self.expected, &self.actual) {
            write!(f, " expected:{expected} got:{actual}")?;
        }
        Ok(())
    }
}
impl std::error::Error for Diagnostic {}
