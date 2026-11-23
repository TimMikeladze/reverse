# source: written for this project from the Book of Verse, Failure and Container types
# uefn-verified: no
Empty : ?int = false
Full : ?int = option{5}

Get(O : ?int) : int = O? or 0
Wrap(Xs : []int) : ?int = option{Xs[0]}

Nested(O : ??int)<decides> : int = O??

Use(O : ?int) : string =
    if (V := O?):
        "has {V}"
    else:
        "empty"
