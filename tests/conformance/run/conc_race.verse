# source: written for this project from the Book of Verse, Concurrency
# uefn-verified: no
#> slow starts
#> fast starts
#> fast ends
#> winner 2 at 1.000000
#> 10 seconds later the loser has still not finished
#> instant wins, second arm never started
Work(Name : string, Delay : float, Result : int)<suspends> : int =
    Print("{Name} starts")
    Sleep(Delay)
    Print("{Name} ends")
    Result

Instant()<suspends> : int = 7

Main()<suspends> : void =
    Winner := race:
        Work("slow", 5.0, 1)
        Work("fast", 1.0, 2)
    Print("winner {Winner} at {GetSimulationElapsedTime()}")
    Sleep(10.0)
    Print("10 seconds later the loser has still not finished")
    # An arm that finishes without suspending wins before later arms start.
    First := race:
        Instant()
        Work("never", 1.0, 3)
    if (First = 7):
        Print("instant wins, second arm never started")
