# source: written for this project from the Book of Verse, Control Flow
# uefn-verified: no
A(Xs : []int) : []int = for (X : Xs) { X }
B(Xs : []int) : []int =
    for (X : Xs):
        X
C(Xs : []int) : []int = for (I -> X : Xs) { I + X }
D() : []int = for (I := 1..3) { I }
E(M : [string]int) : []int = for (K -> V : M) { V }
F(Xs : []int) : []int = for (X : Xs, X > 0, Y := X * 2) { Y }
G() : []int = for (X := 1..2, Y := 1..2) { X * 10 + Y }
