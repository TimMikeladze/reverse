# source: written for this project from the Book of Verse, Primitive types (rational)
# uefn-verified: no
#> half plus third is five sixths
#> 3/4
#> floor of 7/2 plus 1/2 is 4
#> product 1
Main() : void =
    if (1 / 2 + 1 / 3 = 5 / 6):
        Print("half plus third is five sixths")
    if (Q := 6 / 8):
        Print("{Q}")
    if (F := Floor(7 / 2 + 1 / 2)):
        Print("floor of 7/2 plus 1/2 is {F}")
    if (P := Floor((2 / 3) * (3 / 2))):
        Print("product {P}")
