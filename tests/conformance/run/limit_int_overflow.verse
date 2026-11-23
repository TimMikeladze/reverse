# source: written for this project from the Book of Verse, Primitive types (64-bit int with an overflow error)
# uefn-verified: no
#> 9223372036854775807
#= status: error:overflow
Main() : void =
    Big := 9223372036854775807
    Print("{Big}")
    Print("{Big + 1}")
