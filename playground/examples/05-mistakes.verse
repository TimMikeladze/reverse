# Habits from other languages, each caught with a fix.
# Open Problems and press "Apply fix" on each one.

Half(X : int)<decides> : int =
    Mod[X, 2] == 0
    Floor(X / 2)

Main() : void =
    Ready : logic = true
    if (Ready):
        Print("half of 8 is {Half(8) or 0}")
    Flag : bool = false
    Total : float = 1
    Names := array{"ann", "bob"}
    Print("there are {Names.length} names")
    Nothing : ?int = null
