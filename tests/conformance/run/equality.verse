# source: written for this project from the Book of Verse, Operators
# uefn-verified: no
#> arrays equal by value
#> tuples equal by value
#> same object
#> different objects
#> NaN equals itself
#> string equals array of chars
thing := class<unique>:
    ID : int = 0

Main() : void =
    if (array{1, 2} = array{1, 2}, array{1, 2} <> array{2, 1}):
        Print("arrays equal by value")
    if ((1, "a") = (1, "a")):
        Print("tuples equal by value")
    A := thing{}
    B := thing{}
    C := A
    if (A = C):
        Print("same object")
    if (A <> B):
        Print("different objects")
    NaN := 0.0 / 0.0
    if (NaN = NaN):
        Print("NaN equals itself")
    if ("abc" = array{'a', 'b', 'c'}):
        Print("string equals array of chars")
