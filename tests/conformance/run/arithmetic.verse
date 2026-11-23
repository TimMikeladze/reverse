# source: written for this project from the Book of Verse, Operators and Primitive types
# uefn-verified: no
#> 7 -1 12
#> 3.500000 0.500000 3.000000 0.750000
#> 6.000000
#> floor 3 ceil 4
#> floor -4 ceil -3
#> quotient 3 mod 1
#> 9 over 3 is 3
#> min 2 max 5 abs 4
Main() : void =
    Print("{3 + 4} {3 - 4} {3 * 4}")
    Print("{1.5 + 2.0} {2.0 - 1.5} {1.5 * 2.0} {1.5 / 2.0}")
    # int * float is the one mixed operation that is allowed.
    Print("{2 * 3.0}")
    # int / int is a rational and can fail, so it needs a failure context.
    if (F := Floor(7 / 2), C := Ceil(7 / 2)):
        Print("floor {F} ceil {C}")
    if (F := Floor(-7 / 2), C := Ceil(-7 / 2)):
        Print("floor {F} ceil {C}")
    if (Q := Quotient[7, 2], M := Mod[7, 2]):
        Print("quotient {Q} mod {M}")
    if (9 / 3 = 3):
        Print("9 over 3 is 3")
    Print("min {Min(2, 5)} max {Max(2, 5)} abs {Abs(-4)}")
