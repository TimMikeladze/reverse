# source: written for this project from the Book of Verse, Control Flow
# uefn-verified: no
#> matched 10
#> unmatched failed
Pick(N : int)<decides> : int =
    case (N):
        1 => 10
        2 => 20

Main() : void =
    if (V := Pick[1]):
        Print("matched {V}")
    if (not Pick[3]):
        Print("unmatched failed")
