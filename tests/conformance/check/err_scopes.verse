# source: written for this project from the Book of Verse, Expressions
# uefn-verified: no
A() : int =
    X := 1
    X := 2 #! V1004
    X

B(P : int) : int =
    P := 2 #! V1004
    P

C() : int =
    if (1 = 1):
        Inner := 5
    Inner #! V1001

Twice(X : int) : int = X
Twice(Y : int) : int = Y #! V1004

# A name bound in an `if` condition is visible in the body, not after it.
D(Xs : []int) : int =
    if (First := Xs[0]):
        First
    else:
        0
