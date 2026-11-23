# source: written for this project from the Book of Verse, Failure and Container types
# uefn-verified: no
#> full 5
#> empty
#> fallback 0
#> nested 7
#> logic query passed
Describe(O : ?int) : string =
    if (V := O?) then "full {V}" else "empty"

Main() : void =
    Print(Describe(option{5}))
    Print(Describe(false))
    Missing : ?int = false
    Print("fallback {Missing? or 0}")
    Nested : ??int = option{option{7}}
    if (V := Nested??):
        Print("nested {V}")
    Flag : logic = true
    if (Flag?, not false?):
        Print("logic query passed")
