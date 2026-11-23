# Diagnostics

Every problem the checker reports has a stable code. `reverse explain <CODE>` prints
the same text as this page, and the `verse_explain` tool returns it to an agent.
Codes marked *warning* do not stop a program from running.

| Range | Area |
| --- | --- |
| `V0xxx` | Syntax and layout |
| `V1xxx` | Names and definitions |
| `V2xxx` | Types and calls |
| `V3xxx` | Failure contexts |
| `V4xxx` | Effects, specifiers and concurrency |
| `V5xxx` | Assignment and control flow |
| `V6xxx` | Unsupported constructs and Fortnite APIs |
| `V7xxx` | Inheritance and access |
| `V8xxx` | `case` |
| `V9xxx` | Structure and unreachable code |

## V0001: syntax error

The parser found a token it did not expect. The message names what it expected. Each statement is reported once; fix the first error on a line before reading later ones.

Raises it:

```verse
F() : int =
    (1 +
```

Fixed:

```verse
F() : int =
    (1 + 2)
```

## V0002: indentation error

Indentation opens a block only after a line that ends in `:` or `=`. Every line of a block must use the same indentation, and a dedent must return to the indentation of an enclosing block.

Raises it:

```verse
F() : int =
    X := 1
        X
```

Fixed:

```verse
F() : int =
    X := 1
    X
```

## V0003: unterminated literal or comment

A string, character literal, interpolation `{` or block comment `<#` was opened and never closed.

Raises it:

```verse
F() : string = "abc
```

Fixed:

```verse
F() : string = "abc"
```

## V0004: `//` is not a comment

Verse line comments start with `#`. Block comments are written `<# ... #>` and may nest.

Raises it:

```verse
// note
F() : int = 1
```

Fixed:

```verse
# note
F() : int = 1
```

## V0005: operator from another language

Verse spells these differently: equality is `=`, inequality is `<>`, logical operators are the words `and`, `or`, `not`, and there is no `++` or `--` (write `set X += 1`).

Raises it:

```verse
F(A : int, B : int)<decides> : int = A == B
```

Fixed:

```verse
F(A : int, B : int)<decides> : int = A = B
```

## V0006: integer literal out of range

Integer literals must fit in a signed 64-bit integer.

Raises it:

```verse
X : int = 99999999999999999999
```

Fixed:

```verse
X : int = 9999999999
```

## V0007: unknown escape sequence

The escapes are \n \t \r \" \' \\ \{ \} \< \> \& \# \~.

Raises it:

```verse
X : string = "a\qb"
```

Fixed:

```verse
X : string = "a\nb"
```

## V0008: nesting too deep

Expressions, blocks and types may nest at most 200 levels. Split the code into named values or helper functions.

## V1001: unknown name

The name is not defined in this scope. Names are case-sensitive. Verse has no `null` (use an empty option, written `false`), and the current object is `Self`.

Raises it:

```verse
F() : int = Cuont
Count : int = 1
```

Fixed:

```verse
F() : int = Count
Count : int = 1
```

## V1002: unknown type

The type name is not defined. The primitive types are `int`, `float`, `string`, `logic`, `char`, `void` and `any`, all lowercase.

Raises it:

```verse
F(Flag : bool) : void = {}
```

Fixed:

```verse
F(Flag : logic) : void = {}
```

## V1003: unknown member

The type has no field, method or enum value with that name. Containers expose `.Length`; arrays also have methods such as `Find[]` and `Slice[]`.

Raises it:

```verse
F(Xs : []int) : int = Xs.Count
```

Fixed:

```verse
F(Xs : []int) : int = Xs.Length
```

## V1004: duplicate definition

A name may be defined only once in a scope, and a local name may not shadow another local. To change a value, declare it with `var` and update it with `set`.

Raises it:

```verse
F() : int =
    X := 1
    X := 2
    X
```

Fixed:

```verse
F() : int =
    var X : int = 1
    set X = 2
    X
```

## V1005: invalid inheritance

A class may inherit from one class and any number of interfaces. Structs have no inheritance and no methods.

Raises it:

```verse
a := class {}
b := class {}
c := class(a, b) {}
```

Fixed:

```verse
a := class {}
b := interface {}
c := class(a, b) {}
```

## V2001: type mismatch

A value's type does not fit where it is used. Verse never converts implicitly: int to float is `X * 1.0`, float to int is `Floor[X]` / `Round[X]`, anything to string is interpolation `"{X}"`, and a value becomes an option with `option{X}`.

