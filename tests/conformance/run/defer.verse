# source: written for this project from the Book of Verse, Control Flow (defer)
# uefn-verified: no
#> body
#> second defer
#> first defer
#> early exit
#> cleanup on return
#> iteration 1
#> end of iteration 1
#> iteration 2
#> end of iteration 2
#> failed scope skipped its defer: 0
Order() : void =
    defer:
        Print("first defer")
    defer:
        Print("second defer")
    Print("body")

Early(X : int) : int =
    defer:
        Print("cleanup on return")
    if (X > 0):
        Print("early exit")
        return 1
    0

counter := class:
    var N : int = 0

Risky(C : counter)<decides><transacts> : void =
    defer:
        set C.N += 100
    1 = 2

Main() : void =
    Order()
    Early(5)
    for (I := 1..2):
        defer:
            Print("end of iteration {I}")
        Print("iteration {I}")
    C := counter{}
    if (Risky[C]) {}
    Print("failed scope skipped its defer: {C.N}")
