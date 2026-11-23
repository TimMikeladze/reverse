# source: written for this project from the Book of Verse, Functions
# uefn-verified: no
Add(A : int, B : int) : int = A + B
Greet(Name : string, ?Loud : logic = false) : string = Name

A() : int = Add(1) #! V2002
B() : int = Add(1, 2, 3) #! V2002
C() : int = Add(1, "two") #! V2001
D() : string = Greet("x", ?Quiet := true) #! V2002
E() : string = Greet("x", ?Loud := true)
F() : string = Greet("x")
G() : int = Add((1, 2))
