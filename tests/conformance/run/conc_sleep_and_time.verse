# source: written for this project from the Book of Verse, Concurrency (the clock is simulated)
# uefn-verified: no
#> start 0.000000
#> after one hour 3600.000000
Main()<suspends> : void =
    Print("start {GetSimulationElapsedTime()}")
    Sleep(3600.0)
    Print("after one hour {GetSimulationElapsedTime()}")
