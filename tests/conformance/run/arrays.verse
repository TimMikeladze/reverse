# source: written for this project from the Book of Verse, Container types
# uefn-verified: no
#> length 3, first 10
#> out of range failed
#> find 1
#> slice 20 30
#> insert 10 15 20 30
#> remove 10 30
#> replace 10 99 30
#> concat length 5
#> copies diverge: 1 2
#> appended tuple length 5
Show(Xs : []int) : string =
    var S : string = ""
    for (X : Xs):
        set S += " {X}"
    S

Main() : void =
    Xs := array{10, 20, 30}
    if (F := Xs[0]):
        Print("length {Xs.Length}, first {F}")
    if (not Xs[3]):
        Print("out of range failed")
    if (I := Xs.Find[20]):
        Print("find {I}")
    if (S := Xs.Slice[1, 3]):
        Print("slice{Show(S)}")
    if (S := Xs.Insert[1, array{15}]):
        Print("insert{Show(S)}")
    if (S := Xs.RemoveElement[1]):
        Print("remove{Show(S)}")
    if (S := Xs.ReplaceElement[1, 99]):
        Print("replace{Show(S)}")
    Print("concat length {(Xs + array{40, 50}).Length}")
    var A : []int = array{1}
    var B : []int = A
    if (set B[0] = 2, X := A[0], Y := B[0]):
        Print("copies diverge: {X} {Y}")
    var C : []int = array{1, 2, 3}
    set C += (4, 5)
    Print("appended tuple length {C.Length}")
