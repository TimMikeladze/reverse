# source: written for this project from the Book of Verse, Operators and Primitive types
# uefn-verified: no
A(Score : int) : string = "Score: " + Score #! V2004
B(X : int) : float = X + 1.0 #! V2004
C : float = 1 #! V2001
D(X : float) : int = X #! V2006
E(X : int) : float = X * 1.0
F(X : int, Y : float) : float = X * Y
G(Score : int) : string = "Score: {Score}"
H(X : int) : void = Print(X) #! V2001
