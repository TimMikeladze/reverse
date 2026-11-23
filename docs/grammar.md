# Grammar notes

Working notes on Verse syntax as this parser reads it, in our own words, from the Book of
Verse chapters *Expressions*, *Operators*, *Functions*, *Control Flow* and *Classes &
Interfaces*. Where the parser accepts less than the Book describes, that is stated.

## Lexical

- Identifiers: ASCII letter or `_`, then letters, digits, `_`. Case-sensitive.
- Comments: `#` to end of line; `<# ... #>` block comments, which nest.
- Integers: decimal or `0x` hex, must fit 64 bits. Floats: digits, `.`, digits, optional
  exponent, optional `f64` suffix. A float needs the decimal point.
- Characters: `'a'`. Strings: `"..."` with `{Expr}` interpolation and the escapes
  `\n \t \r \" \' \\ \{ \} \< \> \& \# \~`.
- Paths: `/Verse.org/Random`, recognised where a value is expected.
- Specifiers: `<name>` written directly after an identifier, `)` or another specifier.
  A `<` preceded by a space is always the comparison operator.

## Layout

A block is one of:

1. `{ ... }` with items separated by newlines, `;` or `,`;
2. a `:` or `=` at the end of a line, followed by lines indented more deeply;
3. a single expression on the same line.

The lexer emits `Indent`, `Dedent` and `Newline` tokens. Parentheses and square brackets
suppress layout. Braces start a fresh layout context. A line ending in a binary operator or
a comma continues on the next line.

## Precedence, highest first

| Level | Operators | Associativity |
| --- | --- | --- |
| postfix | `.name`  `(args)`  `[args]`  `{fields}`  `?` | left |
| prefix | `-`  `+`  `not` | right |
| multiplicative | `*`  `/` | left |
| additive | `+`  `-` | left |
| comparison | `=`  `<>`  `<`  `<=`  `>`  `>=` | left |
| `and` | | left |
| `or` | | left |
| range | `..` | none |
| definition | `:=`, `set ... =` | right |

Postfix operators must touch their operand: `F (X)` is not a call, and `Name {` is not an
archetype. This is how `if (X) { ... }` is told apart from instantiating `X`.

## Definitions

```
Name : type = value                      constant
Name := value                            constant, inferred type
var Name : type = value                  variable
set place = value                        assignment; also += -= *= /=
Name<access>(Param : type, ?Named : type = default)<effects> : type = body
Name := class<specs>(parent, interface): members
Name := struct: fields
Name := interface: members
Name := enum{A, B, C}
Name := module: definitions
using { /Path/To/Module }
```

At statement start, `Name(...)` followed by `:` (after optional effect specifiers) is a
function definition; anything else is a call. At module level and in class bodies,
`Name(...) = body` is also read as a definition so the missing return type can be reported.

## Expressions

```
if (cond, cond) body [else if (...) body] [else body]      also `if: ... then: ... else: ...`
for (X : Xs) body          for (I -> X : Xs)      for (K -> V : Map)      for (I := A..B)
for (X : Xs, filter, Y := expr) body
loop: body                 break                  return [value]
case (value): pattern => body ... _ => body
block: body
array{a, b}    map{k => v}    option{x}    logic{x}    (a, b)    (a; b)
Type{Field := value}       (super:)Method(args)
sync: / race: / rush: / branch: arms               spawn{expr}        defer: body
```

## Also parsed

```
(Receiver : type).Name(Params) : type = Body        extension method
Name<constructor>(Params) := my_class:              constructor function
    Field := Value
Name(X : t where t : type) : t = Body               parametric function
defer: body
event(int){}                                        a new event
```

## Not parsed yet

Tuple-destructuring parameters, qualified names `(Qualifier:)Name` other than `(super:)`,
`for:` / `do:` block headers, quoted identifier suffixes `name'...'`, the character forms
`0oXX` and `0uXXXX`, `<#>` indented comments (read as a comment to end of line), lambdas,
parametric classes, `first(...)`.
