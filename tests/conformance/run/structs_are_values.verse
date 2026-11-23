# source: written for this project from the Book of Verse, Structs & Enums
# uefn-verified: no
#> copy kept 1.000000, original now 9.000000
#> equal by fields
point := struct:
    X : float = 0.0
    Y : float = 0.0

Main() : void =
    var P : point = point{X := 1.0}
    Copy := P
    set P.X = 9.0
    Print("copy kept {Copy.X}, original now {P.X}")
    if (point{X := 2.0} = point{X := 2.0, Y := 0.0}):
        Print("equal by fields")
