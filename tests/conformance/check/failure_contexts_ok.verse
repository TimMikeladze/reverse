# source: written for this project from the Book of Verse, Failure
# uefn-verified: no
Half(X : int)<decides> : int =
    Mod[X, 2] = 0
    Floor(X / 2)

InIf(Xs : []int) : int =
    if (X := Xs[0], H := Half[X]) then H else 0

InFor(Xs : []int) : []int =
    for (X : Xs, H := Half[X]) { H }

WithOr(Xs : []int) : int = Xs[0] or 0
WithOption(Xs : []int) : ?int = option{Xs[0]}
WithLogic(Xs : []int) : logic = logic{Xs[0]}

InDecidesBody(Xs : []int)<decides> : int =
    X := Xs[0]
    X > 0
    not (X = 5)
    X
