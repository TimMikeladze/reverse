//! Hand-written lexer with the layout pass folded in.
//!
//! Verse blocks come in three spellings: braces, a colon followed by an
//! indented block, and a single inline expression. The lexer turns indentation
//! into `Indent` / `Dedent` / `Newline` tokens so the parser never looks at
//! whitespace. Parentheses and square brackets suppress layout; braces open a
//! fresh layout context.

use crate::diag::{Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Lit(String),
    /// Source text of an interpolated expression and its absolute offset.
    Expr {
        src: String,
        offset: u32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Char(char),
    Str(Vec<StrPart>),
    Path(String),
    /// A specifier such as `<decides>`; holds the name without brackets.
    Spec(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Colon,
    ColonEq,
    Eq,
    FatArrow,
    ThinArrow,
    Dot,
    DotDot,
    Question,
    Plus,
    Minus,
    Star,
    Slash,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    Lt,
    Le,
    Gt,
    Ge,
    Ne,
    At,
    Newline,
    Indent,
    Dedent,
    Eof,
    Error,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
    /// True when whitespace (or a line break) precedes the token.
    pub space_before: bool,
}

enum Ctx {
    Paren,
    /// Indentation stack of a layout context. Empty until its first line.
    Layout(Vec<u32>),
}

struct Lexer<'a> {
    src: &'a str,
    b: &'a [u8],
    i: usize,
    base: u32,
    toks: Vec<Token>,
    diags: Vec<Diagnostic>,
    ctx: Vec<Ctx>,
    at_line_start: bool,
}

/// Lex a whole file.
pub fn lex(src: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    run(src, 0, true)
}

/// Lex a fragment (an interpolated expression) with layout switched off.
pub fn lex_fragment(src: &str, base: u32) -> (Vec<Token>, Vec<Diagnostic>) {
    run(src, base, false)
}

fn run(src: &str, base: u32, layout: bool) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut lx = Lexer {
        src,
        b: src.as_bytes(),
        i: 0,
        base,
        toks: Vec::new(),
        diags: Vec::new(),
        ctx: vec![if layout {
            Ctx::Layout(vec![0])
        } else {
            Ctx::Paren
        }],
        at_line_start: layout,
    };
    lx.run();
    (lx.toks, lx.diags)
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}
fn is_ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

