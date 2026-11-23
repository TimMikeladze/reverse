//! Signatures of the built-in functions this toolchain knows.
//!
//! These cover the language-level library described in the Book of Verse.
//! Fortnite and Unreal Engine APIs are not here: they belong to digest files
//! that ship with UEFN and are never bundled with this project.

use crate::types::*;
use crate::Checker;
use reverse_syntax::ast::*;

/// Every built-in name, for "did you mean" suggestions.
pub const ALL: &[&str] = &[
    "Print",
    "ToString",
    "Abs",
    "Min",
    "Max",
    "Floor",
    "Ceil",
    "Round",
    "Int",
    "Sqrt",
    "Pow",
    "Sin",
    "Cos",
    "Tan",
    "ArcSin",
    "ArcCos",
    "ArcTan",
    "Exp",
    "Ln",
    "Mod",
    "Quotient",
    "GetRandomInt",
    "GetRandomFloat",
    "Shuffle",
    "Sleep",
    "GetSimulationElapsedTime",
    "Join",
    "Concatenate",
    "ConcatenateMaps",
];

const GENERIC: &[&str] = &[
    "ToString",
    "Abs",
    "Min",
    "Max",
    "Floor",
    "Ceil",
    "Concatenate",
    "ConcatenateMaps",
    "Shuffle",
];

pub fn is_generic(name: &str) -> bool {
    GENERIC.contains(&name)
}

pub fn is_builtin(name: &str) -> bool {
    ALL.contains(&name)
}

/// Built-ins with one fixed signature.
pub fn fixed_sig(name: &str) -> Option<FuncSig> {
    use Ty::*;
    let c = Effects::COMPUTES;
    let t = Effects::TRANSACTS;
    Some(match name {
        "Print" => {
            let mut sig = FuncSig::simple("Print", &[Str], Void, t);
            sig.params[0].name = "Message".into();
            sig.params.push(ParamSig {
                name: "Duration".into(),
                ty: Float,
                named: true,
                has_default: true,
            });
            sig.params.push(ParamSig {
                name: "Color".into(),
                ty: Unknown,
                named: true,
                has_default: true,
            });
            sig
        }
        "Sqrt" | "Sin" | "Cos" | "Tan" | "ArcSin" | "ArcCos" | "ArcTan" | "Exp" | "Ln" => {
            FuncSig::simple(name, &[Float], Float, c)
        }
        "Pow" => FuncSig::simple(name, &[Float, Float], Float, c),
        "Mod" | "Quotient" => FuncSig::simple(name, &[Int, Int], Int, c.decides()),
        "Round" | "Int" => FuncSig::simple(name, &[Float], Int, c.decides()),
        "GetRandomInt" => FuncSig::simple(name, &[Int, Int], Int, t),
        "GetRandomFloat" => FuncSig::simple(name, &[Float, Float], Float, t),
        "Sleep" => FuncSig::simple(name, &[Float], Void, c.suspends()),
        "GetSimulationElapsedTime" => FuncSig::simple(
            name,
            &[],
            Float,
            Effects {
                decides: false,
                suspends: false,
                reads: true,
                writes: false,
                allocates: false,
            },
        ),
        "Join" => FuncSig::simple(name, &[Array(Box::new(Str)), Str], Str, c),
        _ => return None,
    })
}

fn arg_types(c: &mut Checker, args: &[Arg]) -> Vec<Ty> {
    args.iter().map(|a| c.expr(&a.value, None)).collect()
}

/// Built-ins whose result type depends on the argument types.
pub(crate) fn generic_call(
    c: &mut Checker,
    name: &str,
    e: &Expr,
    callee_end: u32,
    args: &[Arg],
    bracket: bool,
) -> Ty {
    use Ty::*;
    // Type the arguments once without reporting, to pick a signature.
    let n = c.diags.len();
    let seen = c.failable_seen;
    let ts = arg_types(c, args);
    c.diags.truncate(n);
    c.failable_seen = seen;
    let first = ts.first().cloned().unwrap_or(Unknown);
    let comp = Effects::COMPUTES;
    let sig = match name {
        "ToString" => {
            if ts.len() == 1 && !c.stringable(&first) {
                arg_types(c, args);
                let shown = c.show(&first);
                c.err(
                    "V2007",
                    e.span,
                    format!("there is no `ToString` for type `{}`", shown),
                )
                .help = Some(match first {
                    Logic => "write `if (Flag?) then \"true\" else \"false\"`".into(),
                    _ => format!("define `ToString(Value : {}) : string` yourself", shown),
                });
                return Str;
            }
            FuncSig::simple(name, &[first], Str, comp)
        }
        "Abs" => match first {
            Float => FuncSig::simple(name, &[Float], Float, comp),
            _ => FuncSig::simple(name, &[Int], Int, comp),
        },
        "Min" | "Max" => match first {
            Float => FuncSig::simple(name, &[Float, Float], Float, comp),
            _ => FuncSig::simple(name, &[Int, Int], Int, comp),
        },
        "Floor" | "Ceil" => match first {
            // On a float these can fail (non-finite input), so they use `[]`.
            Float => FuncSig::simple(name, &[Float], Int, comp.decides()),
            _ => FuncSig::simple(name, &[Rational], Int, comp),
        },
        "Shuffle" => FuncSig::simple(
            name,
            std::slice::from_ref(&first),
            first.clone(),
            Effects::TRANSACTS,
        ),
        "Concatenate" => {
            let mut elem = Never;
            for t in &ts {
                match t {
                    Array(inner) => elem = c.join(&elem, inner),
                    Str => elem = c.join(&elem, &Char),
                    _ => {}
                }
            }
            let arr = if ts.iter().all(|t| *t == Str) && !ts.is_empty() {
                Str
            } else {
                Array(Box::new(elem))
            };
            FuncSig::simple(name, &vec![arr.clone(); ts.len().max(1)], arr, comp)
        }
        "ConcatenateMaps" => {
            FuncSig::simple(name, &vec![first.clone(); ts.len().max(1)], first, comp)
        }
        _ => FuncSig::simple(name, &[], Unknown, comp),
    };
    c.apply_sig(&sig, e, callee_end, args, bracket, true)
}

/// Methods on arrays and strings.
pub(crate) fn array_method(_c: &mut Checker, recv: &Ty, name: &str) -> Option<FuncSig> {
    use Ty::*;
    let elem = match recv {
        Array(t) => (**t).clone(),
        Str => Char,
        _ => return None,
    };
    let s = recv.clone();
    let c = Effects::COMPUTES;
    let d = c.decides();
    Some(match name {
        "Find" => FuncSig::simple(name, &[elem], Int, d),
        "Slice" => {
            let mut sig = FuncSig::simple(name, &[Int, Int], s, d);
            sig.params[1].has_default = true;
            sig
        }
        "RemoveFirstElement" => FuncSig::simple(name, &[elem], s, d),
        "RemoveAllElements" => FuncSig::simple(name, &[elem], s, c),
        "RemoveElement" => FuncSig::simple(name, &[Int], s, d),
        "Remove" => FuncSig::simple(name, &[Int, Int], s, d),
        "ReplaceElement" => FuncSig::simple(name, &[Int, elem], s, d),
        "ReplaceFirstElement" => FuncSig::simple(name, &[elem.clone(), elem], s, d),
        "ReplaceAllElements" => FuncSig::simple(name, &[elem.clone(), elem], s, c),
        "Insert" => FuncSig::simple(name, &[Int, s.clone()], s, d),
        _ => return None,
    })
}
