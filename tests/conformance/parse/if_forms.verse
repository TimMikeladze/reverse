# source: written for this project from the Book of Verse, Control Flow
# uefn-verified: no
A(X : int) : int =
    if (X > 0) { 1 } else { 0 }

B(X : int) : int =
    if (X > 0):
        1
    else if (X = 0):
        0
    else:
        -1

C(X : int) : int = if (X > 0) then 1 else 0

D(X : int) : int =
    if:
        X > 0
        X < 10
    then:
        1
    else:
        0

E(X : int, Y : int) : int =
    if (X > 0, Y > 0) then X + Y else 0
