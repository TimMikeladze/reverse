# source: written for this project from the Book of Verse, Concurrency
# uefn-verified: no
#> branch body starts at once
#> after branch
#> tick 1
#> tick 2
#> worker returns at 2.500000
#> no ticks after the function returned: 10.000000
Worker()<suspends> : void =
    branch:
        Print("branch body starts at once")
        var N : int = 0
        loop:
            Sleep(1.0)
            set N += 1
            Print("tick {N}")
    Print("after branch")
    Sleep(2.5)
    Print("worker returns at {GetSimulationElapsedTime()}")

Main()<suspends> : void =
    Worker()
    Sleep(7.5)
    Print("no ticks after the function returned: {GetSimulationElapsedTime()}")
