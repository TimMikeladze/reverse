# source: written for this project from the Book of Verse, Concurrency
# uefn-verified: no
#> outer race won by the timer at 1.000000
#> nothing from the cancelled subtree by 20.000000
Deep(Name : string)<suspends> : void =
    sync:
        block:
            Sleep(5.0)
            Print("{Name} inner a")
        block:
            Sleep(6.0)
            Print("{Name} inner b")
    Print("{Name} done")

Main()<suspends> : void =
    race:
        Deep("cancelled")
        Sleep(1.0)
    Print("outer race won by the timer at {GetSimulationElapsedTime()}")
    Sleep(19.0)
    Print("nothing from the cancelled subtree by {GetSimulationElapsedTime()}")
