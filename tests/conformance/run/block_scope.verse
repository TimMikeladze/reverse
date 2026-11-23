# source: written for this project from the Book of Verse, Expressions
# uefn-verified: no
#> 3
#> 6
Main() : void =
    Total := block:
        A := 1
        A + 2
    Print("{Total}")
    Seq := (1; 2; Total * 2)
    Print("{Seq}")
