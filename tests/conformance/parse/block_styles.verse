# source: written for this project from the Book of Verse, Expressions
# uefn-verified: no
Braces(X : int) : int = { Y := X + 1; Y * 2 }

Indented(X : int) : int =
    Y := X + 1
    Y * 2

Inline(X : int) : int = X + 1

Mixed(X : int) : int =
    if (X > 0) {
        X
    } else {
        0 - X
    }
