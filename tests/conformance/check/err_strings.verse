# source: written for this project from the Book of Verse, Primitive types
# uefn-verified: no
A(Flag : logic) : string = "{Flag}" #! V2007
B(O : ?int) : string = "{O}" #! V2007
C(Xs : []int) : string = "{Xs}" #! V2007
D(X : int, F : float, C : char, S : string) : string = "{X} {F} {C} {S}"
E(Flag : logic) : string = ToString(Flag) #! V2007
