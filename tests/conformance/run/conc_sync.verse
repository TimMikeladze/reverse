# source: written for this project from the Book of Verse, Concurrency
# uefn-verified: no
#> a starts
#> b starts
#> b ends
#> a ends
#> results 1 2, elapsed 2.000000
Work(Name : string, Delay : float, Result : int)<suspends> : int =
    Print("{Name} starts")
    Sleep(Delay)
    Print("{Name} ends")
    Result

Main()<suspends> : void =
    # Arms start in order and run together; the result keeps source order.
    Both := sync:
        Work("a", 2.0, 1)
        Work("b", 1.0, 2)
    Print("results {Both(0)} {Both(1)}, elapsed {GetSimulationElapsedTime()}")
