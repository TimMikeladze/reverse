# source: written for this project from the Book of Verse, Failure
# uefn-verified: no
#> or: 0, n = 0
#> option: empty, n = 0
#> logic: false, n = 0
#> or kept the success: 1, n = 1
counter := class:
    var N : int = 0

BumpAndFail(C : counter)<decides><transacts> : int =
    set C.N += 1
    C.N > 100

BumpAndPass(C : counter)<decides><transacts> : int =
    set C.N += 1
    C.N > 0

Main() : void =
    C := counter{}
    A := BumpAndFail[C] or 0
    Print("or: {A}, n = {C.N}")
    O := option{BumpAndFail[C]}
    Print("option: {if (O?) then "full" else "empty"}, n = {C.N}")
    L := logic{BumpAndFail[C]}
    Print("logic: {if (L?) then "true" else "false"}, n = {C.N}")
    B := BumpAndPass[C] or 0
    Print("or kept the success: {B}, n = {C.N}")
