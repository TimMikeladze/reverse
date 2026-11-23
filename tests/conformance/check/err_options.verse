# source: written for this project from the Book of Verse, Failure and Container types
# uefn-verified: no
A(O : ?int) : int = O #! V2006
B(X : int) : ?int = X #! V2006
C(X : int)<decides> : int = X? #! V3005

thing := class:
    Size : int = 0

D(O : ?thing) : int = O.Size #! V1003
