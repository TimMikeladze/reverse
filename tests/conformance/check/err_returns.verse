# source: written for this project from the Book of Verse, Functions
# uefn-verified: no
A(X : int) : int =
    if (X > 0): #! V2006
        X

B(X : int) : string = X #! V2006

C(X : int) : int =
    var Y : int = X
    set Y += 1

D(X : int) : int =
    if (X > 0):
        return "no" #! V2001
    X

E() : int #! V2012

F() = 1 #! V2013
