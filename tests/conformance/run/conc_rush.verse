# source: written for this project from the Book of Verse, Concurrency
# uefn-verified: no
#> quick ends
#> rush gave 2 at 1.000000
#> slow ends
#> slow arm kept running: 3.500000
#> helper returned at 4.500000
#> abandoned arm was cancelled with its function: 9.500000
Work(Name : string, Delay : float, Result : int)<suspends> : int =
    Sleep(Delay)
    Print("{Name} ends")
    Result

Helper()<suspends> : void =
    rush:
        Work("abandoned", 2.0, 1)
        Sleep(1.0)
    Print("helper returned at {GetSimulationElapsedTime()}")

Main()<suspends> : void =
    First := rush:
        Work("slow", 3.0, 1)
        Work("quick", 1.0, 2)
    Print("rush gave {First} at {GetSimulationElapsedTime()}")
    Sleep(2.5)
    Print("slow arm kept running: {GetSimulationElapsedTime()}")
    # A rush loser lives only as long as the function that started it.
    Helper()
    Sleep(5.0)
    Print("abandoned arm was cancelled with its function: {GetSimulationElapsedTime()}")
