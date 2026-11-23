//! Recursive-descent parser with a Pratt core for operators.
//!
//! Recovery is per statement: a syntax error produces one diagnostic, the
//! parser skips to the end of that statement and carries on, so one mistake
//! does not cascade.

use crate::ast::*;
use crate::diag::{Diagnostic, Span};
use crate::lexer::{self, StrPart, Tok, Token};

type PResult<T> = Result<T, ()>;

const KEYWORDS: &[&str] = &[
    "if",
    "else",
    "then",
    "for",
    "do",
    "loop",
    "break",
    "return",
    "case",
    "block",
    "var",
    "set",
    "and",
    "or",
    "not",
    "true",
    "false",
    "class",
    "struct",
    "enum",
    "interface",
    "module",
    "using",
    "array",
    "map",
    "option",
    "logic",
    "spawn",
    "sync",
    "race",
    "rush",
    "branch",
    "defer",
    "where",
    "while",
];

pub fn is_keyword(s: &str) -> bool {
    KEYWORDS.contains(&s)
}

#[derive(Clone, Copy, PartialEq)]
enum End {
    Eof,
    Dedent,
    RBrace,
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    diags: Vec<Diagnostic>,
    /// Nesting depth of function bodies and definition values. Zero means
    /// module level or a class body, where only definitions can appear.
    in_body: u32,
    /// Current recursion depth, capped so hostile input cannot overflow the
    /// native stack.
    depth: u32,
}

const MAX_NESTING: u32 = 400;

