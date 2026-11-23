# source: written for this project from the Book of Verse, Failure and Container types
# uefn-verified: no
#> array 1 2 3
#> map length 1
#> after success 1 20 3
Main() : void =
    var Xs : []int = array{1, 2, 3}
    var M : [string]int = map{"a" => 1}
    # Index 9 is out of range: both writes in this condition are undone.
    if (set Xs[1] = 20, set M["b"] = 2, set Xs[9] = 0):
        Print("unexpected")
    if (A := Xs[0], B := Xs[1], C := Xs[2]):
        Print("array {A} {B} {C}")
    Print("map length {M.Length}")
    if (set Xs[1] = 20):
        if (A := Xs[0], B := Xs[1], C := Xs[2]):
            Print("after success {A} {B} {C}")
