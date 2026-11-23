# source: written for this project from the Book of Verse, Operators
# uefn-verified: no
#> int division by zero failed
#> float division by zero is Inf
Main() : void =
    Zero := 0
    if (X := Floor(1 / Zero)):
        Print("unexpected {X}")
    else:
        Print("int division by zero failed")
    Print("float division by zero is {1.0 / 0.0}")
