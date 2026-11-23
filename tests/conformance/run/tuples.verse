# source: written for this project from the Book of Verse, Container types and Functions
# uefn-verified: no
#> 1 two 3.000000
#> spread 3
#> nested 5
Add(A : int, B : int) : int = A + B

Main() : void =
    T := (1, "two", 3.0)
    Print("{T(0)} {T(1)} {T(2)}")
    Pair := (1, 2)
    # A tuple passed as the only argument spreads across the parameters.
    Print("spread {Add(Pair)}")
    N := ((4, 5), 6)
    Print("nested {N(0)(1)}")