impl<'a> Lexer<'a> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn peek_at(&self, n: usize) -> Option<u8> {
        self.b.get(self.i + n).copied()
    }
    fn span(&self, start: usize, end: usize) -> Span {
        Span::new(self.base + start as u32, self.base + end as u32)
    }
    fn push(&mut self, tok: Tok, start: usize, end: usize, space_before: bool) {
        let span = self.span(start, end);
        self.toks.push(Token {
            tok,
            span,
            space_before,
        });
    }
    fn err(&mut self, code: &'static str, start: usize, end: usize, msg: &str) -> &mut Diagnostic {
        let span = self.span(start, end);
        self.diags.push(Diagnostic::error(code, span, msg));
        self.diags.last_mut().unwrap()
    }

    /// A line that ends in one of these continues on the next line.
    fn continues_line(&self) -> bool {
        match self.toks.last().map(|t| &t.tok) {
            Some(
                Tok::Plus
                | Tok::Minus
                | Tok::Star
                | Tok::Slash
                | Tok::Comma
                | Tok::Dot
                | Tok::DotDot
                | Tok::Lt
                | Tok::Le
                | Tok::Gt
                | Tok::Ge
                | Tok::Ne
                | Tok::PlusEq
                | Tok::MinusEq
                | Tok::StarEq
                | Tok::SlashEq
                | Tok::ThinArrow,
            ) => true,
            Some(Tok::Ident(s)) => s == "and" || s == "or" || s == "not",
            _ => false,
        }
    }

    fn skip_block_comment(&mut self) {
        // At `<#`. Block comments nest.
        let start = self.i;
        if self.peek_at(2) == Some(b'>') {
            // `<#>` indented comment: approximated as a comment to end of line.
            while let Some(c) = self.peek() {
                if c == b'\n' {
                    break;
                }
                self.i += 1;
            }
            return;
        }
        self.i += 2;
        let mut depth = 1;
        while self.i < self.b.len() {
            if self.b[self.i] == b'<' && self.peek_at(1) == Some(b'#') {
                depth += 1;
                self.i += 2;
            } else if self.b[self.i] == b'#' && self.peek_at(1) == Some(b'>') {
                depth -= 1;
                self.i += 2;
                if depth == 0 {
                    return;
                }
            } else {
                self.i += 1;
            }
        }
        self.err("V0003", start, start + 2, "unterminated block comment")
            .help = Some("close the comment with `#>`".into());
    }

    fn layout(&mut self, col: u32) {
        let here = self.i;
        if matches!(self.peek(), Some(b'}') | Some(b')') | Some(b']')) {
            return;
        }
        let continues = self.continues_line();
        let last_is_lbrace = matches!(self.toks.last().map(|t| &t.tok), Some(Tok::LBrace) | None);
        let mut emit: Vec<Tok> = Vec::new();
        let mut bad_dedent = false;
        match self.ctx.last_mut() {
            Some(Ctx::Layout(stack)) => {
                if continues {
                    return;
                }
                match stack.last().copied() {
                    None => {
                        stack.push(col);
                        if !last_is_lbrace {
                            emit.push(Tok::Newline);
                        }
                    }
                    Some(top) if col > top => {
                        stack.push(col);
                        emit.push(Tok::Indent);
                    }
                    Some(top) if col == top => emit.push(Tok::Newline),
                    Some(_) => {
                        while stack.len() > 1 && col < *stack.last().unwrap() {
                            stack.pop();
                            emit.push(Tok::Dedent);
                        }
                        if col > *stack.last().unwrap() {
                            bad_dedent = true;
                        } else if col < *stack.last().unwrap() {
                            // Shallower than the context's first line: adopt it.
                            *stack.last_mut().unwrap() = col;
                        }
                        emit.push(Tok::Newline);
                    }
                }
            }
            _ => return,
        }
        for t in emit {
            self.push(t, here, here, true);
        }
        if bad_dedent {
            self.err(
                "V0002",
                here,
                here + 1,
                "this line's indentation does not match any enclosing block",
            )
            .help = Some(
                "indent with the same number of spaces as an enclosing block (Verse code uses 4)"
                    .into(),
            );
        }
    }

    fn close_layouts_until_paren(&mut self) {
        // A `)` or `]` arrived: drop any layout contexts opened inside it.
        while let Some(Ctx::Layout(stack)) = self.ctx.last() {
            if self.ctx.len() == 1 {
                return;
            }
            let n = stack.len().saturating_sub(1);
            for _ in 0..n {
                self.push(Tok::Dedent, self.i, self.i, false);
            }
            self.ctx.pop();
        }
        if self.ctx.len() > 1 {
            self.ctx.pop();
        }
    }

    fn run(&mut self) {
        loop {
            if self.at_line_start {
                let mut col = 0u32;
                loop {
                    match self.peek() {
                        Some(b' ') => col += 1,
                        Some(b'\t') => col += 4,
                        Some(b'\r') => {}
                        _ => break,
                    }
                    self.i += 1;
                }
                match self.peek() {
                    None => break,
                    Some(b'\n') => {
                        self.i += 1;
                        continue;
                    }
                    Some(b'#') => {
                        while let Some(c) = self.peek() {
                            if c == b'\n' {
                                break;
                            }
                            self.i += 1;
                        }
                        continue;
                    }
                    Some(b'<') if self.peek_at(1) == Some(b'#') => {
                        self.skip_block_comment();
                        while matches!(self.peek(), Some(b' ') | Some(b'\t') | Some(b'\r')) {
                            self.i += 1;
                        }
                        if matches!(self.peek(), None | Some(b'\n')) {
                            continue;
                        }
                    }
                    Some(b'/') if self.peek_at(1) == Some(b'/') => {
                        // handled as a token below so the diagnostic is reported once
                    }
                    _ => {}
                }
                self.layout(col);
                self.at_line_start = false;
            }
            let ws_start = self.i;
            while matches!(self.peek(), Some(b' ') | Some(b'\t') | Some(b'\r')) {
                self.i += 1;
            }
            let space = self.i > ws_start
                || self.i == 0
                || matches!(
                    self.b.get(self.i.wrapping_sub(1)),
                    Some(b'\n') | Some(b' ') | Some(b'\t')
                );
            let Some(c) = self.peek() else { break };
            let start = self.i;
            match c {
                b'\n' => {
                    self.i += 1;
                    self.at_line_start = true;
                }
                b'#' => {
                    while let Some(c) = self.peek() {
                        if c == b'\n' {
                            break;
                        }
                        self.i += 1;
                    }
                }
                b'(' | b'[' => {
                    self.i += 1;
                    self.ctx.push(Ctx::Paren);
                    self.push(
                        if c == b'(' {
                            Tok::LParen
                        } else {
                            Tok::LBracket
                        },
                        start,
                        self.i,
                        space,
                    );
                }
                b')' | b']' => {
                    self.close_layouts_until_paren();
                    self.i += 1;
                    self.push(
                        if c == b')' {
                            Tok::RParen
                        } else {
                            Tok::RBracket
                        },
                        start,
                        self.i,
                        space,
                    );
                }
                b'{' => {
                    self.i += 1;
                    self.ctx.push(Ctx::Layout(Vec::new()));
                    self.push(Tok::LBrace, start, self.i, space);
                }
                b'}' => {
                    if self.ctx.len() > 1 {
                        if let Some(Ctx::Layout(stack)) = self.ctx.last() {
                            let n = stack.len().saturating_sub(1);
                            for _ in 0..n {
                                self.push(Tok::Dedent, start, start, false);
                            }
                        }
                        self.ctx.pop();
                    }
                    self.i += 1;
                    self.push(Tok::RBrace, start, self.i, space);
                }
                b'"' => self.string(space),
                b'\'' => self.char_lit(space),
                b'0'..=b'9' => self.number(space),
                c if is_ident_start(c) => {
                    while self.peek().map(is_ident_char).unwrap_or(false) {
                        self.i += 1;
                    }
                    let s = &self.src[start..self.i];
                    self.push(Tok::Ident(s.to_string()), start, self.i, space);
                }
                _ => self.punct(space),
            }
        }
        // Close whatever is still open.
        let end = self.b.len();
        let mut dedents = 0;
        for c in &self.ctx {
            if let Ctx::Layout(stack) = c {
                dedents += stack.len().saturating_sub(1);
            }
        }
        for _ in 0..dedents {
            self.push(Tok::Dedent, end, end, false);
        }
        self.push(Tok::Eof, end, end, true);
    }

    fn prev_is_operand_end(&self) -> bool {
        matches!(
            self.toks.last().map(|t| &t.tok),
            Some(
                Tok::Ident(_)
                    | Tok::Int(_)
                    | Tok::Float(_)
                    | Tok::Str(_)
                    | Tok::Char(_)
                    | Tok::RParen
                    | Tok::RBracket
                    | Tok::RBrace
                    | Tok::Question
                    | Tok::Spec(_)
            )
        )
    }

    /// `<name>` written directly after an identifier, `)` or another
    /// specifier is a specifier, not a comparison.
    fn try_spec(&mut self, space: bool) -> bool {
        if space {
            return false;
        }
        if !matches!(
            self.toks.last().map(|t| &t.tok),
            Some(Tok::Ident(_) | Tok::RParen | Tok::Spec(_))
        ) {
            return false;
        }
        let start = self.i;
        let mut j = self.i + 1;
        let name_start = j;
        while j < self.b.len()
            && (self.b[j].is_ascii_lowercase() || self.b[j] == b'_' || self.b[j].is_ascii_digit())
        {
            j += 1;
        }
        if j == name_start || !self.b[name_start].is_ascii_lowercase() {
            return false;
        }
        let name_end = j;
        // Optional `{...}` argument, as in `<scoped {MyModule}>`.
        let mut k = j;
        while k < self.b.len() && self.b[k] == b' ' {
            k += 1;
        }
        if k < self.b.len() && self.b[k] == b'{' {
            while k < self.b.len() && self.b[k] != b'}' && self.b[k] != b'\n' {
                k += 1;
            }
            if k < self.b.len() && self.b[k] == b'}' {
                j = k + 1;
            }
        }
        if self.b.get(j) != Some(&b'>') {
            return false;
        }
        let name = self.src[name_start..name_end].to_string();
        self.i = j + 1;
        self.push(Tok::Spec(name), start, self.i, false);
        true
    }

    fn punct(&mut self, space: bool) {
        let start = self.i;
        let c = self.b[self.i];
        let n = self.peek_at(1);
        let two = |a: u8, b: u8| c == a && n == Some(b);
        // Habits from other languages get a targeted message and a fix.
        if two(b'/', b'/') {
            let mut j = self.i;
            while j < self.b.len() && self.b[j] != b'\n' {
                j += 1;
            }
            let d = self.err(
                "V0004",
                start,
                start + 2,
                "`//` does not start a comment in Verse",
            );
            d.help =
                Some("Verse line comments start with `#`; block comments are `<# ... #>`".into());
            d.fix = Some(crate::diag::Fix {
                span: Span::new(d.span.start, d.span.end),
                replacement: "#".into(),
            });
            self.i = j;
            return;
        }
        let foreign: Option<(usize, Tok, &str, &str)> = if two(b'=', b'=') {
            Some((2, Tok::Eq, "`==` is not a Verse operator", "="))
        } else if two(b'!', b'=') {
            Some((2, Tok::Ne, "`!=` is not a Verse operator", "<>"))
        } else if two(b'&', b'&') {
            Some((
                2,
                Tok::Ident("and".into()),
                "`&&` is not a Verse operator",
                "and",
            ))
        } else if two(b'|', b'|') {
            Some((
                2,
                Tok::Ident("or".into()),
                "`||` is not a Verse operator",
                "or",
            ))
        } else if c == b'!' {
            Some((
                1,
                Tok::Ident("not".into()),
                "`!` is not a Verse operator",
                "not ",
            ))
        } else {
            None
        };
        if let Some((len, tok, msg, repl)) = foreign {
            self.i += len;
            let d = self.err("V0005", start, start + len, msg);
            d.help = Some(format!("write `{}` instead", repl.trim()));
            d.fix = Some(crate::diag::Fix {
                span: d.span,
                replacement: repl.to_string(),
            });
            self.push(tok, start, start + len, space);
            return;
        }
        if (two(b'+', b'+') || two(b'-', b'-')) && self.prev_is_operand_end() && !space {
            let op = if c == b'+' { "+=" } else { "-=" };
            self.i += 2;
            let d = self.err(
                "V0005",
                start,
                start + 2,
                "Verse has no `++` or `--` operator",
            );
            d.help = Some(format!("write `set X {} 1`", op));
            self.push(Tok::Error, start, start + 2, space);
            return;
        }
        if c == b'<' && n == Some(b'#') {
            self.skip_block_comment();
            return;
        }
        if c == b'<' && self.try_spec(space) {
            return;
        }
        if c == b'/'
            && n.map(|x| x.is_ascii_alphabetic()).unwrap_or(false)
            && !self.prev_is_operand_end()
        {
            let mut j = self.i + 1;
            while j < self.b.len()
                && (self.b[j].is_ascii_alphanumeric()
                    || matches!(self.b[j], b'_' | b'.' | b'/' | b'@' | b'-'))
            {
                j += 1;
            }
            let s = self.src[start..j].to_string();
            self.i = j;
            self.push(Tok::Path(s), start, j, space);
            return;
        }
        let (tok, len) = match (c, n) {
            (b':', Some(b'=')) => (Tok::ColonEq, 2),
            (b'=', Some(b'>')) => (Tok::FatArrow, 2),
            (b'-', Some(b'>')) => (Tok::ThinArrow, 2),
            (b'.', Some(b'.')) => (Tok::DotDot, 2),
            (b'+', Some(b'=')) => (Tok::PlusEq, 2),
            (b'-', Some(b'=')) => (Tok::MinusEq, 2),
            (b'*', Some(b'=')) => (Tok::StarEq, 2),
            (b'/', Some(b'=')) => (Tok::SlashEq, 2),
            (b'<', Some(b'=')) => (Tok::Le, 2),
            (b'>', Some(b'=')) => (Tok::Ge, 2),
            (b'<', Some(b'>')) => (Tok::Ne, 2),
            (b':', _) => (Tok::Colon, 1),
            (b'=', _) => (Tok::Eq, 1),
            (b'.', _) => (Tok::Dot, 1),
            (b',', _) => (Tok::Comma, 1),
            (b';', _) => (Tok::Semi, 1),
            (b'?', _) => (Tok::Question, 1),
            (b'+', _) => (Tok::Plus, 1),
            (b'-', _) => (Tok::Minus, 1),
            (b'*', _) => (Tok::Star, 1),
            (b'/', _) => (Tok::Slash, 1),
            (b'<', _) => (Tok::Lt, 1),
            (b'>', _) => (Tok::Gt, 1),
            (b'@', _) => (Tok::At, 1),
            _ => {
                // Skip one whole UTF-8 character.
                let ch = self.src[start..].chars().next().unwrap();
                self.i += ch.len_utf8();
                self.err(
                    "V0001",
                    start,
                    self.i,
                    &format!("unexpected character `{}`", ch),
                );
                self.push(Tok::Error, start, self.i, space);
                return;
            }
        };
        self.i += len;
        self.push(tok, start, self.i, space);
    }

    fn number(&mut self, space: bool) {
        let start = self.i;
        if self.b[self.i] == b'0' && matches!(self.peek_at(1), Some(b'x') | Some(b'X')) {
            self.i += 2;
            let ds = self.i;
            while self.peek().map(|c| c.is_ascii_hexdigit()).unwrap_or(false) {
                self.i += 1;
            }
            match i64::from_str_radix(&self.src[ds..self.i], 16) {
                Ok(v) => self.push(Tok::Int(v), start, self.i, space),
                Err(_) => {
                    self.err(
                        "V0006",
                        start,
                        self.i,
                        "integer literal does not fit in 64 bits",
                    );
                    self.push(Tok::Int(0), start, self.i, space);
                }
            }
            return;
        }
        while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            self.i += 1;
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') && self.peek_at(1).map(|c| c.is_ascii_digit()).unwrap_or(false)
        {
            is_float = true;
            self.i += 1;
            while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            let mut j = self.i + 1;
            if matches!(self.b.get(j), Some(b'+') | Some(b'-')) {
                j += 1;
            }
            if self.b.get(j).map(|c| c.is_ascii_digit()).unwrap_or(false) {
                is_float = true;
                while self.b.get(j).map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    j += 1;
                }
                self.i = j;
            }
        }
        let text_end = self.i;
        if self.src[self.i..].starts_with("f64") {
            is_float = true;
            self.i += 3;
        }
        let text = &self.src[start..text_end];
        if is_float {
            let v: f64 = text.parse().unwrap_or(0.0);
            self.push(Tok::Float(v), start, self.i, space);
        } else {
            match text.parse::<i64>() {
                Ok(v) => self.push(Tok::Int(v), start, self.i, space),
                Err(_) => {
                    self.err(
                        "V0006",
                        start,
                        self.i,
                        "integer literal does not fit in 64 bits",
                    );
                    self.push(Tok::Int(0), start, self.i, space);
                }
            }
        }
    }

    fn escape(&mut self) -> Option<char> {
        // At the character after a backslash.
        let c = self.peek()?;
        self.i += 1;
        Some(match c {
            b'n' => '\n',
            b't' => '\t',
            b'r' => '\r',
            b'"' | b'\'' | b'\\' | b'{' | b'}' | b'<' | b'>' | b'&' | b'#' | b'~' => c as char,
            _ => {
                self.err(
                    "V0007",
                    self.i - 2,
                    self.i,
                    &format!("unknown escape `\\{}`", c as char),
                );
                c as char
            }
        })
    }

    fn char_lit(&mut self, space: bool) {
        let start = self.i;
        self.i += 1;
        let ch = if self.peek() == Some(b'\\') {
            self.i += 1;
            self.escape()
        } else {
            let ch = self.src[self.i..].chars().next();
            if let Some(ch) = ch {
                self.i += ch.len_utf8();
            }
            ch
        };
        if let (Some(b'\''), Some(ch)) = (self.peek(), ch) {
            self.i += 1;
            self.push(Tok::Char(ch), start, self.i, space);
        } else {
            self.err("V0003", start, self.i, "unterminated character literal")
                .help = Some(
                "character literals hold one character: 'a'. Strings use double quotes.".into(),
            );
            // Recover at end of line.
            while let Some(c) = self.peek() {
                if c == b'\n' || c == b'\'' {
                    break;
                }
                self.i += 1;
            }
            if self.peek() == Some(b'\'') {
                self.i += 1;
            }
            self.push(Tok::Error, start, self.i, space);
        }
    }

    fn string(&mut self, space: bool) {
        let start = self.i;
        self.i += 1;
        let mut parts: Vec<StrPart> = Vec::new();
        let mut cur = String::new();
        loop {
            let Some(c) = self.peek() else {
                self.err("V0003", start, start + 1, "unterminated string literal");
                break;
            };
            match c {
                b'"' => {
                    self.i += 1;
                    break;
                }
                b'\n' => {
                    self.err("V0003", start, start + 1, "unterminated string literal")
                        .help = Some("close the string with `\"` on the same line".into());
                    break;
                }
                b'\\' => {
                    self.i += 1;
                    if let Some(ch) = self.escape() {
                        cur.push(ch);
                    }
                }
                b'{' => {
                    let open = self.i;
                    let mut j = self.i + 1;
                    let mut depth = 1;
                    let mut in_str = false;
                    while j < self.b.len() {
                        let d = self.b[j];
                        if in_str {
                            if d == b'\\' {
                                j += 1;
                            } else if d == b'"' {
                                in_str = false;
                            }
                        } else if d == b'"' {
                            in_str = true;
                        } else if d == b'{' {
                            depth += 1;
                        } else if d == b'}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        j += 1;
                    }
                    if j >= self.b.len() {
                        self.err("V0003", open, open + 1, "unterminated `{` in string")
                            .help = Some(
                            "close the interpolation with `}` or write `\\{` for a literal brace"
                                .into(),
                        );
                        self.i = self.b.len();
                        break;
                    }
                    let inner = &self.src[open + 1..j];
                    if !inner.trim().is_empty() {
                        if !cur.is_empty() {
                            parts.push(StrPart::Lit(std::mem::take(&mut cur)));
                        }
                        parts.push(StrPart::Expr {
                            src: inner.to_string(),
                            offset: self.base + open as u32 + 1,
                        });
                    }
                    self.i = j + 1;
                }
                _ => {
                    let ch = self.src[self.i..].chars().next().unwrap();
                    cur.push(ch);
                    self.i += ch.len_utf8();
                }
            }
        }
        if !cur.is_empty() || parts.is_empty() {
            parts.push(StrPart::Lit(cur));
        }
        self.push(Tok::Str(parts), start, self.i, space);
    }
}