Raises it:

```verse
X : float = 1
```

Fixed:

```verse
X : float = 1.0
```

## V2002: wrong arguments

The call passes the wrong number of arguments, or a named argument the function does not declare. Named arguments are written `?Name := Value` and come last.

Raises it:

```verse
Add(A : int, B : int) : int = A + B
F() : int = Add(1)
```

Fixed:

```verse
Add(A : int, B : int) : int = A + B
F() : int = Add(1, 2)
```

## V2003: wrong call or index syntax

Arrays, maps and strings are indexed with square brackets (`Items[0]`). Tuple elements use parentheses (`Pair(0)`). Options must be unwrapped with `?` before use.

Raises it:

```verse
F(Xs : []int)<decides> : int = Xs(0)
```

Fixed:

```verse
F(Xs : []int)<decides> : int = Xs[0]
```

## V2004: operator not defined for these types

`+ - *` need two ints or two floats (`*` also accepts int with float). `+` joins two strings or two arrays, but never a string and a number: use interpolation. `< <= > >=` order numbers only.

Raises it:

```verse
F(Score : int) : string = "Score: " + Score
```

Fixed:

```verse
F(Score : int) : string = "Score: {Score}"
```

## V2005: missing required field

A field declared without a default value must be given when the class or struct is instantiated.

Raises it:

```verse
hero := class:
    Name : string
F() : hero = hero{}
```

Fixed:

```verse
hero := class:
    Name : string
F() : hero = hero{Name := "Ann"}
```

## V2006: wrong return value

A function returns the value of its last expression, which must match the declared return type. A `set`, an `if` without `else`, or a `for` used as the last line does not produce the value you may expect.

Raises it:

```verse
F(X : int) : int =
    if (X > 0):
        X
```

Fixed:

```verse
F(X : int) : int =
    if (X > 0) then X else 0
```

## V2007: value cannot become a string

String interpolation and `ToString` work for int, float, char and string. `logic`, options, containers and classes have no built-in conversion. Define your own `ToString(Value : my_type) : string`, or convert by hand.

Raises it:

```verse
F(Flag : logic) : string = "{Flag}"
```

Fixed:

```verse
F(Flag : logic) : string = if (Flag?) then "true" else "false"
```

## V2008: comparison always fails

Comparing values of different types compiles but can never succeed, so it is almost always a mistake. Convert one side first.

Raises it:

```verse
F(X : int)<decides> : int = X = 1.0
```

Fixed:

```verse
F(X : int)<decides> : int = X = 1
```

## V2009: range used as a value

`A..B` is not a value. It can only be iterated: `for (I := A..B)`. Both ends are included.

Raises it:

```verse
F() : void =
    R := 1..3
```

Fixed:

```verse
F() : []int = for (I := 1..3) { I }
```

## V2010: `case` on an unsupported type

`case` matches int, logic, char, string and enum values. For floats, tuples and objects use `if` / `else if`.

Raises it:

```verse
F(X : float) : int =
    case (X):
        1.0 => 1
        _ => 0
```

Fixed:

```verse
F(X : float) : int =
    if (X = 1.0) then 1 else 0
```

## V2011: cannot instantiate

Abstract classes and interfaces cannot be instantiated; create a concrete subclass. Enum values are written `my_enum.Value`.

Raises it:

```verse
shape := class<abstract>:
    Area() : float
F() : shape = shape{}
```

Fixed:

```verse
shape := class<abstract>:
    Area() : float
dot := class(shape):
    Area<override>() : float = 0.0
F() : shape = dot{}
```

## V2012: missing body or initial value

Functions need a body after `=` and local definitions need an initial value. Only methods of abstract classes and interfaces may omit the body, and only class fields may omit the value.

Raises it:

```verse
F() : int
```

Fixed:

```verse
F() : int = 0
```

## V2013: missing return type

Write the return type after the parameter list: `Name(Params) : type = Body`. Use `void` when there is nothing to return.

Raises it:

```verse
F()<computes> = 1
```

Fixed:

```verse
F()<computes> : int = 1
```

## V2014: cannot infer the type of an empty literal

`array{}` and `map{}` carry no element type. Give the definition an explicit type.

Raises it:

```verse
F() : void =
    Xs := array{}
```

Fixed:

```verse
F() : void =
    Xs : []int = array{}
```

## V2015: comparing instances of a class that is not `<unique>`

