# source: written for this project from the Book of Verse, Concurrency
# uefn-verified: no
#> a 1
#> b 1
#> a 2
#> b 2
#> a 3
#> b 3
Spin(Name : string)<suspends> : void =
    for (I := 1..3):
        Print("{Name} {I}")
        # Sleep(0.0) yields: every other task runs before this one continues.
        Sleep(0.0)

Main()<suspends> : void =
    sync:
        Spin("a")
        Spin("b")
