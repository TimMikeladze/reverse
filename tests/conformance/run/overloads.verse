# source: written for this project from the Book of Verse, Functions (overloading)
# uefn-verified: no
#> int 3
#> float 2.500000
#> text hi
#> two ints 7
#> custom point(1, 2) and builtin 5
Show(X : int) : string = "int {X}"
Show(X : float) : string = "float {X}"
Show(X : string) : string = "text {X}"
Show(A : int, B : int) : string = "two ints {A + B}"

point := class:
    X : int = 0
    Y : int = 0

# A user ToString adds to the built-in ones instead of replacing them.
ToString(P : point) : string = "point({P.X}, {P.Y})"

Main() : void =
    Print(Show(3))
    Print(Show(2.5))
    Print(Show("hi"))
    Print(Show(3, 4))
    Print("custom {point{X := 1, Y := 2}} and builtin {ToString(5)}")
