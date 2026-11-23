# source: written for this project from the Book of Verse, Primitive types
# uefn-verified: no
#> hello world
#> length 5
#> utf-8 code units: 5
#> first h
#> equal
#> braces {literal}
#> 42 1.500000 x str
#> joined a, b, c
Main() : void =
    Print("hello" + " " + "world")
    Print("length {"hello".Length}")
    # The Book: Length counts UTF-8 code units, so the accented letter counts twice.
    Print("utf-8 code units: {"José".Length}")
    if (C := "hello"[0]):
        Print("first {C}")
    if ("abc" = "abc", "abc" <> "abd"):
        Print("equal")
    Print("braces \{literal\}")
    Print("{42} {1.5} {'x'} {"str"}")
    Print("joined {Join(array{"a", "b", "c"}, ", ")}")
