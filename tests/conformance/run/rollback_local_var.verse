# source: written for this project from the Book of Verse, Failure (speculative execution)
# uefn-verified: no
#> before 1
#> after failed if 1
#> after successful if 2
Main() : void =
    var X : int = 1
    Print("before {X}")
    # The write happens, then the condition fails, so the write is undone.
    if (set X = 99, X < 0):
        Print("unexpected")
    Print("after failed if {X}")
    if (set X = 2, X > 0):
        Print("after successful if {X}")
