# source: written for this project from the Book of Verse, Primitive types (ToString)
# uefn-verified: no
#> point (1, 2)
#> still works for ints: 5
point := class:
    X : int = 0
    Y : int = 0

ToString(P : point) : string = "({P.X}, {P.Y})"

Main() : void =
    P := point{X := 1, Y := 2}
    Print("point {P}")
    Print("still works for ints: {ToString(5)}")
