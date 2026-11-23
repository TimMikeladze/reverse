# source: written for this project from the Book of Verse, Control Flow
# uefn-verified: no
#> negative zero positive
Sign(N : int) : string =
    if (N < 0):
        return "negative"
    if (N = 0):
        return "zero"
    "positive"

Main() : void =
    Print("{Sign(-1)} {Sign(0)} {Sign(1)}")
