# source: written for this project from the Book of Verse, (error recovery is a property of this parser)
# uefn-verified: no
# Two independent syntax errors are both reported, and checking continues
# into later definitions.
F() : int =
    X := ) #! V0001
    2

G() : int = let Y = 3 #! V0001

H() : float = 1 #! V2006
