# source: written for this project from the Book of Verse, Concurrency
# uefn-verified: no
#> task starts
#> spawn returned
#> task ends
#> awaited 42 at 2.000000
#> awaited again 42 at 2.000000
#> outlived its creator: 6.000000
Compute()<suspends> : int =
    Print("task starts")
    Sleep(2.0)
    Print("task ends")
    42

Late()<suspends> : void =
    Sleep(4.0)
    Print("outlived its creator: {GetSimulationElapsedTime()}")

Starter() : void =
    # spawn works from code that is not <suspends>, and the task is not
    # tied to the function that started it.
    spawn{Late()}

Main()<suspends> : void =
    T := spawn{Compute()}
    Print("spawn returned")
    Print("awaited {T.Await()} at {GetSimulationElapsedTime()}")
    Print("awaited again {T.Await()} at {GetSimulationElapsedTime()}")
    Starter()
    Sleep(5.0)
