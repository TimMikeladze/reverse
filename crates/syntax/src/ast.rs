//! Abstract syntax tree. Every node carries its source span.

use crate::diag::Span;

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Pos,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Eq => "=",
            BinOp::Ne => "<>",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
        }
    }
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        )
    }
}

#[derive(Clone, Debug)]
pub enum StrSeg {
    Lit(String),
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct Spec {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypeExpr {
    /// `int`, `my_class`, `event(int)` ...
    Named {
        name: String,
        args: Vec<TypeExpr>,
        span: Span,
    },
    Array(Box<TypeExpr>, Span),
    Map(Box<TypeExpr>, Box<TypeExpr>, Span),
    Option(Box<TypeExpr>, Span),
    Tuple(Vec<TypeExpr>, Span),
    /// Function types and anything else parsed but not modelled yet.
    Opaque(Span),
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named { span, .. }
            | TypeExpr::Array(_, span)
            | TypeExpr::Map(_, _, span)
            | TypeExpr::Option(_, span)
            | TypeExpr::Tuple(_, span)
            | TypeExpr::Opaque(span) => *span,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Arg {
    /// `Some` for `?Name := value`.
    pub name: Option<String>,
    pub value: Expr,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub span: Span,
    pub ty: TypeExpr,
    pub named: bool,
    pub default: Option<Expr>,
}

#[derive(Clone, Debug)]
pub struct FuncDef {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    /// Names bound by a `where` clause.
    pub type_vars: Vec<String>,
    /// Specifiers before the parameter list (`<public>`, `<override>` ...).
    pub pre_specs: Vec<Spec>,
    /// Effect specifiers after the parameter list.
    pub effects: Vec<Spec>,
    pub ret: Option<TypeExpr>,
    pub body: Option<Expr>,
    /// An extension method `(Receiver : type).Name(...)`; the receiver is
    /// the first parameter.
    pub extension: bool,
}

#[derive(Clone, Debug)]
pub struct Def {
    pub name: String,
    pub name_span: Span,
    pub ty: Option<TypeExpr>,
    pub value: Option<Expr>,
    pub is_var: bool,
    pub specs: Vec<Spec>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    Struct,
    Interface,
}

impl ClassKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ClassKind::Class => "class",
            ClassKind::Struct => "struct",
            ClassKind::Interface => "interface",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClassDef {
    pub name: String,
    pub name_span: Span,
    pub kind: ClassKind,
    pub specs: Vec<Spec>,
    pub parents: Vec<(String, Span)>,
    /// Fields (`Def`), methods (`Func`) and `block` clauses.
    pub members: Vec<Expr>,
}

#[derive(Clone, Debug)]
pub struct EnumDef {
    pub name: String,
    pub name_span: Span,
    pub specs: Vec<Spec>,
    pub values: Vec<(String, Span)>,
}

#[derive(Clone, Debug)]
pub struct IfArm {
    pub cond: Vec<Expr>,
    pub body: Expr,
}

#[derive(Clone, Debug)]
pub enum ForClause {
    /// `X : Xs`, `I -> X : Xs`, `K -> V : Map`, `I := 0..9`
    Gen {
        key: Option<(String, Span)>,
        name: String,
        name_span: Span,
        iter: Expr,
    },
    /// A definition (`Y := F(X)`) or a filter (`X > 2`).
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct CaseArm {
    /// `None` is the `_` wildcard.
    pub pattern: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConcKind {
    Sync,
    Race,
    Rush,
    Branch,
}

impl ConcKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ConcKind::Sync => "sync",
            ConcKind::Race => "race",
            ConcKind::Rush => "rush",
            ConcKind::Branch => "branch",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Char(char),
    Str(Vec<StrSeg>),
    Logic(bool),
    /// `()`
    Unit,
    Ident(String),
    Path(String),
    Tuple(Vec<Expr>),
    /// A sequence with its own scope; yields its last value.
    Block(Vec<Expr>),
    Def(Box<Def>),
    Set {
        target: Box<Expr>,
        op: Option<BinOp>,
        value: Box<Expr>,
    },
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
    /// `X?`
    Query(Box<Expr>),
    /// `F(a)` or, with `bracket`, `F[a]`.
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
        bracket: bool,
    },
    Member(Box<Expr>, String, Span),
    /// `(super:)Method(args)`
    SuperCall {
        method: String,
        args: Vec<Arg>,
    },
    If {
        arms: Vec<IfArm>,
        else_body: Option<Box<Expr>>,
    },
    For {
        clauses: Vec<ForClause>,
        body: Box<Expr>,
    },
    Loop(Box<Expr>),
    Break,
    Return(Option<Box<Expr>>),
    Case {
        scrutinee: Box<Expr>,
        arms: Vec<CaseArm>,
    },
    ArrayLit(Vec<Expr>),
    MapLit(Vec<(Expr, Expr)>),
    /// `option{X}`
    OptionLit(Option<Box<Expr>>),
    /// `logic{X}`
    LogicLit(Box<Expr>),
    /// `my_class{Field := value}`
    Archetype {
        ty: Box<Expr>,
        fields: Vec<(String, Span, Expr)>,
    },
    Func(Box<FuncDef>),
    Class(Box<ClassDef>),
    Enum(Box<EnumDef>),
    Module {
        name: String,
        items: Vec<Expr>,
    },
    Using(String),
    Concurrent {
        kind: ConcKind,
        arms: Vec<Expr>,
    },
    Spawn(Box<Expr>),
    Defer(Box<Expr>),
    /// Placeholder left behind by error recovery.
    Error,
}

#[derive(Clone, Debug, Default)]
pub struct Module {
    pub items: Vec<Expr>,
}
