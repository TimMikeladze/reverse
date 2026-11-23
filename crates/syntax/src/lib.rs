//! Front end for the Verse language: lexer, layout pass, parser and AST.
//!
//! This is an independent implementation written from public documentation
//! (the Book of Verse and Epic's Verse reference). It is not affiliated with
//! Epic Games.

pub mod ast;
pub mod diag;
pub mod lexer;
pub mod parser;

pub use diag::{Diagnostic, Fix, LineIndex, Severity, Span};
pub use parser::{is_keyword, parse};