/// Parse a whole source file. Always returns a tree; errors are in the list.
pub fn parse(src: &str) -> (Module, Vec<Diagnostic>) {
    let (toks, mut diags) = lexer::lex(src);
    let mut p = Parser {
        toks,
        pos: 0,
        diags: Vec::new(),
        in_body: 0,
        depth: 0,
    };
    let items = p.stmts_until(End::Eof);
    diags.extend(p.diags);
    diags.sort_by_key(|d| d.span.start);
    // Lexer recoveries can make the parser repeat a complaint at the same place.
    diags.dedup_by(|a, b| a.span.start == b.span.start && a.code == "V0001" && b.code != "V0001");
    (Module { items }, diags)
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(s) => format!("`{}`", s),
        Tok::Int(_) | Tok::Float(_) => "a number".into(),
        Tok::Char(_) => "a character literal".into(),
        Tok::Str(_) => "a string".into(),
        Tok::Path(p) => format!("`{}`", p),
        Tok::Spec(s) => format!("`<{}>`", s),
        Tok::LParen => "`(`".into(),
        Tok::RParen => "`)`".into(),
        Tok::LBrace => "`{`".into(),
        Tok::RBrace => "`}`".into(),
        Tok::LBracket => "`[`".into(),
        Tok::RBracket => "`]`".into(),
        Tok::Comma => "`,`".into(),
        Tok::Semi => "`;`".into(),
        Tok::Colon => "`:`".into(),
        Tok::ColonEq => "`:=`".into(),
        Tok::Eq => "`=`".into(),
        Tok::FatArrow => "`=>`".into(),
        Tok::ThinArrow => "`->`".into(),
        Tok::Dot => "`.`".into(),
        Tok::DotDot => "`..`".into(),
        Tok::Question => "`?`".into(),
        Tok::Plus => "`+`".into(),
        Tok::Minus => "`-`".into(),
        Tok::Star => "`*`".into(),
        Tok::Slash => "`/`".into(),
        Tok::PlusEq => "`+=`".into(),
        Tok::MinusEq => "`-=`".into(),
        Tok::StarEq => "`*=`".into(),
        Tok::SlashEq => "`/=`".into(),
        Tok::Lt => "`<`".into(),
        Tok::Le => "`<=`".into(),
        Tok::Gt => "`>`".into(),
        Tok::Ge => "`>=`".into(),
        Tok::Ne => "`<>`".into(),
        Tok::At => "`@`".into(),
        Tok::Newline => "the end of the line".into(),
        Tok::Indent => "an indented block".into(),
        Tok::Dedent => "the end of the block".into(),
        Tok::Eof => "the end of the file".into(),
        Tok::Error => "an invalid token".into(),
    }
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }
    fn peek_n(&self, n: usize) -> &Tok {
        let i = (self.pos + n).min(self.toks.len() - 1);
        &self.toks[i].tok
    }
    fn tok_n(&self, n: usize) -> &Token {
        let i = (self.pos + n).min(self.toks.len() - 1);
        &self.toks[i]
    }
    fn span(&self) -> Span {
        self.toks[self.pos].span
    }
    fn prev_span(&self) -> Span {
        if self.pos == 0 {
            self.toks[0].span
        } else {
            self.toks[self.pos - 1].span
        }
    }
    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }
    fn at(&self, t: &Tok) -> bool {
        self.peek() == t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.at(t) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn at_kw(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Ident(x) if x == s)
    }
    fn eat_kw(&mut self, s: &str) -> bool {
        if self.at_kw(s) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn fail<T>(&mut self, span: Span, msg: impl Into<String>) -> PResult<T> {
        self.diags.push(Diagnostic::error("V0001", span, msg));
        Err(())
    }
    fn fail_help<T>(
        &mut self,
        span: Span,
        msg: impl Into<String>,
        help: impl Into<String>,
    ) -> PResult<T> {
        self.diags
            .push(Diagnostic::error("V0001", span, msg).with_help(help));
        Err(())
    }
    fn expect(&mut self, t: Tok, context: &str) -> PResult<Span> {
        if self.at(&t) {
            Ok(self.bump().span)
        } else {
            let found = describe(self.peek());
            let span = self.span();
            self.fail(
                span,
                format!("expected {} {}, found {}", describe(&t), context, found),
            )
        }
    }
    fn ident(&mut self, context: &str) -> PResult<(String, Span)> {
        if let Tok::Ident(s) = self.peek().clone() {
            let sp = self.bump().span;
            Ok((s, sp))
        } else {
            let found = describe(self.peek());
            let span = self.span();
            self.fail(
                span,
                format!("expected a name {}, found {}", context, found),
            )
        }
    }
    fn specs(&mut self) -> Vec<Spec> {
        let mut out = Vec::new();
        while let Tok::Spec(name) = self.peek().clone() {
            let span = self.bump().span;
            out.push(Spec { name, span });
        }
        out
    }

    fn synchronize(&mut self) {
        let mut depth = 0i32;
        loop {
            match self.peek() {
                Tok::Eof => break,
                Tok::Newline if depth == 0 => break,
                Tok::LParen | Tok::LBracket | Tok::LBrace | Tok::Indent => depth += 1,
                Tok::RParen | Tok::RBracket => depth = (depth - 1).max(0),
                Tok::RBrace | Tok::Dedent => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            self.bump();
        }
    }

    fn stmts_until(&mut self, end: End) -> Vec<Expr> {
        let mut out = Vec::new();
        loop {
            while matches!(self.peek(), Tok::Newline | Tok::Semi | Tok::Comma) {
                self.bump();
            }
            match self.peek() {
                Tok::Eof => break,
                Tok::Dedent => {
                    if end == End::Dedent {
                        break;
                    }
                    self.bump();
                    continue;
                }
                Tok::RBrace => {
                    if end == End::RBrace {
                        break;
                    }
                    if end == End::Dedent {
                        break;
                    }
                    let sp = self.span();
                    self.diags
                        .push(Diagnostic::error("V0001", sp, "unmatched `}`"));
                    self.bump();
                    continue;
                }
                Tok::Indent => {
                    let sp = self.span();
                    self.diags.push(
                        Diagnostic::error("V0002", sp, "unexpected indentation")
                            .with_help("a deeper indent is only allowed after a line that opens a block (ends in `:` or `=`)"),
                    );
                    self.bump();
                    let inner = self.stmts_until(End::Dedent);
                    self.eat(&Tok::Dedent);
                    out.extend(inner);
                    continue;
                }
                _ => {}
            }
            let before = self.pos;
            let errors_before = self.diags.len();
            match self.stmt() {
                Ok(e) => out.push(e),
                Err(()) => {
                    let sp = self.toks[before].span;
                    self.synchronize();
                    out.push(Expr {
                        kind: ExprKind::Error,
                        span: sp,
                    });
                    continue;
                }
            }
            match self.peek() {
                Tok::Newline
                | Tok::Semi
                | Tok::Comma
                | Tok::Eof
                | Tok::Dedent
                | Tok::RBrace
                | Tok::Indent => {}
                other => {
                    if self.diags.len() == errors_before {
                        let found = describe(other);
                        let sp = self.span();
                        let mut d = Diagnostic::error(
                            "V0001",
                            sp,
                            format!("expected the end of the statement, found {}", found),
                        );
                        if matches!(other, Tok::LBrace) {
                            d = d.with_help("a block opens with `:` or `{` directly after its header, for example `if (X?) {`");
                        }
                        self.diags.push(d);
                    }
                    if self.pos == before {
                        self.bump();
                    }
                    self.synchronize();
                }
            }
        }
        out
    }

    // ---------------------------------------------------------------- statements

    fn stmt(&mut self) -> PResult<Expr> {
        if self.depth >= MAX_NESTING {
            return self.too_deep();
        }
        self.depth += 1;
        let r = self.stmt_inner();
        self.depth -= 1;
        r
    }

    fn stmt_inner(&mut self) -> PResult<Expr> {
        while self.at(&Tok::At) {
            self.bump();
            self.ident("after `@`")?;
            if self.at(&Tok::LParen) && !self.tok_n(0).space_before {
                self.skip_balanced();
            }
            while self.at(&Tok::Newline) {
                self.bump();
            }
        }
        if self.at(&Tok::LParen) && self.looks_like_extension_def() {
            return self.extension_def();
        }
        let Tok::Ident(word) = self.peek().clone() else {
            return self.expr_or_def();
        };
        match word.as_str() {
            "using" => self.using(),
            "var" => self.var_def(),
            w if !is_keyword(w) => {
                let mut i = 1;
                while matches!(self.peek_n(i), Tok::Spec(_)) {
                    i += 1;
                }
                match self.peek_n(i) {
                    Tok::LParen if !self.tok_n(i).space_before && self.looks_like_func_def(i) => {
                        self.func_def()
                    }
                    Tok::Colon => self.typed_def(false),
                    Tok::ColonEq => self.named_def(),
                    Tok::Eq if i > 1 => self.typed_def(false),
                    _ => self.foreign_decl_or_expr(&word),
                }
            }
            _ => self.expr_or_def(),
        }
    }

    /// `let x = 1`, `const x = 1`, `func f()`, `int X = 1` ... are habits from
    /// other languages. Catch them before they become a confusing parse error.
    fn foreign_decl_or_expr(&mut self, word: &str) -> PResult<Expr> {
        let next_is_name =
            matches!(self.peek_n(1), Tok::Ident(s) if !is_keyword(s)) && self.tok_n(1).space_before;
        if next_is_name {
            let sp = self.span();
            let help = match word {
                "let" | "const" | "val" | "auto" => Some("Verse has no declaration keyword: write `Name := Value` or `Name : type = Value` (add `var` in front to make it mutable)"),
                "func" | "fn" | "def" | "function" => Some("Verse functions have no keyword: write `Name(Param : type) : return_type = Body`"),
                "int" | "float" | "string" | "logic" | "void" | "char" => Some("the type goes after the name: write `Name : type = Value`"),
                _ => None,
            };
            if let Some(help) = help {
                return self.fail_help(
                    sp,
                    format!("`{}` does not start a definition in Verse", word),
                    help,
                );
            }
        }
        self.expr_or_def()
    }

    fn skip_balanced(&mut self) {
        let mut depth = 0i32;
        loop {
            match self.peek() {
                Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
                Tok::RParen | Tok::RBracket | Tok::RBrace => {
                    depth -= 1;
                    if depth <= 0 {
                        self.bump();
                        return;
                    }
                }
                Tok::Eof => return,
                _ => {}
            }
            self.bump();
        }
    }

    /// `i` is the offset of the `(` after the name. A definition is a
    /// balanced parameter list followed (after effect specifiers) by `:`.
    fn looks_like_func_def(&self, mut i: usize) -> bool {
        let mut depth = 0i32;
        loop {
            match self.peek_n(i) {
                Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
                Tok::RParen | Tok::RBracket | Tok::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                Tok::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        let mut saw_spec = false;
        while matches!(self.peek_n(i), Tok::Spec(_)) {
            saw_spec = true;
            i += 1;
        }
        match self.peek_n(i) {
            Tok::Colon | Tok::ColonEq => true,
            Tok::Eq => saw_spec || self.in_body == 0,
            _ => false,
        }
    }

    fn using(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        self.expect(Tok::LBrace, "after `using`")?;
        let path = match self.peek().clone() {
            Tok::Path(p) => {
                self.bump();
                p
            }
            Tok::Ident(_) => {
                let mut s = self.ident("")?.0;
                while self.at(&Tok::Dot) {
                    self.bump();
                    s.push('.');
                    s.push_str(&self.ident("after `.`")?.0);
                }
                s
            }
            _ => {
                let sp = self.span();
                return self.fail_help(
                    sp,
                    "expected a module path",
                    "for example `using { /Verse.org/Random }`",
                );
            }
        };
        self.expect(Tok::RBrace, "to close `using`")?;
        Ok(Expr {
            kind: ExprKind::Using(path),
            span: start.to(self.prev_span()),
        })
    }

    fn var_def(&mut self) -> PResult<Expr> {
        self.bump();
        self.typed_def(true)
    }

    /// `Name<specs> : type = value`; with `is_var` the leading `var` is gone.
    fn typed_def(&mut self, is_var: bool) -> PResult<Expr> {
        let (name, name_span) = self.ident(if is_var { "after `var`" } else { "" })?;
        let specs = self.specs();
        if self.at(&Tok::ColonEq) {
            let sp = self.span();
            self.diags.push(
                Diagnostic::error("V0001", sp, "a `var` needs an explicit type").with_help(
                    format!(
                        "write `var {} : int = 0` (use the real type) instead of `var {} := ...`",
                        name, name
                    ),
                ),
            );
            self.bump();
            let value = self.body()?;
            return Ok(Expr {
                span: name_span.to(self.prev_span()),
                kind: ExprKind::Def(Box::new(Def {
                    name,
                    name_span,
                    ty: None,
                    value: Some(value),
                    is_var,
                    specs,
                })),
            });
        }
        let ty = if self.eat(&Tok::Colon) {
            Some(self.ty()?)
        } else {
            None
        };
        if is_var && ty.is_none() && self.at(&Tok::Eq) {
            self.diags.push(
                Diagnostic::error("V0001", name_span, "a `var` needs an explicit type").with_help(
                    format!("write `var {} : int = 0` (use the real type)", name),
                ),
            );
        }
        let value = if self.eat(&Tok::Eq) {
            Some(self.body()?)
        } else {
            None
        };
        if ty.is_none() && value.is_none() {
            let sp = self.span();
            return self.fail(sp, format!("expected `:` and a type after `{}`", name));
        }
        Ok(Expr {
            span: name_span.to(self.prev_span()),
            kind: ExprKind::Def(Box::new(Def {
                name,
                name_span,
                ty,
                value,
                is_var,
                specs,
            })),
        })
    }

    /// `Name := value`, where the value may be a class, enum or module.
    fn named_def(&mut self) -> PResult<Expr> {
        let (name, name_span) = self.ident("")?;
        let specs = self.specs();
        self.expect(Tok::ColonEq, "in definition")?;
        let Tok::Ident(kw) = self.peek().clone() else {
            let value = self.body()?;
            return Ok(Expr {
                span: name_span.to(self.prev_span()),
                kind: ExprKind::Def(Box::new(Def {
                    name,
                    name_span,
                    ty: None,
                    value: Some(value),
                    is_var: false,
                    specs,
                })),
            });
        };
        match kw.as_str() {
            "class" | "struct" | "interface" => {
                let kind = match kw.as_str() {
                    "class" => ClassKind::Class,
                    "struct" => ClassKind::Struct,
                    _ => ClassKind::Interface,
                };
                self.bump();
                let mut cspecs = self.specs();
                let mut parents = Vec::new();
                if self.at(&Tok::LParen) {
                    self.bump();
                    while !self.at(&Tok::RParen) {
                        let (mut p, mut psp) = self.ident("for a parent type")?;
                        while self.at(&Tok::Dot) {
                            self.bump();
                            let (seg, ssp) = self.ident("after `.`")?;
                            p = seg;
                            psp = psp.to(ssp);
                        }
                        parents.push((p, psp));
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(Tok::RParen, "after the parent list")?;
                }
                cspecs.extend(self.specs());
                let members = self.member_block()?;
                Ok(Expr {
                    span: name_span.to(self.prev_span()),
                    kind: ExprKind::Class(Box::new(ClassDef {
                        name,
                        name_span,
                        kind,
                        specs: cspecs,
                        parents,
                        members,
                    })),
                })
            }
            "enum" => {
                self.bump();
                let especs = self.specs();
                let values = self.delimited("in the enum", |p| p.ident("for an enum value"))?;
                Ok(Expr {
                    span: name_span.to(self.prev_span()),
                    kind: ExprKind::Enum(Box::new(EnumDef {
                        name,
                        name_span,
                        specs: especs,
                        values,
                    })),
                })
            }
            "module" => {
                self.bump();
                let items = self.member_block()?;
                Ok(Expr {
                    span: name_span.to(self.prev_span()),
                    kind: ExprKind::Module { name, items },
                })
            }
            _ => {
                let value = self.body()?;
                Ok(Expr {
                    span: name_span.to(self.prev_span()),
                    kind: ExprKind::Def(Box::new(Def {
                        name,
                        name_span,
                        ty: None,
                        value: Some(value),
                        is_var: false,
                        specs,
                    })),
                })
            }
        }
    }

    fn member_block(&mut self) -> PResult<Vec<Expr>> {
        if self.eat(&Tok::Colon) {
            if self.eat(&Tok::Indent) {
                let m = self.stmts_until(End::Dedent);
                self.expect(Tok::Dedent, "to close the block")?;
                Ok(m)
            } else {
                Ok(Vec::new())
            }
        } else if self.eat(&Tok::LBrace) {
            let m = self.stmts_until(End::RBrace);
            self.expect(Tok::RBrace, "to close the block")?;
            Ok(m)
        } else {
            Ok(Vec::new())
        }
    }

    fn func_def(&mut self) -> PResult<Expr> {
        let (name, name_span) = self.ident("")?;
        let pre_specs = self.specs();
        self.expect(Tok::LParen, "to open the parameter list")?;
        let mut params = Vec::new();
        let mut type_vars = Vec::new();
        while !self.at(&Tok::RParen) {
            let named = self.eat(&Tok::Question);
            if self.at(&Tok::LParen) {
                let sp = self.span();
                self.diags.push(Diagnostic::error(
                    "V6001",
                    sp,
                    "tuple-destructuring parameters are not supported yet",
                ));
                return Err(());
            }
            let (pname, pspan) = if self.at(&Tok::Colon) {
                ("_".to_string(), self.span())
            } else {
                self.ident("for a parameter")?
            };
            if !self.at(&Tok::Colon) {
                let sp = pspan;
                return self.fail_help(
                    sp,
                    format!("parameter `{}` needs a type", pname),
                    format!("write `{} : type`, for example `{} : int`", pname, pname),
                );
            }
            self.bump();
            let ty = self.ty()?;
            if self.eat_kw("where") {
                loop {
                    let (tv, _) = self.ident("for a type variable")?;
                    self.expect(Tok::Colon, "after the type variable")?;
                    self.ty()?;
                    type_vars.push(tv);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
            }
            let default = if self.eat(&Tok::Eq) {
                Some(self.expr()?)
            } else {
                None
            };
            params.push(Param {
                name: pname,
                span: pspan,
                ty,
                named,
                default,
            });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(Tok::RParen, "to close the parameter list")?;
        let effects = self.specs();
        let ret = if self.eat(&Tok::Colon) {
            Some(self.ty()?)
        } else if self.at(&Tok::ColonEq) {
            // A constructor function: `Make<constructor>(...) := my_class:`
            // followed by field initialisers. It is a function that returns
            // an archetype.
            self.bump();
            let (tname, tspan) = self.ident("for the type a constructor function creates")?;
            let ty_expr = Expr {
                kind: ExprKind::Ident(tname.clone()),
                span: tspan,
            };
            let fields = self.delimited("in the constructor function", |p| {
                let (fname, nsp) = p.ident("for a field")?;
                p.expect(Tok::ColonEq, "after the field name")?;
                let v = p.expr()?;
                Ok((fname, nsp, v))
            })?;
            let span = tspan.to(self.prev_span());
            let body = Expr {
                kind: ExprKind::Archetype {
                    ty: Box::new(ty_expr),
                    fields,
                },
                span,
            };
            return Ok(Expr {
                span: name_span.to(self.prev_span()),
                kind: ExprKind::Func(Box::new(FuncDef {
                    name,
                    name_span,
                    params,
                    type_vars,
                    pre_specs,
                    effects,
                    ret: Some(TypeExpr::Named {
                        name: tname,
                        args: Vec::new(),
                        span: tspan,
                    }),
                    body: Some(body),
                    extension: false,
                })),
            });
        } else {
            None
        };
        let body = if self.eat(&Tok::Eq) {
            Some(self.body()?)
        } else {
            None
        };
        Ok(Expr {
            span: name_span.to(self.prev_span()),
            kind: ExprKind::Func(Box::new(FuncDef {
                name,
                name_span,
                params,
                type_vars,
                pre_specs,
                effects,
                ret,
                body,
                extension: false,
            })),
        })
    }

    /// `(Receiver : type).Name(Params) : type = Body`
    fn extension_def(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        let (rname, rspan) = self.ident("for the receiver")?;
        self.expect(Tok::Colon, "after the receiver name")?;
        let rty = self.ty()?;
        self.expect(Tok::RParen, "after the receiver type")?;
        self.expect(Tok::Dot, "before the method name")?;
        let mut def = self.func_def()?;
        if let ExprKind::Func(f) = &mut def.kind {
            f.params.insert(
                0,
                Param {
                    name: rname,
                    span: rspan,
                    ty: rty,
                    named: false,
                    default: None,
                },
            );
            f.extension = true;
        }
        def.span = start.to(def.span);
        Ok(def)
    }

    /// At `(`: is this `(Name : type).Method(` at definition level?
    fn looks_like_extension_def(&self) -> bool {
        if self.in_body != 0
            || !matches!(self.peek_n(1), Tok::Ident(_))
            || self.peek_n(2) != &Tok::Colon
        {
            return false;
        }
        let mut depth = 0i32;
        let mut i = 0;
        loop {
            match self.peek_n(i) {
                Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
                Tok::RParen | Tok::RBracket | Tok::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Tok::Eof | Tok::Newline => return false,
                _ => {}
            }
            i += 1;
        }
        self.peek_n(i + 1) == &Tok::Dot && matches!(self.peek_n(i + 2), Tok::Ident(_))
    }

    // ---------------------------------------------------------------- types

    fn ty(&mut self) -> PResult<TypeExpr> {
        if self.depth >= MAX_NESTING {
            return self.too_deep();
        }
        self.depth += 1;
        let r = self.ty_inner();
        self.depth -= 1;
        r
    }

    fn ty_inner(&mut self) -> PResult<TypeExpr> {
        let start = self.span();
        match self.peek().clone() {
            Tok::LBracket => {
                self.bump();
                if self.eat(&Tok::RBracket) {
                    let inner = self.ty()?;
                    Ok(TypeExpr::Array(Box::new(inner), start.to(self.prev_span())))
                } else {
                    let k = self.ty()?;
                    self.expect(Tok::RBracket, "after the map key type")?;
                    let v = self.ty()?;
                    Ok(TypeExpr::Map(
                        Box::new(k),
                        Box::new(v),
                        start.to(self.prev_span()),
                    ))
                }
            }
            Tok::Question => {
                self.bump();
                let inner = self.ty()?;
                Ok(TypeExpr::Option(
                    Box::new(inner),
                    start.to(self.prev_span()),
                ))
            }
            Tok::LParen => {
                self.bump();
                let mut items = Vec::new();
                while !self.at(&Tok::RParen) {
                    items.push(self.ty()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(Tok::RParen, "to close the tuple type")?;
                Ok(TypeExpr::Tuple(items, start.to(self.prev_span())))
            }
            Tok::Ident(n) => {
                self.bump();
                if n == "tuple" && self.at(&Tok::LParen) {
                    self.bump();
                    let mut items = Vec::new();
                    while !self.at(&Tok::RParen) {
                        items.push(self.ty()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(Tok::RParen, "to close the tuple type")?;
                    return Ok(TypeExpr::Tuple(items, start.to(self.prev_span())));
                }
                if n == "type" && self.at(&Tok::LBrace) {
                    self.skip_balanced();
                    return Ok(TypeExpr::Opaque(start.to(self.prev_span())));
                }
                let mut name = n;
                while self.at(&Tok::Dot) && matches!(self.peek_n(1), Tok::Ident(_)) {
                    self.bump();
                    name = self.ident("")?.0;
                }
                let mut args = Vec::new();
                if self.at(&Tok::LParen) && !self.tok_n(0).space_before {
                    self.bump();
                    while !self.at(&Tok::RParen) {
                        match self.ty() {
                            Ok(t) => args.push(t),
                            Err(()) => return Err(()),
                        }
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(Tok::RParen, "to close the type arguments")?;
                }
                Ok(TypeExpr::Named {
                    name,
                    args,
                    span: start.to(self.prev_span()),
                })
            }
            other => {
                let found = describe(&other);
                self.fail_help(
                    start,
                    format!("expected a type, found {}", found),
                    "types look like `int`, `[]int` (array), `[string]int` (map), `?int` (option) or `tuple(int, float)`",
                )
            }
        }
    }

    // ---------------------------------------------------------------- blocks

    /// The value side of `=`, `:`, `then`, `else`, `=>`: an indented block,
    /// a brace block, or one inline statement.
    fn body(&mut self) -> PResult<Expr> {
        self.in_body += 1;
        let r = self.body_inner();
        self.in_body -= 1;
        r
    }

    fn body_inner(&mut self) -> PResult<Expr> {
        let start = self.span();
        if self.eat(&Tok::Indent) {
            let stmts = self.stmts_until(End::Dedent);
            self.expect(Tok::Dedent, "to close the block")?;
            return Ok(Expr {
                kind: ExprKind::Block(stmts),
                span: start.to(self.prev_span()),
            });
        }
        if self.at(&Tok::LBrace) {
            return self.brace_block();
        }
        self.stmt()
    }

    fn brace_block(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        let stmts = self.stmts_until(End::RBrace);
        self.expect(Tok::RBrace, "to close the block")?;
        Ok(Expr {
            kind: ExprKind::Block(stmts),
            span: start.to(self.prev_span()),
        })
    }

    /// Body after a header such as `if (...)`: optional `:` or `.`, then a body.
    fn header_body(&mut self) -> PResult<Expr> {
        if !self.eat(&Tok::Colon) && self.at(&Tok::Dot) {
            self.bump();
        }
        if matches!(self.peek(), Tok::Newline | Tok::Eof | Tok::Dedent) {
            let sp = self.prev_span();
            return self.fail_help(
                sp,
                "expected a block here",
                "put the body on the next line with deeper indentation, or wrap it in `{ }`",
            );
        }
        self.body()
    }

    fn next_is_kw_past_newlines(&self, kw: &str) -> bool {
        let mut i = 0;
        while matches!(self.peek_n(i), Tok::Newline) {
            i += 1;
        }
        matches!(self.peek_n(i), Tok::Ident(s) if s == kw)
    }

    /// Items inside `{ ... }` or after `:` in an indented block.
    fn delimited<T>(
        &mut self,
        context: &str,
        mut item: impl FnMut(&mut Parser) -> PResult<T>,
    ) -> PResult<Vec<T>> {
        let end = if self.eat(&Tok::LBrace) {
            Tok::RBrace
        } else if self.eat(&Tok::Colon) {
            self.expect(Tok::Indent, context)?;
            Tok::Dedent
        } else {
            let sp = self.span();
            let found = describe(self.peek());
            return self.fail(
                sp,
                format!("expected `{{` or `:` {}, found {}", context, found),
            );
        };
        let mut out = Vec::new();
        loop {
            while matches!(self.peek(), Tok::Newline | Tok::Semi | Tok::Comma) {
                self.bump();
            }
            if self.at(&end) || self.at(&Tok::Eof) {
                break;
            }
            out.push(item(self)?);
            if !matches!(self.peek(), Tok::Newline | Tok::Semi | Tok::Comma) && !self.at(&end) {
                let sp = self.span();
                let found = describe(self.peek());
                return self.fail(
                    sp,
                    format!(
                        "expected `,` or the end of the list {}, found {}",
                        context, found
                    ),
                );
            }
        }
        self.expect(end, context)?;
        Ok(out)
    }

    // ---------------------------------------------------------------- expressions

    fn expr_or_def(&mut self) -> PResult<Expr> {
        if let Tok::Ident(n) = self.peek().clone() {
            if !is_keyword(&n) && self.peek_n(1) == &Tok::ColonEq {
                let name_span = self.bump().span;
                self.bump();
                let value = self.expr()?;
                return Ok(Expr {
                    span: name_span.to(self.prev_span()),
                    kind: ExprKind::Def(Box::new(Def {
                        name: n,
                        name_span,
                        ty: None,
                        value: Some(value),
                        is_var: false,
                        specs: Vec::new(),
                    })),
                });
            }
        }
        self.expr()
    }

    fn expr(&mut self) -> PResult<Expr> {
        let lhs = self.binary(0)?;
        if self.eat(&Tok::DotDot) {
            let rhs = self.binary(0)?;
            let span = lhs.span.to(rhs.span);
            return Ok(Expr {
                kind: ExprKind::Range(Box::new(lhs), Box::new(rhs)),
                span,
            });
        }
        Ok(lhs)
    }

    fn binary(&mut self, min: u8) -> PResult<Expr> {
        // A long chain `A + B + C ...` builds a left-deep tree, so each
        // link counts toward the nesting limit until the chain ends.
        let base = self.depth;
        let r = self.binary_inner(min);
        self.depth = base;
        r
    }

    fn binary_inner(&mut self, min: u8) -> PResult<Expr> {
        let mut lhs = self.unary()?;
        loop {
            let (op, prec) = match self.peek() {
                Tok::Star => (BinOp::Mul, 9),
                Tok::Slash => (BinOp::Div, 9),
                Tok::Plus => (BinOp::Add, 8),
                Tok::Minus => (BinOp::Sub, 8),
                Tok::Eq => (BinOp::Eq, 7),
                Tok::Ne => (BinOp::Ne, 7),
                Tok::Lt => (BinOp::Lt, 7),
                Tok::Le => (BinOp::Le, 7),
                Tok::Gt => (BinOp::Gt, 7),
                Tok::Ge => (BinOp::Ge, 7),
                Tok::Ident(s) if s == "and" => (BinOp::And, 5),
                Tok::Ident(s) if s == "or" => (BinOp::Or, 4),
                _ => break,
            };
            if prec < min {
                break;
            }
            if self.depth >= MAX_NESTING {
                return self.too_deep();
            }
            self.depth += 1;
            self.bump();
            let rhs = self.binary(prec + 1)?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
                span,
            };
        }
        Ok(lhs)
    }

    fn too_deep<T>(&mut self) -> PResult<T> {
        let sp = self.span();
        // Report once, then unwind quietly.
        if !self.diags.iter().any(|d| d.code == "V0008") {
            self.diags.push(
                Diagnostic::error(
                    "V0008",
                    sp,
                    format!("code is nested more than {} levels deep", MAX_NESTING),
                )
                .with_help(
                    "split the expression into named intermediate values or helper functions",
                ),
            );
        }
        Err(())
    }

    fn unary(&mut self) -> PResult<Expr> {
        if self.depth >= MAX_NESTING {
            return self.too_deep();
        }
        self.depth += 1;
        let r = self.unary_inner();
        self.depth -= 1;
        r
    }

    fn unary_inner(&mut self) -> PResult<Expr> {
        let start = self.span();
        let op = match self.peek() {
            Tok::Minus => UnOp::Neg,
            Tok::Plus => UnOp::Pos,
            Tok::Ident(s) if s == "not" => UnOp::Not,
            _ => return self.postfix(),
        };
        self.bump();
        let inner = self.unary()?;
        let span = start.to(inner.span);
        // Fold `-5` so the most negative integer can be written.
        if let (UnOp::Neg, ExprKind::Int(v)) = (op, &inner.kind) {
            return Ok(Expr {
                kind: ExprKind::Int(v.wrapping_neg()),
                span,
            });
        }
        if let (UnOp::Neg, ExprKind::Float(v)) = (op, &inner.kind) {
            return Ok(Expr {
                kind: ExprKind::Float(-v),
                span,
            });
        }
        Ok(Expr {
            kind: ExprKind::Unary(op, Box::new(inner)),
            span,
        })
    }

    fn args(&mut self, close: Tok) -> PResult<Vec<Arg>> {
        let mut out = Vec::new();
        while !self.at(&close) {
            if self.at(&Tok::Question)
                && matches!(self.peek_n(1), Tok::Ident(_))
                && self.peek_n(2) == &Tok::ColonEq
            {
                self.bump();
                let (name, _) = self.ident("")?;
                self.bump();
                let value = self.expr()?;
                out.push(Arg {
                    name: Some(name),
                    value,
                });
            } else if self.at(&Tok::Question) {
                // `?t` as an argument is an option type, as in `event(?agent){}`.
                let start = self.span().start;
                let qspan = self.span();
                self.bump();
                let inner = self.expr()?;
                let span = Span::new(start, inner.span.end);
                out.push(Arg {
                    name: None,
                    value: Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(Expr {
                                kind: ExprKind::Ident("?".into()),
                                span: qspan,
                            }),
                            args: vec![Arg {
                                name: None,
                                value: inner,
                            }],
                            bracket: false,
                        },
                        span,
                    },
                });
            } else {
                let value = self.expr()?;
                out.push(Arg { name: None, value });
            }
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(close, "to close the argument list")?;
        Ok(out)
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let base = self.depth;
        let r = self.postfix_inner();
        self.depth = base;
        r
    }

    fn postfix_inner(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        loop {
            if self.depth >= MAX_NESTING {
                return self.too_deep();
            }
            self.depth += 1;
            let adjacent = !self.tok_n(0).space_before;
            match self.peek() {
                Tok::Dot if matches!(self.peek_n(1), Tok::Ident(_)) => {
                    self.bump();
                    let (name, nsp) = self.ident("")?;
                    let span = e.span.to(nsp);
                    e = Expr {
                        kind: ExprKind::Member(Box::new(e), name, nsp),
                        span,
                    };
                }
                Tok::LParen if adjacent => {
                    self.bump();
                    let args = self.args(Tok::RParen)?;
                    let span = e.span.to(self.prev_span());
                    e = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(e),
                            args,
                            bracket: false,
                        },
                        span,
                    };
                }
                Tok::LBracket if adjacent => {
                    self.bump();
                    let args = self.args(Tok::RBracket)?;
                    let span = e.span.to(self.prev_span());
                    e = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(e),
                            args,
                            bracket: true,
                        },
                        span,
                    };
                }
                Tok::LBrace
                    if adjacent
                        && matches!(
                            e.kind,
                            ExprKind::Ident(_)
                                | ExprKind::Member(..)
                                | ExprKind::Call { bracket: false, .. }
                        ) =>
                {
                    let fields = self.delimited("in the archetype", |p| {
                        let (name, nsp) = p.ident("for a field")?;
                        if !p.at(&Tok::ColonEq) {
                            let sp = p.span();
                            return p.fail_help(
                                sp,
                                format!("expected `:=` after field `{}`", name),
                                format!(
                                    "fields are initialised with `:=`, for example `{} := Value`",
                                    name
                                ),
                            );
                        }
                        p.bump();
                        let v = p.expr()?;
                        Ok((name, nsp, v))
                    })?;
                    let span = e.span.to(self.prev_span());
                    e = Expr {
                        kind: ExprKind::Archetype {
                            ty: Box::new(e),
                            fields,
                        },
                        span,
                    };
                }
                Tok::Question if adjacent => {
                    let q = self.bump().span;
                    let span = e.span.to(q);
                    e = Expr {
                        kind: ExprKind::Query(Box::new(e)),
                        span,
                    };
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn string(&mut self, parts: Vec<StrPart>) -> Vec<StrSeg> {
        let mut segs = Vec::new();
        for part in parts {
            match part {
                StrPart::Lit(s) => segs.push(StrSeg::Lit(s)),
                StrPart::Expr { src, offset } => {
                    let (toks, diags) = lexer::lex_fragment(&src, offset);
                    self.diags.extend(diags);
                    let mut sub = Parser {
                        toks,
                        pos: 0,
                        diags: Vec::new(),
                        in_body: self.in_body.max(1),
                        depth: self.depth + 1,
                    };
                    let parsed = sub.expr();
                    if parsed.is_ok() && !sub.at(&Tok::Eof) {
                        let sp = sub.span();
                        let found = describe(sub.peek());
                        sub.diags.push(Diagnostic::error(
                            "V0001",
                            sp,
                            format!("expected `}}` to end the interpolation, found {}", found),
                        ));
                    }
                    self.diags.extend(sub.diags);
                    match parsed {
                        Ok(e) => segs.push(StrSeg::Expr(e)),
                        Err(()) => segs.push(StrSeg::Expr(Expr {
                            kind: ExprKind::Error,
                            span: Span::new(offset, offset + src.len() as u32),
                        })),
                    }
                }
            }
        }
        segs
    }

    fn primary(&mut self) -> PResult<Expr> {
        let start = self.span();
        let mk = |kind: ExprKind, span: Span| Ok(Expr { kind, span });
        match self.peek().clone() {
            Tok::Int(v) => {
                self.bump();
                mk(ExprKind::Int(v), start)
            }
            Tok::Float(v) => {
                self.bump();
                mk(ExprKind::Float(v), start)
            }
            Tok::Char(c) => {
                self.bump();
                mk(ExprKind::Char(c), start)
            }
            Tok::Str(parts) => {
                self.bump();
                let segs = self.string(parts);
                mk(ExprKind::Str(segs), start)
            }
            Tok::Path(p) => {
                self.bump();
                mk(ExprKind::Path(p), start)
            }
            Tok::LParen => self.paren(),
            Tok::LBrace => self.brace_block(),
            Tok::Indent => {
                self.bump();
                let stmts = self.stmts_until(End::Dedent);
                self.expect(Tok::Dedent, "to close the block")?;
                mk(ExprKind::Block(stmts), start.to(self.prev_span()))
            }
            Tok::Ident(name) => {
                let opens_block = matches!(self.peek_n(1), Tok::LBrace | Tok::Colon);
                match name.as_str() {
                    "true" | "false" => {
                        self.bump();
                        mk(ExprKind::Logic(name == "true"), start)
                    }
                    "if" => self.if_expr(),
                    "for" => self.for_expr(),
                    "case" => self.case_expr(),
                    "loop" => {
                        self.bump();
                        let body = self.header_body()?;
                        mk(ExprKind::Loop(Box::new(body)), start.to(self.prev_span()))
                    }
                    "block" => {
                        self.bump();
                        let body = self.header_body()?;
                        let span = start.to(self.prev_span());
                        match body.kind {
                            ExprKind::Block(_) => Ok(Expr { kind: body.kind, span }),
                            _ => mk(ExprKind::Block(vec![body]), span),
                        }
                    }
                    "break" => {
                        self.bump();
                        mk(ExprKind::Break, start)
                    }
                    "return" => {
                        self.bump();
                        let has_value = !matches!(
                            self.peek(),
                            Tok::Newline | Tok::Semi | Tok::Dedent | Tok::RBrace | Tok::Eof | Tok::RParen | Tok::Comma
                        ) && !self.at_kw("else");
                        let value = if has_value { Some(Box::new(self.expr()?)) } else { None };
                        mk(ExprKind::Return(value), start.to(self.prev_span()))
                    }
                    "set" => self.set_expr(),
                    "array" if opens_block => {
                        self.bump();
                        let items = self.delimited("in the array", |p| p.expr())?;
                        mk(ExprKind::ArrayLit(items), start.to(self.prev_span()))
                    }
                    "map" if opens_block => {
                        self.bump();
                        let items = self.delimited("in the map", |p| {
                            let k = p.expr()?;
                            if !p.at(&Tok::FatArrow) {
                                let sp = p.span();
                                return p.fail_help(
                                    sp,
                                    "expected `=>` between a map key and its value",
                                    "map literals look like `map{\"a\" => 1, \"b\" => 2}`",
                                );
                            }
                            p.bump();
                            let v = p.expr()?;
                            Ok((k, v))
                        })?;
                        mk(ExprKind::MapLit(items), start.to(self.prev_span()))
                    }
                    "option" if opens_block => {
                        self.bump();
                        let mut items = self.delimited("in `option`", |p| p.expr_or_def())?;
                        let span = start.to(self.prev_span());
                        let inner = match items.len() {
                            0 => None,
                            1 => Some(Box::new(items.remove(0))),
                            _ => Some(Box::new(Expr { kind: ExprKind::Tuple(items), span })),
                        };
                        mk(ExprKind::OptionLit(inner), span)
                    }
                    "logic" if opens_block => {
                        self.bump();
                        let mut items = self.delimited("in `logic`", |p| p.expr_or_def())?;
                        let span = start.to(self.prev_span());
                        if items.len() != 1 {
                            return self.fail_help(span, "`logic{...}` takes exactly one expression", "join several conditions with `and`: `logic{A > 0 and B > 0}`");
                        }
                        mk(ExprKind::LogicLit(Box::new(items.remove(0))), span)
                    }
                    "spawn" if opens_block => {
                        self.bump();
                        let body = self.header_body()?;
                        mk(ExprKind::Spawn(Box::new(body)), start.to(self.prev_span()))
                    }
                    "defer" if opens_block => {
                        self.bump();
                        let body = self.header_body()?;
                        mk(ExprKind::Defer(Box::new(body)), start.to(self.prev_span()))
                    }
                    "sync" | "race" | "rush" | "branch" if opens_block => {
                        let kind = match name.as_str() {
                            "sync" => ConcKind::Sync,
                            "race" => ConcKind::Race,
                            "rush" => ConcKind::Rush,
                            _ => ConcKind::Branch,
                        };
                        self.bump();
                        let body = self.header_body()?;
                        let arms = match body.kind {
                            ExprKind::Block(stmts) => stmts,
                            _ => vec![body],
                        };
                        mk(ExprKind::Concurrent { kind, arms }, start.to(self.prev_span()))
                    }
                    "while" => self.fail_help(
                        start,
                        "Verse has no `while` loop",
                        "use `loop:` with `if (Done?) then break` inside, or `for (I := 0..N)` for a counted loop",
                    ),
                    "class" | "struct" | "interface" | "enum" | "module" => self.fail_help(
                        start,
                        format!("`{}` must be the right-hand side of a definition", name),
                        format!("write `my_name := {}:` followed by an indented body", name),
                    ),
                    "var" => self.fail_help(
                        start,
                        "`var` is only allowed at the start of a statement",
                        "declare the variable on its own line: `var Name : type = Value`",
                    ),
                    "else" | "then" | "do" | "and" | "or" | "where" | "using" => {
                        self.fail(start, format!("unexpected `{}`", name))
                    }
                    _ => {
                        self.bump();
                        mk(ExprKind::Ident(name), start)
                    }
                }
            }
            Tok::Spec(s) => self.fail_help(
                start,
                format!("specifier `<{}>` is not allowed here", s),
                "specifiers attach to definitions: `Name<public>(Params)<decides> : type = Body`",
            ),
            other => {
                let found = describe(&other);
                self.fail(start, format!("expected an expression, found {}", found))
            }
        }
    }

    fn paren(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        if self.eat(&Tok::RParen) {
            return Ok(Expr {
                kind: ExprKind::Unit,
                span: start.to(self.prev_span()),
            });
        }
        if matches!(self.peek(), Tok::Ident(_))
            && self.peek_n(1) == &Tok::Colon
            && self.peek_n(2) == &Tok::RParen
        {
            let (qual, qsp) = self.ident("")?;
            self.bump();
            self.bump();
            if qual != "super" {
                self.diags.push(Diagnostic::error(
                    "V6001",
                    qsp,
                    "qualified names `(Qualifier:)Name` are only supported for `(super:)`",
                ));
                return Err(());
            }
            let (method, _) = self.ident("after `(super:)`")?;
            self.expect(Tok::LParen, "to call the parent method")?;
            let args = self.args(Tok::RParen)?;
            return Ok(Expr {
                kind: ExprKind::SuperCall { method, args },
                span: start.to(self.prev_span()),
            });
        }
        let first = self.expr_or_def()?;
        if self.at(&Tok::Comma) {
            let mut items = vec![first];
            while self.eat(&Tok::Comma) {
                if self.at(&Tok::RParen) {
                    break;
                }
                items.push(self.expr_or_def()?);
            }
            self.expect(Tok::RParen, "to close the tuple")?;
            return Ok(Expr {
                kind: ExprKind::Tuple(items),
                span: start.to(self.prev_span()),
            });
        }
        if self.at(&Tok::Semi) {
            let mut items = vec![first];
            while self.eat(&Tok::Semi) {
                if self.at(&Tok::RParen) {
                    break;
                }
                items.push(self.expr_or_def()?);
            }
            self.expect(Tok::RParen, "to close the parenthesis")?;
            return Ok(Expr {
                kind: ExprKind::Block(items),
                span: start.to(self.prev_span()),
            });
        }
        self.expect(Tok::RParen, "to close the parenthesis")?;
        Ok(first)
    }

    fn set_expr(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        let target = self.postfix()?;
        let op = match self.peek() {
            Tok::Eq => None,
            Tok::PlusEq => Some(BinOp::Add),
            Tok::MinusEq => Some(BinOp::Sub),
            Tok::StarEq => Some(BinOp::Mul),
            Tok::SlashEq => Some(BinOp::Div),
            other => {
                let found = describe(other);
                let sp = self.span();
                return self.fail_help(
                    sp,
                    format!("expected `=` after the target of `set`, found {}", found),
                    "write `set Name = Value` or `set Name += Value`",
                );
            }
        };
        self.bump();
        let value = self.expr()?;
        Ok(Expr {
            span: start.to(self.prev_span()),
            kind: ExprKind::Set {
                target: Box::new(target),
                op,
                value: Box::new(value),
            },
        })
    }

    fn if_expr(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        let mut arms = Vec::new();
        let mut else_body = None;
        loop {
            let cond = if self.eat(&Tok::LParen) {
                let mut c = Vec::new();
                while !self.at(&Tok::RParen) {
                    c.push(self.expr_or_def()?);
                    if !self.eat(&Tok::Comma) && !self.eat(&Tok::Semi) {
                        break;
                    }
                }
                self.expect(Tok::RParen, "to close the condition")?;
                c
            } else if self.at(&Tok::Colon) && self.peek_n(1) == &Tok::Indent {
                self.bump();
                self.bump();
                let c = self.stmts_until(End::Dedent);
                self.expect(Tok::Dedent, "to close the condition block")?;
                while self.at(&Tok::Newline) {
                    self.bump();
                }
                if !self.at_kw("then") {
                    let sp = self.span();
                    return self.fail(sp, "expected `then:` after the `if:` condition block");
                }
                c
            } else {
                let sp = self.span();
                return self.fail_help(
                    sp,
                    "expected `(` after `if`",
                    "the condition goes in parentheses: `if (Condition) { ... }` or `if (Condition):` with an indented body",
                );
            };
            self.eat_kw("then");
            let body = self.header_body()?;
            arms.push(IfArm { cond, body });
            if self.next_is_kw_past_newlines("else") {
                while self.at(&Tok::Newline) {
                    self.bump();
                }
                self.bump();
                if self.at_kw("if") {
                    self.bump();
                    continue;
                }
                else_body = Some(Box::new(self.header_body()?));
            }
            break;
        }
        Ok(Expr {
            kind: ExprKind::If { arms, else_body },
            span: start.to(self.prev_span()),
        })
    }

    fn for_expr(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        if !self.at(&Tok::LParen) {
            let sp = self.span();
            return self.fail_help(
                sp,
                "expected `(` after `for`",
                "write `for (Item : Items):`, `for (I := 0..9):` or `for (Index -> Item : Items):`",
            );
        }
        self.bump();
        let mut clauses = Vec::new();
        while !self.at(&Tok::RParen) {
            clauses.push(self.for_clause()?);
            if self.at(&Tok::Semi)
                && clauses.len() == 1
                && matches!(self.peek_n(2), Tok::Lt | Tok::Le | Tok::Gt | Tok::Ge)
            {
                let sp = self.span();
                return self.fail_help(
                    sp,
                    "C-style `for (init; condition; step)` loops do not exist in Verse",
                    "iterate a range instead: `for (I := 0..Count - 1):` (both ends are included)",
                );
            }
            if !self.eat(&Tok::Comma) && !self.eat(&Tok::Semi) {
                break;
            }
        }
        self.expect(Tok::RParen, "to close the `for` header")?;
        self.eat_kw("do");
        let body = self.header_body()?;
        Ok(Expr {
            kind: ExprKind::For {
                clauses,
                body: Box::new(body),
            },
            span: start.to(self.prev_span()),
        })
    }

    fn for_clause(&mut self) -> PResult<ForClause> {
        if let Tok::Ident(n) = self.peek().clone() {
            if !is_keyword(&n) {
                if self.peek_n(1) == &Tok::Colon {
                    let name_span = self.bump().span;
                    self.bump();
                    let iter = self.expr()?;
                    return Ok(ForClause::Gen {
                        key: None,
                        name: n,
                        name_span,
                        iter,
                    });
                }
                if self.peek_n(1) == &Tok::ThinArrow
                    && matches!(self.peek_n(2), Tok::Ident(_))
                    && self.peek_n(3) == &Tok::Colon
                {
                    let ksp = self.bump().span;
                    self.bump();
                    let (name, name_span) = self.ident("")?;
                    self.bump();
                    let iter = self.expr()?;
                    return Ok(ForClause::Gen {
                        key: Some((n, ksp)),
                        name,
                        name_span,
                        iter,
                    });
                }
                if self.peek_n(1) == &Tok::ColonEq {
                    let name_span = self.bump().span;
                    self.bump();
                    let value = self.expr()?;
                    if matches!(value.kind, ExprKind::Range(..)) {
                        return Ok(ForClause::Gen {
                            key: None,
                            name: n,
                            name_span,
                            iter: value,
                        });
                    }
                    let span = name_span.to(value.span);
                    return Ok(ForClause::Expr(Expr {
                        span,
                        kind: ExprKind::Def(Box::new(Def {
                            name: n,
                            name_span,
                            ty: None,
                            value: Some(value),
                            is_var: false,
                            specs: Vec::new(),
                        })),
                    }));
                }
                if self.peek_n(1) == &Tok::Eq
                    && matches!(self.peek_n(2), Tok::Int(_))
                    && self.peek_n(3) == &Tok::Semi
                {
                    let sp = self.span();
                    return self.fail_help(
                        sp,
                        "C-style `for (init; condition; step)` loops do not exist in Verse",
                        "iterate a range instead: `for (I := 0..Count - 1):` (both ends are included)",
                    );
                }
            }
        }
        Ok(ForClause::Expr(self.expr()?))
    }

    fn case_expr(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        self.expect(Tok::LParen, "after `case`")?;
        let scrutinee = self.expr()?;
        self.expect(Tok::RParen, "after the value being matched")?;
        let arms = self.delimited("in `case`", |p| {
            let astart = p.span();
            let pattern = if p.at_kw("_") {
                p.bump();
                None
            } else {
                Some(p.expr()?)
            };
            if !p.at(&Tok::FatArrow) {
                let sp = p.span();
                return p.fail_help(
                    sp,
                    "expected `=>` after the case pattern",
                    "arms look like `Pattern => Result`, with `_` as the default",
                );
            }
            p.bump();
            let body = p.body()?;
            Ok(CaseArm {
                pattern,
                body,
                span: astart.to(p.prev_span()),
            })
        })?;
        Ok(Expr {
            kind: ExprKind::Case {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span: start.to(self.prev_span()),
        })
    }
}