Class instances can be compared with `=` and `<>` only when the class is declared `<unique>`, and then by identity: two separately created instances are never equal. Structs compare field by field and need no specifier.

Raises it:

```verse
thing := class:
    ID : int = 0
Same(A : thing, B : thing)<decides> : void = A = B
```

Fixed:

```verse
thing := class<unique>:
    ID : int = 0
Same(A : thing, B : thing)<decides> : void = A = B
```

## V2016: no matching overload

The function has several definitions, and none accepts these argument types. Overloads are chosen by the number and types of the positional arguments.

Raises it:

```verse
Show(X : int) : string = "int"
Show(X : float) : string = "float"
F() : string = Show("text")
```

Fixed:

```verse
Show(X : int) : string = "int"
Show(X : float) : string = "float"
F() : string = Show(1.5)
```

## V3001: failable expression outside a failure context

Comparisons, `Opt?`, indexing `Items[I]`, map lookups, integer division and calls to `<decides>` functions can fail. They are only allowed where failure is handled: an `if` condition, a `for` header, the left side of `or`, inside `option{}` or `logic{}`, or the body of a `<decides>` function.

Raises it:

```verse
First(Xs : []int) : int = Xs[0]
```

Fixed:

```verse
First(Xs : []int) : int = Xs[0] or 0
```

## V3002: `<decides>` function called with parentheses

A function that can fail is called with square brackets, which marks the call as failable at the call site: `Find[Key]`, not `Find(Key)`.

Raises it:

```verse
Half(X : int)<decides> : int = Mod[X, 2] = 0
F() : int = Half(4) or 0
```

Fixed:

```verse
Half(X : int)<decides> : int = Mod[X, 2] = 0
F() : int = Half[4] or 0
```

## V3003: square brackets on a call that cannot fail

Only `<decides>` functions, indexing and casts use `[]`. Ordinary calls use parentheses.

Raises it:

```verse
Double(X : int) : int = X * 2
F() : int = Double[4]
```

Fixed:

```verse
Double(X : int) : int = X * 2
F() : int = Double(4)
```

## V3004: condition cannot fail

`if` and `not` test success or failure, not truth. A `logic` value does not fail by itself: query it with `?`. `if (Flag)` is an error; `if (Flag?)` is correct.

Raises it:

```verse
F(Flag : logic) : int =
    if (Flag) then 1 else 0
```

Fixed:

```verse
F(Flag : logic) : int =
    if (Flag?) then 1 else 0
```

## V3005: `?` on a value that is not an option or logic (warning)

The query operator unwraps an option (`?type`) or tests a `logic`. On any other value it always succeeds, which is almost never what was meant. Compare instead: `X > 0`.

Raises it:

```verse
F(X : int)<decides> : int = X?
```

Fixed:

```verse
F(X : int)<decides> : int = X > 0
```

## V4001: `<suspends>` code outside a `<suspends>` function

Functions that take time (`Sleep`, anything marked `<suspends>`) and the concurrency expressions `sync`, `race`, `rush` and `branch` are only allowed inside a `<suspends>` function. From ordinary code, start them with `spawn{...}`.

Raises it:

```verse
F() : void = Sleep(1.0)
```

Fixed:

```verse
F()<suspends> : void = Sleep(1.0)
```

## V4002: effect not allowed by the enclosing function

A function declared `<computes>` may not touch mutable state: no `var`, no `set`, no calls to functions that do (including `Print`). Remove the specifier to get the default `<transacts>`, or remove the state access.

Raises it:

```verse
F()<computes> : void = Print("hi")
```

Fixed:

```verse
F() : void = Print("hi")
```

## V4003: conflicting specifiers

A function cannot be both `<decides>` and `<suspends>`. At most one of `<transacts>`, `<computes>` and `<converges>` may be given, and no specifier may repeat.

Raises it:

```verse
F()<decides><suspends> : void = {}
```

Fixed:

```verse
F()<suspends> : void = {}
```

## V4004: unknown specifier

Effect specifiers are `<decides>`, `<suspends>`, `<transacts>`, `<computes>`, `<reads>`, `<writes>` and `<allocates>`. Access specifiers include `<public>`, `<private>`, `<protected>`, `<internal>`, `<override>` and `<final>`.

Raises it:

```verse
F()<decide> : int = 1 = 1
```

Fixed:

```verse
F()<decides> : int = 1 = 1
```

## V4005: specifier in the wrong position

