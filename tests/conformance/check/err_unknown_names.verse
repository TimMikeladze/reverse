# source: written for this project from the Book of Verse, (suggestions are a property of this checker)
# uefn-verified: no
Count : int = 1
A() : int = Cuont #! V1001
B() : ?int = null #! V1001
C(Xs : []int) : int = Xs.length #! V1003
D(Flag : bool) : void = {} #! V1002
E() : void = print("hi") #! V1001

color := enum{Red, Green}
F() : color = color.Blue #! V1003
