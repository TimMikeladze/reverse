# source: written for this project from the Book of Verse, Failure
# uefn-verified: no
#> not of a failure succeeds, x = 1
#> not of a success fails, x = 1
#> not not keeps the result but drops the effects, x = 1
Bump(C : counter)<decides><transacts> : void =
    set C.N += 1
    C.N > 0

counter := class:
    var N : int = 1

Main() : void =
    C := counter{}
    if (not (set C.N = 5, C.N < 0)):
        Print("not of a failure succeeds, x = {C.N}")
    if (not Bump[C]):
        Print("unexpected")
    else:
        Print("not of a success fails, x = {C.N}")
    if (not not Bump[C]):
        Print("not not keeps the result but drops the effects, x = {C.N}")