Access specifiers go between the name and the parameter list; effect specifiers go after the parameter list: `Name<public>(Params)<decides> : type = Body`.

Raises it:

```verse
F<decides>() : int = 1 = 1
```

Fixed:

```verse
F()<decides> : int = 1 = 1
```

## V4006: suspending call inside a failure context

A failure context can be rolled back, and time that has passed cannot. So `Sleep`, `Await` and calls to `<suspends>` functions are not allowed in an `if` condition, a `for` header, `option{}`, `logic{}`, `not`, or on the left of `or`. Make the call first, then test its result.

Raises it:

```verse
Fetch()<suspends> : int =
    Sleep(1.0)
    5
F()<suspends> : int =
    if (X := Fetch(), X > 3) then X else 0
```

Fixed:

```verse
Fetch()<suspends> : int =
    Sleep(1.0)
    5
F()<suspends> : int =
    X := Fetch()
    if (X > 3) then X else 0
```

## V4007: `branch` or `rush` directly in a loop

`branch` and `rush` leave tasks running after they return. Directly inside a `loop` or `for` body they would pile up, so Verse rejects them there. Put them in a `<suspends>` function and call it from the loop.

Raises it:

```verse
Tick()<suspends> : void = Sleep(1.0)
F()<suspends> : void =
    loop:
        branch:
            Tick()
        Sleep(1.0)
```

Fixed:

```verse
Tick()<suspends> : void = Sleep(1.0)
Once()<suspends> : void =
    branch:
        Tick()
    Sleep(1.0)
F()<suspends> : void =
    loop:
        Once()
```

## V5001: assignment to something immutable

Only names declared with `var` can be changed with `set`. Parameters and `:=` definitions are constants.

Raises it:

```verse
F() : int =
    X : int = 1
    set X = 2
    X
```

Fixed:

```verse
F() : int =
    var X : int = 1
    set X = 2
    X
```

## V5002: `=` used as assignment

`=` is the equality comparison. Assignment is `set Name = Value`, and it only works on a `var`. A new name is defined with `Name := Value`.

Raises it:

```verse
F() : int =
    var X : int = 1
    X = 2
    X
```

Fixed:

```verse
F() : int =
    var X : int = 1
    set X = 2
    X
```

## V5003: `break` outside `loop`

`break` leaves the innermost `loop`. A `for` cannot be exited early: filter in its header instead (`for (X : Xs, X > 0)`).

Raises it:

```verse
F(Xs : []int) : void =
    for (X : Xs):
        break
```

Fixed:

```verse
F(Xs : []int) : void =
    for (X : Xs, X > 0):
        Print("{X}")
```

## V5004: `var` field in a struct

Struct fields are immutable. Declare the variable that holds the struct with `var` and update a field with `set Value.Field = ...`, which replaces the whole value.

Raises it:

```verse
p := struct:
    var X : int = 0
```

Fixed:

```verse
p := struct:
    X : int = 0
```

## V6001: not supported by this toolchain yet

The construct is valid Verse, or may be, but this toolchain does not implement it yet. The code may still compile in UEFN.

## V6002: name from an unloaded API (warning)

The file imports a Fortnite or Unreal Engine module, and the name is neither defined locally nor modelled by the built-in stand-ins for common devices. API digest files are not loaded, so this use is not checked. Compile in UEFN to verify it.

## V6004: module-level `var` (warning)

UEFN only allows module-scoped variables of `weak_map` type. This toolchain accepts a plain module-level `var` so small test programs can keep state, but the code will not compile in UEFN.

## V6005: signalling a device event

Device events such as `button_device.InteractedWithEvent` have type `listenable(t)`: code may `Await()` them or `Subscribe(Handler)` to them, but only the game fires them. In a test, make the device fire with its `Sim...` method. For an event of your own, declare it as `event(t)`, which can be signalled.

Raises it:

```verse
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

my_device := class(creative_device):
    Button : button_device = button_device{}
    Press(Agent : agent) : void =
        Button.InteractedWithEvent.Signal(Agent)
```

Fixed:

```verse
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

my_device := class(creative_device):
    Button : button_device = button_device{}

TestPress()<transacts> : void =
    Device := my_device{}
    Device.Button.SimInteract(SimAddPlayer("Ana"))
```

## V6006: missing `using` for an API name (warning)

The name is a Fortnite or Verse API type that lives in a module this file does not import. UEFN rejects the file until the module is imported. The fix adds the `using` line.

