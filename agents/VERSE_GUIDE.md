# Verse for AI agents

Read this before writing Verse. Verse looks like Python or Swift but differs in ways that
language models reliably get wrong. Every `verse` block below passes `reverse check`.

## The loop to follow

1. Write the code.
2. Run `reverse check file.verse --json` (or the `verse_check` tool). Fix every error.
   Each diagnostic has a `code`, a `help` sentence and often a machine-applicable `fix`.
3. Add `Test...()` functions and run `reverse test file.verse` (or `verse_test`).
   Concurrent code runs too, on a simulated clock.
4. Never present Verse that has not passed step 2. If a rule is unclear, run
   `reverse explain <CODE>` for a failing and a fixed example.

`reverse` checks the language itself. It does not know Fortnite or Unreal Engine APIs
(devices, players, UI). Names from those modules get a `V6002` warning and are not checked;
compile in UEFN to verify them.

## The ten mistakes to avoid

| Wrong (habit from other languages) | Right (Verse) |
| --- | --- |
| `x = 5` to assign | `set X = 5`, and only if `X` was declared with `var` |
| `let x = 5`, `int x = 5` | `X := 5` or `X : int = 5`; mutable: `var X : int = 5` |
| `if (Flag)` on a `logic` | `if (Flag?)` |
| `==`, `!=`, `&&`, `\|\|`, `!` | `=`, `<>`, `and`, `or`, `not` |
| `Items[0]` used anywhere | `Items[0]` only where failure is handled (see below) |
| `Find(Key)` on a function that can fail | `Find[Key]`: square brackets mark a call that can fail |
| `"Score: " + Score` | `"Score: {Score}"` |
| `1` where a float is needed, `X + 1.0` with int `X` | `1.0`; convert with `X * 1.0` |
| `while (...)`, `for (i = 0; i < n; i++)` | `loop:` with `break`; `for (I := 0..N - 1):` |
| `null`, `nil` | an empty option: `false`, of type `?t` |

## Definitions

```verse
MaxHealth : int = 100            # constant with a type
Greeting := "hello"              # constant, type inferred

Example() : void =
    var Score : int = 0          # mutable: needs `var` and a type
    set Score = 10
    set Score += 5
    Print("{Greeting}: {Score} of {MaxHealth}")
```

A name cannot be redefined or shadowed inside a function. Comments start with `#`;
block comments are `<# ... #>`. Blocks are either indented after `:` / `=`, or wrapped in `{ }`.

## Functions and effects

```verse
Add(A : int, B : int) : int = A + B

# Specifiers: access before the parameter list, effects after it.
Area<public>(W : float, H : float)<computes> : float = W * H

# A named parameter with a default; callers write ?Bonus := 5.
Total(Base : int, ?Bonus : int = 0) : int = Base + Bonus

UseThem() : int = Add(1, 2) + Total(10, ?Bonus := 5)
```

The last expression is the return value. Effects that matter:

- no specifier: may read and write state (this is `<transacts>`), cannot fail.
- `<decides>`: may fail. Must be called with `[]`.
- `<suspends>`: takes time. Only callable from `<suspends>` code or `spawn{}`.
- `<computes>`: pure. No `var`, no `set`, no `Print`.
- `<decides>` and `<suspends>` cannot be combined.

## Failure is control flow

Some expressions can fail instead of producing a value: comparisons (`A < B`, `A = B`),
queries (`Opt?`, `Flag?`), indexing (`Items[0]`, `Map[Key]`), integer division (`A / B`),
and `[]` calls to `<decides>` functions. They are legal only in a failure context:

- an `if` condition or a `for` header
- the left side of `or`
- inside `option{...}` or `logic{...}`
- the body of a `<decides>` function

```verse
# Fails when the array is empty or the first element is not positive.
FirstPositive(Items : []int)<decides> : int =
    First := Items[0]
    First > 0
    First

Describe(Items : []int) : string =
    if (Value := FirstPositive[Items]):       # binds Value when it succeeds
        "first is {Value}"
    else:
        "nothing positive up front"

FirstOrZero(Items : []int) : int = Items[0] or 0          # fallback
MaybeFirst(Items : []int) : ?int = option{Items[0]}       # failure becomes an empty option
IsEmpty(Items : []int) : logic = logic{not Items[0]}      # failure becomes false
```

A condition is a list: `if (A := F[], B := G[A], B > 0)` runs left to right and stops at
the first failure. A comparison yields its left operand, so `X := Score > 0` binds `Score`.

**Rollback.** If a failure context fails, every `set` made inside it is undone, including
writes made by functions it called. Use this instead of manual cleanup:

```verse
account := class:
    var Balance : int = 0

Withdraw(A : account, Amount : int)<decides><transacts> : void =
    set A.Balance -= Amount
    A.Balance >= 0                 # if this fails, the `set` above is rolled back

TryWithdraw(A : account, Amount : int) : string =
    if (Withdraw[A, Amount]) then "ok" else "insufficient funds, balance unchanged"
```

## Types

