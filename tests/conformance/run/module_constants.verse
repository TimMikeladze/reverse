# source: written for this project from the Book of Verse, Modules and Paths
# uefn-verified: no
#> 110
#> twice 8
Base : int = 100
Derived : int = Base + 10

helpers := module:
    Twice(X : int) : int = X * 2

Main() : void =
    Print("{Derived}")
    Print("twice {helpers.Twice(4)}")