Raises it:

```verse
using { /Fortnite.com/Devices }

my_device := class(creative_device):
    Greet(Agent : agent) : void =
        Print("hello")
```

Fixed:

```verse
using { /Verse.org/Simulation }
using { /Fortnite.com/Devices }

my_device := class(creative_device):
    Greet(Agent : agent) : void =
        Print("hello")
```

## V6007: simulation-only name in device code (warning)

Names that start with `Sim` (`SimAddPlayer`, `Button.SimInteract`, `Timer.SimDuration`, ...) belong to this toolchain's stand-ins for Fortnite devices. They do not exist in UEFN. Use them only in `Main` or in functions named `Test...` or `Sim...`, ideally in a separate test file, so the device code stays valid for UEFN.

Raises it:

```verse
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

my_device := class(creative_device):
    Button : button_device = button_device{}
    OnBegin<override>()<suspends> : void =
        Button.SimInteract(agent{})
```

Fixed:

```verse
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

my_device := class(creative_device):
    Button : button_device = button_device{}

TestPress()<transacts> : void =
    Device := my_device{}
    Device.Button.SimInteract(SimAddPlayer("Ana"))
```

## V7001: missing `<override>`

A method that replaces an inherited one must be marked `<override>` between its name and parameter list. Fields cannot be redeclared in a subclass at all.

Raises it:

```verse
a := class:
    Hi() : string = "a"
b := class(a):
    Hi() : string = "b"
```

Fixed:

```verse
a := class:
    Hi() : string = "a"
b := class(a):
    Hi<override>() : string = "b"
```

## V7002: `<override>` without a parent method

The method is marked `<override>` but no parent class or interface declares a method with that name.

Raises it:

```verse
a := class:
    Hi<override>() : string = "a"
```

Fixed:

```verse
a := class:
    Hi() : string = "a"
```

## V7003: override does not match

An override must take the same parameters as the method it replaces and may not add `<decides>` or `<suspends>`.

Raises it:

```verse
a := class:
    Hi() : string = "a"
b := class(a):
    Hi<override>(N : int) : string = "b"
```

Fixed:

```verse
a := class:
    Hi() : string = "a"
b := class(a):
    Hi<override>() : string = "b"
```

## V7004: abstract method not implemented

A concrete class must give a body to every method its parents and interfaces declare without one.

Raises it:

```verse
named := interface:
    Label() : string
ann := class(named) {}
```

Fixed:

```verse
named := interface:
    Label() : string
ann := class(named):
    Label<override>() : string = "Ann"
```

## V7005: member is not accessible here

A `<private>` member can be used only inside the class that declares it; a `<protected>` member also inside subclasses. Members with no access specifier are `<internal>`: visible throughout their module.

Raises it:

```verse
vault := class:
    Code<private> : int = 42
Peek(V : vault) : int = V.Code
```

Fixed:

```verse
vault := class:
    Code<private> : int = 42
    Reveal<public>() : int = Code
Peek(V : vault) : int = V.Reveal()
```

## V8001: `case` is not exhaustive

Outside a failure context a `case` must cover every value: list all values of a closed enum, or add a default arm `_ => Value`.

Raises it:

```verse
color := enum{Red, Green}
F(C : color) : int =
    case (C):
        color.Red => 1
```

Fixed:

```verse
color := enum{Red, Green}
F(C : color) : int =
    case (C):
        color.Red => 1
        _ => 0
```

## V8002: more than one default arm

A `case` may have a single `_` arm, and it should come last.

Raises it:

```verse
F(X : int) : int =
    case (X):
        _ => 1
        _ => 0
```

Fixed:

```verse
F(X : int) : int =
    case (X):
        1 => 1
        _ => 0
```

## V9001: construct in the wrong place

Only definitions are allowed at module level; statements belong in a function. Classes, structs, interfaces and enums must be defined at module level, not inside functions.

Raises it:

```verse
Print("hi")
```

Fixed:

```verse
Main() : void = Print("hi")
```

## V9002: unreachable code (warning)

Code after `return` or `break`, or a `case` arm after `_`, never runs.

## V9003: loop never ends (warning)

A `loop` with no `break` or `return` runs forever. In a `<suspends>` function that is fine if the body sleeps; otherwise add an exit.

## V9004: expression always succeeds (warning)

The left side of `or`, or the body of `logic{}`, cannot fail, so the fallback is dead code.
