# source: written for this project from the Book of Verse, Functions (extension methods)
# uefn-verified: no
#> 14
#> 8
#> hello!
#> sum 6
#> own method wins
(X : int).Double() : int = X * 2
(S : string).Shout() : string = S + "!"
(Xs : []int).Sum() : int =
    var Total : int = 0
    for (X : Xs):
        set Total += X
    Total

thing := class:
    Describe() : string = "own method wins"

(T : thing).Describe() : string = "extension"

Main() : void =
    Seven := 7
    Print("{Seven.Double()}")
    Print("{Seven.Double().Double() - 20}")
    Print("hello".Shout())
    Print("sum {array{1, 2, 3}.Sum()}")
    Print(thing{}.Describe())
