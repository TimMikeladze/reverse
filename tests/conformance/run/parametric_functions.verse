# source: written for this project from the Book of Verse, Functions (parametric)
# uefn-verified: no
#> 5 five
#> first 10
#> empty has no first
#> pair b 2
Identity(X : t where t : type) : t = X

First(Xs : []t where t : type)<decides> : t = Xs[0]

Swap(P : tuple(a, b) where a : type, b : type) : tuple(b, a) = (P(1), P(0))

Main() : void =
    N : int = Identity(5)
    S : string = Identity("five")
    Print("{N} {S}")
    if (F := First[array{10, 20}]):
        Print("first {F}")
    Empty : []int = array{}
    if (not First[Empty]):
        Print("empty has no first")
    Q := Swap((2, "b"))
    Print("pair {Q(0)} {Q(1)}")
