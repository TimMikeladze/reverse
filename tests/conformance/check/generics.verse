# source: written for this project from the Book of Verse, Functions (parametric)
# uefn-verified: no
Identity(X : t where t : type) : t = X
Wrap(X : t where t : type) : []t = array{X}

A() : int = Identity(1)
B() : string = Identity(1) #! V2006
C() : []float = Wrap(1.5)
D() : []float = Wrap("x") #! V2006

# Inside the body a type variable is opaque: it supports no arithmetic.
Bad(X : t where t : type) : t = X + X #! V2004
