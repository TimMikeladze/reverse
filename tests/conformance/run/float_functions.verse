# source: written for this project from the Book of Verse, Primitive types
# uefn-verified: no
#> floor 2 ceil 3 int 2
#> round ties to even: 2 4
#> round of infinity failed
#> sqrt 3.000000
Main() : void =
    if (F := Floor[2.7], C := Ceil[2.2], I := Int[2.9]):
        Print("floor {F} ceil {C} int {I}")
    if (A := Round[2.5], B := Round[3.5]):
        Print("round ties to even: {A} {B}")
    if (not Round[1.0 / 0.0]):
        Print("round of infinity failed")
    Print("sqrt {Sqrt(9.0)}")
