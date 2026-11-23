# source: written for this project from the Book of Verse, Mutability
# uefn-verified: no
A() : int =
    var X : int = 1
    X = 2 #! V5002
    X

B() : int =
    X : int = 1
    set X = 2 #! V5001
    X

C(P : int) : int =
    set P = 2 #! V5001
    P

thing := class:
    Fixed : int = 0
    var Free : int = 0

D(T : thing) : void =
    set T.Free = 1
    set T.Fixed = 1 #! V5001
