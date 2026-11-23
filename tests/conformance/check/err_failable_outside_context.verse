# source: written for this project from the Book of Verse, Failure
# uefn-verified: no
Index(Xs : []int) : int = Xs[0] #! V3001
Lookup(M : [string]int) : int = M["a"] #! V3001
Unwrap(O : ?int) : int = O? #! V3001
Divide(A : int, B : int) : int = Floor(A / B) #! V3001
Compare(A : int) : void =
    X := A > 0 #! V3001