| Type | Literal | Notes |
| --- | --- | --- |
| `int` | `42`, `0xFF` | 64-bit here; overflow is a runtime error |
| `float` | `1.0`, `2.5e3` | needs the decimal point |
| `logic` | `true`, `false` | test with `Flag?`, never bare |
| `string` | `"text {Expr}"` | `.Length`; `{}` interpolates int, float, char, string |
| `char` | `'a'` | |
| `[]t` | `array{1, 2}` | `.Length`, `Items[I]` can fail, `+` concatenates |
| `[k]v` | `map{"a" => 1}` | `Map[Key]` can fail; iteration keeps insertion order |
| `?t` | `option{X}`, `false` | unwrap with `Opt?` in a failure context |
| `tuple(a, b)` | `(1, "x")` | `Pair(0)` reads an element (parentheses, not brackets) |

No implicit conversions. int to float: `X * 1.0`. float to int: `Floor[X]`, `Ceil[X]`,
`Round[X]`, `Int[X]` (all can fail). `int / int` gives a `rational`; use `Floor(A / B)`,
or `Quotient[A, B]` and `Mod[A, B]`. `logic`, options and containers have no `ToString`.

```verse
Average(Items : []int)<decides> : float =
    Items.Length > 0
    var Sum : int = 0
    for (Item : Items):
        set Sum += Item
    (Sum * 1.0) / (Items.Length * 1.0)

ShowFlag(Flag : logic) : string = if (Flag?) then "on" else "off"
```

Arrays and maps are values. To change one, it must be a `var`, and updates build a new value:

```verse
Collect() : []int =
    var Items : []int = array{}
    set Items += array{1}                  # append
    var Ages : [string]int = map{}
    if (set Ages["ann"] = 31) {}           # element assignment can fail, so it needs a context
    if (set Items[0] = 5) {}
    Items
```

## Control flow

```verse
Sign(N : int) : string =
    if (N < 0):
        "negative"
    else if (N = 0):
        "zero"
    else:
        "positive"

SumEvens(Limit : int) : int =
    var Sum : int = 0
    for (I := 1..Limit, Mod[I, 2] = 0):    # range is inclusive; the filter skips odd numbers
        set Sum += I
    Sum

Squares(Items : []int) : []int =
    for (Item : Items) { Item * Item }     # `for` is an expression: it returns an array

Labels(Items : []string) : []string =
    for (Index -> Item : Items) { "{Index}: {Item}" }

CountDown(Start : int) : int =
    var N : int = Start
    loop:
        if (N <= 0) then break
        set N -= 1
    N
```

`for` cannot be left early and has no `continue`: filter in the header. `break` only exits
`loop`. `if` used as a value needs an `else`.

## Enums, structs, classes

```verse
color := enum{Red, Green, Blue}

Name(C : color) : string =
    case (C):
        color.Red => "red"
        color.Green => "green"
        _ => "blue"

point := struct:                 # value type: copied on assignment, no methods
    X : float = 0.0
    Y : float = 0.0

shape := class<abstract>:
    Name : string                # no default: required when instantiated
    Area() : float               # no body: subclasses must override

circle := class(shape):
    Radius : float = 1.0
    var Hits : int = 0
    Area<override>() : float = 3.14159 * Radius * Radius
    Hit()<transacts> : void = set Hits += 1

MakeCircle() : float =
    C := circle{Name := "c", Radius := 2.0}     # fields are set with :=
    C.Hit()
    C.Area()
```

Classes are references; structs are values. Inside a method, fields are used by bare name
or through `Self`. An override needs `<override>`; call the parent with `(super:)Method()`.
Narrow a type with a failable cast: `if (C := circle[SomeShape])`.

## More of the language

```verse
# Overloads are chosen by the number and types of the arguments.
Surface(Side : float) : float = Side * Side
Surface(W : float, H : float) : float = W * H

# An extension method adds a method to an existing type.
(Xs : []int).Total() : int =
    var Sum : int = 0
    for (X : Xs):
        set Sum += X
    Sum

# A parametric function works for any type.
FirstOr(Xs : []t, Default : t where t : type) : t = Xs[0] or Default

UseMore() : float = Surface(2.0) + Surface(2.0, 3.0) + (array{1, 2}.Total() * 1.0) + FirstOr(array{0.5}, 0.0)
```

`defer:` runs a block when the enclosing scope exits, including when a task is cancelled.
`<private>` and `<protected>` members are enforced. Class instances can be compared with
`=` only if the class is declared `class<unique>`.

## Concurrency

Code that takes time is `<suspends>`. Time is simulated here: `Sleep(60.0)` returns at once
and every run is reproducible, so timing logic can be tested exactly.

```verse
Fetch(Delay : float, Value : int)<suspends> : int =
    Sleep(Delay)
    Value

Demo()<suspends> : int =
    Both := sync:                 # run together, wait for all: a tuple in source order
        Fetch(2.0, 1)
        Fetch(1.0, 2)
    Fastest := race:              # first to finish wins, the others are cancelled
        Fetch(5.0, 10)
        Fetch(1.0, 20)
    Task := spawn{Fetch(3.0, 100)}    # a background task; allowed from any code
    Both(0) + Both(1) + Fastest + Task.Await()

Ticker(Done : event(int))<suspends> : void =
    branch:                       # background work, cancelled when this function returns
        loop:
            Sleep(1.0)
    Sleep(3.5)
    Done.Signal(3)                # wakes one task waiting in Done.Await()
```

