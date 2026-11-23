# source: written for this project from the Book of Verse, Container types
# uefn-verified: no
A(T : tuple(int, string)) : int = T(0)
B(T : tuple(int, string)) : int = T[0] or 0 #! V2003
C(T : tuple(int, string)) : int = T(5) #! V2002
D(Xs : []int) : int = Xs(0) #! V2003
E(M : [string]int)<decides> : int = M[1] #! V2001
