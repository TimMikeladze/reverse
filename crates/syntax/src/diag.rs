//! Spans and diagnostics shared by every stage.
//!
//! Diagnostics are designed to be read by programs as well as people: each has
//! a stable code, an exact span, an optional `help` sentence and an optional
//! machine-applicable `fix`.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Span {
        Span { start, end }
    }
    pub fn to(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// A replacement that, applied to the source, resolves the diagnostic.
#[derive(Clone, Debug, PartialEq)]
pub struct Fix {
    pub span: Span,
    pub replacement: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
    pub fix: Option<Fix>,
}

impl Diagnostic {
    pub fn error(code: &'static str, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            code,
            severity: Severity::Error,
            message: message.into(),
            span,
            help: None,
            fix: None,
        }
    }
    pub fn warning(code: &'static str, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            code,
            severity: Severity::Warning,
            message: message.into(),
            span,
            help: None,
            fix: None,
        }
    }
    pub fn with_help(mut self, help: impl Into<String>) -> Diagnostic {
        self.help = Some(help.into());
        self
    }
    pub fn with_fix(mut self, span: Span, replacement: impl Into<String>) -> Diagnostic {
        self.fix = Some(Fix {
            span,
            replacement: replacement.into(),
        });
        self
    }
}

/// Maps byte offsets to 1-based line and column numbers.
pub struct LineIndex {
    line_starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(src: &str) -> LineIndex {
        let mut line_starts = vec![0u32];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i as u32 + 1);
            }
        }
        LineIndex { line_starts }
    }

    /// 1-based (line, column). Columns count bytes.
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        (line as u32 + 1, offset - self.line_starts[line] + 1)
    }

    pub fn line_text<'a>(&self, src: &'a str, line: u32) -> &'a str {
        let i = (line - 1) as usize;
        let start = self.line_starts[i] as usize;
        let end = self
            .line_starts
            .get(i + 1)
            .map(|e| *e as usize)
            .unwrap_or(src.len());
        src[start..end].trim_end_matches(['\n', '\r'])
    }
}