Rules that trip models up:

- A `<suspends>` function can only be called from `<suspends>` code or inside `spawn{}`.
- Nothing that suspends may appear in a failure context. Not `if (X := Fetch(1.0, 2), X > 0)`:
  call first (`X := Fetch(1.0, 2)`), then test (`if (X > 0)`).
- `<suspends>` and `<decides>` cannot be combined. Return `?t` or `logic` instead of failing.
- `rush` is `race` without cancelling the losers. `branch` and `rush` are not allowed
  directly inside a `loop` or `for` body.
- An event is created with `event(int){}`. Each `Signal` wakes exactly one awaiter and calls
  every handler added with `Subscribe`; a signal nobody listens for is lost.

## Testing convention

`reverse test` runs every function named `Test...` that takes no arguments. A `<decides>`
test passes when every line succeeds, so comparisons act as assertions. A `<suspends>` test
cannot fail, so it returns `logic` and passes when the result is `true`:

```verse
Double(X : int) : int = X * 2

TestDouble()<decides><transacts> : void =
    Double(2) = 4
    Double(0) = 0
    not (Double(1) = 3)

Slow(X : int)<suspends> : int =
    Sleep(2.0)
    X * 2

TestSlow()<suspends> : logic =
    Result := Slow(4)
    logic{Result = 8 and GetSimulationElapsedTime() = 2.0}
```

## Fortnite devices

A file that imports a `/Fortnite.com/...` module is checked and run against built-in
stand-ins for the common device types: `creative_device`, `button_device`,
`trigger_device`, `timer_device`, `item_granter_device`, `item_spawner_device`,
`score_manager_device`, `hud_message_device`, `player_spawner_device`, `end_game_device`,
`teleporter_device`, `mutator_zone_device`, `elimination_manager_device`, `switch_device`,
`billboard_device`, `barrier_device`, plus `agent`, `player`, `fort_character`,
`fort_playspace` and `log`. Names match Epic's public API; behaviour is a simple model.

```verse
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

gate_device := class(creative_device):
    @editable
    Button : button_device = button_device{}
    @editable
    Score : score_manager_device = score_manager_device{}
    var Opened : logic = false

    OnBegin<override>()<suspends> : void =
        Button.InteractedWithEvent.Subscribe(OnPressed)

    OnPressed(Agent : agent) : void =
        Score.Activate(Agent)
        set Opened = true

TestGateOpens()<decides><transacts> : void =
    Ana := SimAddPlayer("Ana")
    Gate := gate_device{}
    SimStart(Gate)
    not Gate.Opened?
    Gate.Button.SimInteract(Ana)
    Gate.Opened?
    SimScore(Ana) = 1
```

- Device events are `listenable(t)`: `Subscribe(Handler)` or `Await()`, never `Signal`
  (`V6005`). `button_device.InteractedWithEvent` sends `agent`; `trigger_device.TriggeredEvent`
  and the timer events send `?agent`.
- A handler is a plain method `(Payload) : void`. It cannot be `<suspends>` or `<decides>`;
  for slow work write `spawn{DoSlowThing(Agent)}` inside it.
- `Subscribe` returns a `cancelable`. Keep it in `var Sub : ?cancelable = false` and call
  `Cancel()` to stop listening.
- Names that start with `Sim` exist only here: `SimAddPlayer(Name)`, `SimStart(Device)`,
  `SimScore(Agent)`, `SimGameEnded[]`, `Button.SimInteract(Agent)`, `Zone.SimEnter(Agent)`,
  `timer_device{SimDuration := 5.0}` and so on. Use them only in `Test...` functions (or
  `Main`), and keep tests in a second file: `reverse test device.verse device_test.verse`,
  or the `files` argument of the MCP tools. In device code they are warning `V6007`.
- Import the module of every API type you name (`agent` and `player`:
  `/Verse.org/Simulation`; `fort_character`: `/Fortnite.com/Characters`); `V6006` supplies
  the missing line.
- A device or member the stand-ins do not model is warning `V6002`: it is not checked, and
  calling it at run time is an error. Say so to the user and leave that part for UEFN.

## Bevy gameplay

Attach a `.verse` asset to an entity with `VerseScript`. Define handlers using the
same name and payload type registered by the Bevy app. A registered component getter
or setter can fail if the entity lacks the component, so call it with square brackets
inside a failure context. `Sleep` resumes on the fixed clock. Check Bevy-bound source
with `app.check_verse(source)` to use the game's binding registry.
[Bevy plugin setup](../docs/bevy-integration.md)

## Not yet supported by reverse

Rejected with a clear error: nested functions, tuple-destructuring parameters, parametric
classes, qualified names other than `(super:)`.
Not modelled: Fortnite and Unreal Engine APIs beyond the device stand-ins above (UI widgets,
items, transforms, teams), `weak_map`, persistence, live variables. For those, write a small
stand-in class of your own with the same shape and test the logic here.
