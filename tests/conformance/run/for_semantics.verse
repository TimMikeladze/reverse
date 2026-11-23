# source: written for this project from the Book of Verse, Control Flow
# uefn-verified: no
#> squares 1 4 9
#> evens 2 4
#> product 11 12 21 22
#> indexed 0:a 1:b
#> empty range 0
#> chars h i
#> defined 2 4 6
#> skipped missing key 0
Show(Xs : []int) : string =
    var S : string = ""
    for (X : Xs):
        set S += " {X}"
    S

Main() : void =
    Print("squares{Show(for (I := 1..3) { I * I })}")
    Print("evens{Show(for (I := 1..5, Mod[I, 2] = 0) { I })}")
    Print("product{Show(for (X := 1..2, Y := 1..2) { X * 10 + Y })}")
    var Line : string = "indexed"
    for (I -> S : array{"a", "b"}):
        set Line += " {I}:{S}"
    Print(Line)
    Print("empty range {(for (I := 5..1) { I }).Length}")
    var Chars : string = "chars"
    for (C : "hi"):
        set Chars += " {C}"
    Print(Chars)
    Print("defined{Show(for (I := 1..3, D := I * 2) { D })}")
    M : [string][]int = map{}
    Print("skipped missing key {(for (X : M["nope"]) { X }).Length}")
