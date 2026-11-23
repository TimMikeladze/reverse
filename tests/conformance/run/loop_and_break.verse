# source: written for this project from the Book of Verse, Control Flow
# uefn-verified: no
#> 0
#> 1
#> 2
#> done at 3
Main() : void =
    var I : int = 0
    loop:
        if (I >= 3) then break
        Print("{I}")
        set I += 1
    Print("done at {I}")
