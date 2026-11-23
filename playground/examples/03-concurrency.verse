# Time is simulated: Sleep(60.0) finishes instantly and every run is the same.

Collect(Name : string, Seconds : float, Coins : int)<suspends> : int =
    Sleep(Seconds)
    Print("{Name} collected {Coins} coins at {GetSimulationElapsedTime()}s")
    Coins

Main()<suspends> : void =
    # sync: run together, wait for all, results in source order
    Both := sync:
        Collect("ann", 2.0, 5)
        Collect("bob", 1.0, 3)
    Print("together they have {Both(0) + Both(1)}")

    # race: first to finish wins, the rest are cancelled
    Winner := race:
        Collect("slow", 30.0, 100)
        Collect("fast", 4.0, 1)
        block:
            Sleep(60.0)
            0
    Print("the race paid {Winner}")

    # spawn: start a task and collect its result later
    Task := spawn{Collect("background", 10.0, 7)}
    Print("carrying on while it works")
    Print("background task returned {Task.Await()}")
    Print("total simulated time: {GetSimulationElapsedTime()}s")
