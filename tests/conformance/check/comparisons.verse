# source: written for this project from the Book of Verse, Operators
# uefn-verified: no
A(X : int)<decides> : int = X < 10
B(S : string)<decides> : string = S = "yes"
C(S : string, T : string)<decides> : void = S < T #! V2004
D(X : int)<decides> : void = X = 1.0 #! V2008
