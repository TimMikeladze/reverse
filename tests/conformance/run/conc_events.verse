# source: written for this project from the Book of Verse, Concurrency
# uefn-verified: no
#> first got 1
#> second got 2
#> nobody got 3
#> third signal was discarded: only 2 deliveries
Listen(Name : string, E : event(int), Log : log)<suspends> : void =
    Value := E.Await()
    Print("{Name} got {Value}")
    set Log.Count += 1

log := class:
    var Count : int = 0

Main()<suspends> : void =
    E : event(int) = event(int){}
    L := log{}
    spawn{Listen("first", E, L)}
    spawn{Listen("second", E, L)}
    # Each signal wakes exactly one awaiter, oldest first.
    E.Signal(1)
    Sleep(0.0)
    E.Signal(2)
    Sleep(0.0)
    E.Signal(3)
    Sleep(0.0)
    Print("nobody got 3")
    Print("third signal was discarded: only {L.Count} deliveries")
