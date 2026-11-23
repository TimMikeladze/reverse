# source: written for this project from the Book of Verse, Operators and Functions
# uefn-verified: no
plain := class:
    ID : int = 0

tagged := class<unique>:
    ID : int = 0

pair := struct:
    A : int = 0

Show(X : int) : string = "int"
Show(X : float) : string = "float"

A(X : plain, Y : plain)<decides> : void = X = Y #! V2015
B(X : tagged, Y : tagged)<decides> : void = X = Y
C(X : pair, Y : pair)<decides> : void = X = Y
D() : string = Show(1)
E() : string = Show("text") #! V2016
F() : string = Show(1, 2) #! V2016
