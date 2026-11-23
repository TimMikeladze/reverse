# source: written for this project from the Book of Verse, Control Flow
# uefn-verified: no
color := enum{Red, Green, Blue}

A(C : color) : int =
    case (C): #! V8001
        color.Red => 1
        color.Green => 2

B(C : color) : int =
    case (C):
        color.Red => 1
        color.Green => 2
        color.Blue => 3

C(N : int) : string =
    case (N): #! V8001
        0 => "zero"

D(Xs : []int) : void =
    for (X : Xs):
        break #! V5003

E() : void =
    break #! V5003

F(X : float) : int =
    case (X): #! V2010
        _ => 0

G() : void =
    R := 1..3 #! V2009
