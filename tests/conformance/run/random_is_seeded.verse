# source: written for this project from the Book of Verse, (determinism is a property of this interpreter)
# uefn-verified: no
#> in range
#> shuffled keeps length 5
Main() : void =
    R := GetRandomInt(1, 6)
    if (R >= 1, R <= 6):
        Print("in range")
    Print("shuffled keeps length {Shuffle(array{1, 2, 3, 4, 5}).Length}")
