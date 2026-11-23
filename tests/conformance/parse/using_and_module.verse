# source: written for this project from the Book of Verse, Modules and Paths
# uefn-verified: no
using { /Verse.org/Random }

helpers := module:
    Twice(X : int) : int = X * 2

Use() : int = helpers.Twice(4)
