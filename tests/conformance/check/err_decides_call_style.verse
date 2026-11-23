# source: written for this project from the Book of Verse, Failure and Functions
# uefn-verified: no
Half(X : int)<decides> : int = Mod[X, 2] = 0
Twice(X : int) : int = X * 2

A() : int = Half(4) or 0 #! V3002
B() : int = Twice[4] #! V3003
C() : int = Mod(7, 2) or 0 #! V3002
