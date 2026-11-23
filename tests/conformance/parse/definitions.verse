# source: written for this project from the Book of Verse, Expressions and Mutability
# uefn-verified: no
Typed : int = 1
Inferred := 2

F() : int =
    A : int = 1
    B := 2
    var C : int = 3
    set C = 4
    set C += A
    set C -= B
    set C *= 2
    C
