# source: written for this project from the Book of Verse, Concurrency and Mutability
# uefn-verified: no
#> total 30
#> counter 3
counter := class:
    var N : int = 0
    Add(Delay : float, Amount : int)<suspends> : void =
        Sleep(Delay)
        set N += Amount

Main()<suspends> : void =
    C := counter{}
    var Finished : int = 0
    sync:
        block:
            C.Add(3.0, 10)
            set Finished += 1
        block:
            C.Add(1.0, 10)
            set Finished += 1
        block:
            C.Add(2.0, 10)
            set Finished += 1
    Print("total {C.N}")
    Print("counter {Finished}")
