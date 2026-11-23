# source: written for this project from the Book of Verse, Failure
# uefn-verified: no
#> 3
#> 7
#> chained 5
Small(X : int)<decides> : int = X < 10

Main() : void =
    if (V := Small[3]):
        Print("{V}")
    if (V := 7 > 2):
        Print("{V}")
    X := 5
    if (0 < X, V := X < 10):
        Print("chained {V}")
